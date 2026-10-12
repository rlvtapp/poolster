use anyhow::Result;
use output::Go;
use poolster_core::{
    engine::{Handle, Meta, Packages, Plugin, PluginContext, Provision},
    native::events::{EventOperations, model_blocks},
};
use poolster_plugin_go as output;

struct Source {
    meta: Meta,
    input: EventOperations,
}
impl Source {
    fn new() -> Self {
        let document = poolster_input_asyncapi::parse(include_str!(
            "../../../inputs/asyncapi/tests/fixtures/kafka-json.yaml"
        ))
        .unwrap();
        let input =
            poolster_input_asyncapi::lower_operations(&document, &Default::default()).unwrap();
        Self {
            meta: Meta::new(),
            input,
        }
    }
    fn handle(&self) -> Handle<EventOperations> {
        self.meta.handle()
    }
}
impl Plugin<Go> for Source {
    fn kind(&self) -> &'static str {
        "test-events"
    }
    fn meta(&self) -> &Meta {
        &self.meta
    }
    fn supports_native_input(&self) -> bool {
        true
    }
    fn provides(&self) -> Vec<Provision> {
        vec![Provision::of::<EventOperations>()]
    }
    fn generate(&self, cx: &mut PluginContext<'_, Go>) -> Result<()> {
        cx.publish(self.input.clone())
    }
}
fn generate(shared: bool) -> poolster_core::GeneratedTree {
    let source = Source::new();
    let events = output::asyncapi(Some(source.handle()));
    let package = if shared {
        let blocks = model_blocks(Some(source.handle()), "events");
        let models = output::shared_models(None).input_shapes(blocks.handle());
        let events = events.models(models.handle());
        output::package("sdk")
            .with(events)
            .with(models)
            .with(blocks)
            .with(source)
    } else {
        output::package("sdk").with(events).with(source)
    };
    Packages::new().package(package).generate_native().unwrap()
}
#[test]
fn standalone_and_explicit_models_emit_split_packages() {
    for shared in [false, true] {
        let tree = generate(shared);
        let files = tree
            .into_files()
            .map(|(file, _)| (file.path, file.contents))
            .collect::<std::collections::BTreeMap<_, _>>();
        assert!(files.contains_key(std::path::Path::new("sdk/kafka_runtime.go")));
        assert!(files.contains_key(std::path::Path::new("sdk/events_operation_sendOrder.go")));
        assert!(files[std::path::Path::new("sdk/go.mod")].contains("franz-go v1.18.1"));
        assert_eq!(
            files
                .keys()
                .filter(|p| p.to_string_lossy().contains("model_ordercreatedpayload.go"))
                .count(),
            1
        );
    }
}
#[test]
fn unsupported_key_is_diagnosed() {
    let mut source = Source::new();
    source.input.operations[0]
        .message
        .key
        .as_mut()
        .unwrap()
        .kind = poolster_core::SchemaKind::Integer;
    let events = output::asyncapi(Some(source.handle()));
    let error = Packages::new()
        .package(output::package("sdk").with(events).with(source))
        .generate_native()
        .expect_err("non-string Kafka keys require an explicit encoding");
    assert!(format!("{error:#}").contains("string keys"));
}
#[test]
#[ignore = "requires Go toolchain and dependency downloads"]
fn generated_package_compiles_and_validates_wire_messages() {
    for shared in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let tree = generate(shared);
        tree.write_to(dir.path()).unwrap();
        std::fs::write(dir.path().join("sdk/events_test.go"), TEST).unwrap();
        let status = std::process::Command::new("go")
            .args(["test", "-mod=mod", "./..."])
            .current_dir(dir.path().join("sdk"))
            .env("GOCACHE", "/private/tmp/kaji-go-cache")
            .output()
            .unwrap();
        assert!(
            status.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&status.stdout),
            String::from_utf8_lossy(&status.stderr)
        );
    }
}
const TEST: &str = r#"package models
import("context";"encoding/json";"testing";"github.com/twmb/franz-go/pkg/kgo")
type fakeProducer struct{record *kgo.Record; calls int}
func(p *fakeProducer) ProduceSync(_ context.Context, records ...*kgo.Record) kgo.ProduceResults{p.record=records[0];p.calls++;return kgo.ProduceResults{{Record:records[0]}}}
func TestWire(t *testing.T){
 var message SendOrderMessage
 if err:=json.Unmarshal([]byte(`{"Payload":{"id":"a","quantity":1},"Headers":{"trace":"trace"},"Key":"key"}`),&message);err!=nil{t.Fatal(err)}
 producer:=&fakeProducer{};client:=NewKafkaClientWithProducer(producer)
 record,err:=client.SendOrder(context.Background(),message);if err!=nil{t.Fatal(err)}
 received,err:=DecodeReceiveOrder(record);if err!=nil{t.Fatal(err)}
 data,_:=json.Marshal(received.Payload);if string(data)!="{\"id\":\"a\",\"quantity\":1}"{t.Fatalf("wire %s",data)}
 if received.Key==nil || string(*received.Key)!="key"{t.Fatal("key")}
 record.Value=[]byte(`{"id":"a","quantity":0}`);if _,err=DecodeReceiveOrder(record);err==nil{t.Fatal("constraint bypass")}
 record.Value=[]byte(`{"id":"a","quantity":1}`);record.Headers=nil;if _,err=DecodeReceiveOrder(record);err==nil{t.Fatal("missing headers")}
 record.Headers=[]kgo.RecordHeader{{Key:"trace",Value:[]byte("x")},{Key:"trace",Value:[]byte("y")}};if _,err=DecodeReceiveOrder(record);err==nil{t.Fatal("duplicate headers")}
 record.Headers=[]kgo.RecordHeader{{Key:"trace",Value:[]byte("x")}};record.Key=[]byte("");if _,err=DecodeReceiveOrder(record);err==nil{t.Fatal("invalid key")}
 record.Key=nil;record.Value=nil;if _,err=DecodeReceiveOrder(record);err==nil{t.Fatal("tombstone")}
 record.Value=[]byte{0xff};if _,err=DecodeReceiveOrder(record);err==nil{t.Fatal("invalid payload UTF-8")}
 record.Value=[]byte(`{"id":"a","quantity":1}`);record.Key=[]byte{0xff};if _,err=DecodeReceiveOrder(record);err==nil{t.Fatal("invalid key UTF-8")}
 record.Key=nil;record.Headers=[]kgo.RecordHeader{{Key:"trace",Value:[]byte{0xff}}};if _,err=DecodeReceiveOrder(record);err==nil{t.Fatal("invalid header UTF-8")}
 message.Payload.Quantity=0;if _,err=client.SendOrder(context.Background(),message);err==nil || producer.calls!=1{t.Fatal("invalid publish reached broker")}
}
"#;

#[test]
#[ignore = "requires local Kafka broker at 127.0.0.1:29092 and Go dependencies"]
fn generated_client_roundtrips_local_kafka() {
    let dir = tempfile::tempdir().unwrap();
    let topic = format!(
        "poolster-go-events-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let mut tree = poolster_core::GeneratedTree::default();
    for (mut file, _) in generate(true).into_files() {
        file.contents = file.contents.replace("poolster-orders", &topic);
        tree.insert(file).unwrap();
    }
    tree.write_to(dir.path()).unwrap();
    std::fs::write(dir.path().join("sdk/broker_test.go"), BROKER_TEST).unwrap();
    let result = std::process::Command::new("go")
        .args([
            "test",
            "-mod=mod",
            "-run",
            "TestBroker",
            "-timeout",
            "60s",
            "./...",
        ])
        .env("GOCACHE", "/private/tmp/kaji-go-cache")
        .current_dir(dir.path().join("sdk"))
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
}
const BROKER_TEST: &str = r#"package models
import("context";"encoding/json";"errors";"testing";"time";"github.com/twmb/franz-go/pkg/kgo")
func TestBroker(t *testing.T){
 client,err:=NewKafkaClient(DefaultKafkaBrokers,DefaultKafkaClientID,kgo.AllowAutoTopicCreation());if err!=nil{t.Fatal(err)};defer client.Close()
 ctx,cancel:=context.WithTimeout(context.Background(),35*time.Second);defer cancel()
 var message SendOrderMessage;if err=json.Unmarshal([]byte(`{"Payload":{"id":"broker","quantity":1},"Headers":{"trace":"local"},"Key":"account"}`),&message);err!=nil{t.Fatal(err)}
 if _,err=client.SendOrder(ctx,message);err!=nil{t.Fatal(err)}
 // A failed handler must leave the first offset uncommitted. Reopen the same
 // generated binding and require the same record, rather than only a new one.
 failure:=errors.New("handler rejected record")
 var failedOffset int64=-1
 err=client.ReceiveOrder(ctx,func(_ context.Context,message ReceiveOrderMessage,record *kgo.Record) error {
   failedOffset=record.Offset;return failure
 },KafkaConsumeOptions{FromBeginning:true})
 if !errors.Is(err,failure) || failedOffset<0 {t.Fatalf("failed handler result offset=%d err=%v",failedOffset,err)}
 seen:=false
 err=client.ReceiveOrder(ctx,func(_ context.Context,message ReceiveOrderMessage,record *kgo.Record) error {
   if message.Payload.ID!="broker" || message.Key==nil || string(*message.Key)!="account" || record.Offset!=failedOffset {t.Fatal("incorrect broker delivery or failed-handler offset was committed")};seen=true;cancel();return nil
 },KafkaConsumeOptions{FromBeginning:true})
 if !seen || !errors.Is(err,context.Canceled){t.Fatalf("broker receive seen=%v err=%v",seen,err)}
}
"#;

#[test]
fn stale_shared_model_revision_is_rejected() {
    let original = Source::new();
    let mut changed = Source::new();
    changed.input.operations[0].topic = "different-topic".into();
    let blocks = model_blocks(Some(original.handle()), "events");
    let models = output::shared_models(None).input_shapes(blocks.handle());
    let events = output::asyncapi(Some(changed.handle())).using_models(models.handle());
    let error = Packages::new()
        .package(
            output::package("sdk")
                .with(events)
                .with(models)
                .with(blocks)
                .with(original)
                .with(changed),
        )
        .generate_native()
        .expect_err("stale event models must not bind");
    assert!(format!("{error:#}").contains("revision"));
}
