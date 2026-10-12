require_relative 'lib/poolster_models'
include PoolsterModels
include PoolsterModels::Events

def assert(condition, label)
  raise "failed #{label}" unless condition
end

def rejects(label)
  begin
    yield
  rescue StandardError
    return
  end
  raise "accepted #{label}"
end

Dir['lib/**/*.rb'].each { |file| raise "syntax #{file}" unless system('ruby', '-c', file, out: File::NULL) }
raise 'gem package build' unless system('gem', 'build', 'poolster_models.gemspec', out: File::NULL)
definition = SendOrder::DEFINITION
payload = Models::OrderCreatedPayload.from_hash('id' => 'order-1', 'quantity' => 1)
headers = Models::OrderCreatedHeaders.from_hash('trace' => 'trace-1')
assert(!payload.to_h.key?('note'), 'absence preserved')
null_payload = Models::OrderCreatedPayload.from_hash('id' => 'order-1', 'quantity' => 1, 'note' => nil)
assert(null_payload.to_h.key?('note'), 'explicit null preserved')
client = Events.connect
rejects('constraint') { SendOrder.send(client, {'id'=>'x','quantity'=>0}, headers: headers, key: 'key') }
rejects('nonfinite') { SendOrder.send(client, {'id'=>'x','quantity'=>Float::NAN}, headers: headers, key: 'key') }
rejects('outgoing UTF8') { SendOrder.send(client, payload, headers: {'trace'=>"\xff".b}, key: 'key') }
rejects('key UTF8') { SendOrder.send(client, payload, headers: headers, key: "\xff".b) }
rejects('group binding') { ReceiveOrder.subscribe(client, group: 'wrong-group') }
message_type = Struct.new(:payload, :headers, :key, :topic, :partition, :offset)
valid = message_type.new('{"id":"x","quantity":1}', {'trace'=>'trace'}, 'key', 'topic', 0, 0)
absent = valid.dup
absent.key = nil
assert(KafkaCodec.decode(absent, definition)[2].nil?, 'absent key remains absent')
empty = valid.dup
empty.key = ''
rejects('empty constrained key') { KafkaCodec.decode(empty, definition) }
values = KafkaCodec.decode(valid, definition)
assert(values[0].id == 'x', 'typed payload')
[[:payload,"\xff".b], [:key,"\xff".b], [:headers,{'trace'=>"\xff".b}], [:headers,{'trace'=>['a','b']}], [:headers,{'trace'=>nil}], [:payload,nil], [:payload,'{"id":"x","id":"y","quantity":1}']].each do |field,value|
  bad=valid.dup
  bad[field]=value
  rejects("wire #{field}") { KafkaCodec.decode(bad, definition) }
end
if ARGV[0] == 'broker'
  SendOrder.send(client, payload, headers: headers, key: 'key')
  native_producer = client.instance_variable_get(:@producers).values.first
  assert(native_producer.name.include?('ruby-bound-producer'), 'native producer client ID')
  subscription = ReceiveOrder.subscribe(client)
  assert(subscription.instance_variable_get(:@consumer).name.include?('ruby-bound-consumer'), 'native consumer client ID')
  record = nil
  deadline=Process.clock_gettime(Process::CLOCK_MONOTONIC)+30
  until record || Process.clock_gettime(Process::CLOCK_MONOTONIC)>deadline
    record=subscription.poll(timeout: 1)
  end
  assert(record && record.payload.id=='order-1' && record.headers.trace=='trace-1' && record.key=='key', 'broker values')
  rejects('poll before ack') { subscription.poll }
  # Exercise the actual success-ack helper. Failed handlers leave it pending.
  subscription.close
  handler_subscription = ReceiveOrder.subscribe(client)
  rejects('handler failure') { handler_subscription.run { |item| assert(item.payload.id == 'order-1', 'handler payload'); raise 'application failed' } }
  handler_subscription.close
  replay=ReceiveOrder.subscribe(client)
  record=nil
  deadline=Process.clock_gettime(Process::CLOCK_MONOTONIC)+30
  until record || Process.clock_gettime(Process::CLOCK_MONOTONIC)>deadline
    record=replay.poll(timeout: 1)
  end
  assert(record && record.payload.id=='order-1', 'failed handler redelivery')
  record.ack
  rejects('double ack') { record.ack }
  cancel_thread = Thread.new { sleep 0.1; replay.cancel }
  started = Process.clock_gettime(Process::CLOCK_MONOTONIC)
  assert(replay.poll(timeout: 5).nil?, 'cross-thread cancellation')
  assert(Process.clock_gettime(Process::CLOCK_MONOTONIC) - started < 1, 'bounded cancellation')
  cancel_thread.join
  replay.close
  8.times do |index|
    wire = ReceiveOrder::DEFINITION.wire.merge('topic' => "#{definition.wire['topic']}-bad-#{index}", 'group_id' => nil)
    invalid_definition = KafkaDefinition.new(wire, Models::OrderCreatedPayload, Models::OrderCreatedHeaders, Models::OrderCreatedKey)
    group = "#{wire['topic']}-consumer"
    2.times do
      invalid_subscription = client.subscribe(invalid_definition, group: group)
      error = nil
      deadline = Process.clock_gettime(Process::CLOCK_MONOTONIC) + 30
      until error || Process.clock_gettime(Process::CLOCK_MONOTONIC) > deadline
        begin
          item = invalid_subscription.poll(timeout: 1)
          raise 'invalid record was silently accepted' if item
        rescue StandardError => exception
          error = exception
        end
      end
      assert(error && error.message != 'invalid record was silently accepted', "broker malformed #{index}")
      rejects('poisoned subscription cannot advance') { invalid_subscription.poll }
      started = Process.clock_gettime(Process::CLOCK_MONOTONIC)
      invalid_subscription.close
      assert(Process.clock_gettime(Process::CLOCK_MONOTONIC) - started < 5, 'bounded invalid close')
    end
  end
end
client.close
rejects('closed client') { SendOrder.send(client,payload,headers:headers,key:'key') }
puts 'RUBY KAFKA CONFORMANCE OK'
