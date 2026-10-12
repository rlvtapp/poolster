using Poolster.Models;
using Poolster.Models.Events;
using Confluent.Kafka;
using System.Text;
using System.Text.Json;

static void Check(bool value, string message) { if (!value) throw new Exception(message); }
static void Reject(Action action) { try { action(); } catch (Exception e) when (e is JsonException or ArgumentException or InvalidOperationException) { return; } throw new Exception("Expected rejection"); }
var payload = new OrderCreatedPayload { Id = "order-1", Quantity = 1 };
var headers = new OrderCreatedHeaders { Trace = "trace-1" };
var key = new OrderCreatedKey("account-1");
using var runtime = KafkaDefaults.Connect();
Reject(() => ReceiveOrderEvent.Subscribe(runtime, "wrong"));
Reject(() => SendOrderEvent.SendAsync(runtime, payload with { Quantity = 0 }, headers, key));
Reject(() => SendOrderEvent.SendAsync(runtime, payload, headers with { Trace = "" }, key));
Reject(() => SendOrderEvent.SendAsync(runtime, payload, headers, new OrderCreatedKey("")));
Reject(() => SendOrderEvent.SendAsync(runtime, payload with { Id = "\ud800" }, headers, key));
Reject(() => SendOrderEvent.SendAsync(runtime, payload, headers with { Trace = "\ud800" }, key));
Reject(() => SendOrderEvent.SendAsync(runtime, payload, headers, new OrderCreatedKey("\ud800")));
if (args[0] == "wire") { Console.WriteLine("wire-validation-ok"); return; }
using (var admin = new AdminClientBuilder(new AdminClientConfig { BootstrapServers = "127.0.0.1:29092" }).Build())
{
    await admin.CreateTopicsAsync(new[] { new Confluent.Kafka.Admin.TopicSpecification { Name = SendOrderEvent.Definition.Topic, NumPartitions = 1, ReplicationFactor = 1 } });
}
var delivered = await SendOrderEvent.SendAsync(runtime, payload, headers, key);
Check(delivered.Status == PersistenceStatus.Persisted, "delivery metadata");
KafkaRecord<OrderCreatedPayload, OrderCreatedHeaders, OrderCreatedKey> Poll(KafkaSubscription<OrderCreatedPayload, OrderCreatedHeaders, OrderCreatedKey> subscription)
{
    return subscription.Poll(TimeSpan.FromSeconds(15)) ?? throw new Exception("No record");
}
Check(runtime.ProducerClientNames.Single().Contains("csharp-bound-producer"), "producer client id");
long failedOffset = -1;
using (var subscription = ReceiveOrderEvent.Subscribe(runtime))
{
    Check(subscription.ClientName.Contains("csharp-bound-consumer"), "operation client id");
    try
    {
        subscription.Run(record =>
        {
            Check(record.Payload.Id == "order-1" && record.Headers!.Trace == "trace-1" && record.Key!.Value == "account-1", "roundtrip");
            failedOffset = record.Offset;
            throw new ApplicationException("handler-failed");
        });
        throw new Exception("Run should propagate handler failure");
    }
    catch (ApplicationException e) when (e.Message == "handler-failed") { }
    Reject(() => subscription.Poll(TimeSpan.Zero));
}
using (var subscription = ReceiveOrderEvent.Subscribe(runtime))
{
    var replay = Poll(subscription);
    Check(replay.Payload.Id == "order-1" && replay.Offset == failedOffset, "failed handler offset redelivery");
    replay.Ack();
    Reject(() => replay.Ack());
    using var cts = new CancellationTokenSource();
    var cancel = Task.Run(() => { Thread.Sleep(200); cts.Cancel(); });
    var watch = System.Diagnostics.Stopwatch.StartNew();
    try { subscription.Poll(TimeSpan.FromSeconds(10), cts.Token); throw new Exception("Expected cancellation"); }
    catch (OperationCanceledException) { }
    Check(watch.Elapsed < TimeSpan.FromSeconds(2), "cross-thread cancellation latency");
    cancel.GetAwaiter().GetResult();
}
// Each malformed input uses an independent explicit group and topic so one poison
// record cannot mask another. No generated client auto-provisions infrastructure.
using var producer = new ProducerBuilder<byte[], byte[]>(new ProducerConfig { BootstrapServers = "127.0.0.1:29092" }).Build();
using var adminInvalid = new AdminClientBuilder(new AdminClientConfig { BootstrapServers = "127.0.0.1:29092" }).Build();
var validPayload = Encoding.UTF8.GetBytes("{\"id\":\"order-1\",\"quantity\":1}");
var validKey = Encoding.UTF8.GetBytes("account-1");
Headers ValidHeaders() => new() { { "trace", Encoding.UTF8.GetBytes("trace-1") } };
var invalid = new[]
{
    new Message<byte[], byte[]> { Value = new byte[] { 0xff }, Key = validKey, Headers = ValidHeaders() },
    new Message<byte[], byte[]> { Value = validPayload, Key = new byte[] { 0xff }, Headers = ValidHeaders() },
    new Message<byte[], byte[]> { Value = validPayload, Key = validKey, Headers = new Headers { { "trace", new byte[] { 0xff } } } },
    new Message<byte[], byte[]> { Value = Encoding.UTF8.GetBytes("{\"id\":\"a\",\"id\":\"b\",\"quantity\":1}"), Key = validKey, Headers = ValidHeaders() },
    new Message<byte[], byte[]> { Value = validPayload, Key = validKey, Headers = new Headers { { "trace", validKey }, { "trace", validKey } } },
    new Message<byte[], byte[]> { Value = null!, Key = validKey, Headers = ValidHeaders() },
    new Message<byte[], byte[]> { Value = validPayload, Key = validKey, Headers = new Headers { { "trace", null } } },
    new Message<byte[], byte[]> { Value = Encoding.UTF8.GetBytes("{\"id\":\"a\",\"quantity\":0}"), Key = validKey, Headers = ValidHeaders() },
    new Message<byte[], byte[]> { Value = Encoding.UTF8.GetBytes("{\"id\":\"a\",\"quantity\":NaN}"), Key = validKey, Headers = ValidHeaders() }
};
for (int i = 0; i < invalid.Length; i++)
{
    var topic = $"{SendOrderEvent.Definition.Topic}-invalid-{i}";
    adminInvalid.CreateTopicsAsync(new[] { new Confluent.Kafka.Admin.TopicSpecification { Name = topic, NumPartitions = 1, ReplicationFactor = 1 } }).GetAwaiter().GetResult();
    producer.ProduceAsync(topic, invalid[i]).GetAwaiter().GetResult();
    var definition = new KafkaDefinition<OrderCreatedPayload, OrderCreatedHeaders, OrderCreatedKey>(topic, "csharp-invalid-consumer", $"{topic}-group", "{\"type\":\"object\",\"required\":[\"id\",\"quantity\"],\"properties\":{\"id\":{\"type\":\"string\",\"minLength\":1},\"quantity\":{\"type\":\"integer\",\"minimum\":1}},\"additionalProperties\":false}", "{\"type\":\"object\",\"required\":[\"trace\"],\"properties\":{\"trace\":{\"type\":\"string\",\"minLength\":1}},\"additionalProperties\":false}", "{\"type\":\"string\",\"minLength\":1}");
    using var subscription = runtime.Subscribe(definition);
    Reject(() => Poll(subscription));
    Reject(() => subscription.Poll(TimeSpan.Zero));
}
Console.WriteLine("broker-roundtrip-handler-replay-cancel-invalidwire-ok");
