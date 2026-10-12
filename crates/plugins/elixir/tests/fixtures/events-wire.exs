alias PoolsterModels.{Models, Events}
alias PoolsterModels.Events.{SendOrder, ReceiveOrder, KafkaClient, KafkaCodec, KafkaSubscription, KafkaRecord}
assert = fn condition, label -> if !condition, do: raise("failed #{label}") end
rejects = fn label, fun ->
  rejected = try do
    fun.()
    false
  rescue
    _ -> true
  catch
    :exit, _ -> true
  end
  assert.(rejected, "accepted #{label}")
end
definition = SendOrder.definition()
payload = Models.OrderCreatedPayload.from_map(%{"id" => "order-1", "quantity" => 1})
headers = Models.OrderCreatedHeaders.from_map(%{"trace" => "trace-1"})
assert.(!Map.has_key?(Models.OrderCreatedPayload.to_map(payload), "note"), "absence")
null = Models.OrderCreatedPayload.from_map(%{"id" => "order-1", "quantity" => 1, "note" => nil})
assert.(Map.has_key?(Models.OrderCreatedPayload.to_map(null), "note"), "explicit null")
rejects.("constraints", fn -> KafkaCodec.encode(definition, %{"id" => "x", "quantity" => 0}, headers: headers, key: "key") end)
rejects.("outgoing UTF8", fn -> KafkaCodec.encode(definition, payload, headers: %{"trace" => <<255>>}, key: "key") end)
rejects.("key UTF8", fn -> KafkaCodec.encode(definition, payload, headers: headers, key: <<255>>) end)
rejects.("omitted declared key", fn -> KafkaCodec.encode(definition, payload, headers: headers) end)
keyless = put_in(definition, [:wire, "key_schema"], nil) |> Map.put(:key_type, nil)
{<<>>, _, _} = KafkaCodec.encode(keyless, payload, headers: headers)
valid = {:kafka_message, 0, "key", ~s({"id":"x","quantity":1}), :create, 0, [{"trace", "trace"}]}
rejects.("ambiguous null-empty declared key", fn -> KafkaCodec.decode(put_elem(valid, 2, <<>>), definition) end)
assert.(KafkaCodec.decode(put_elem(valid, 2, <<>>), keyless).key == nil, "keyless incoming key")
values = KafkaCodec.decode(valid, definition)
assert.(values.payload.id == "x", "typed payload")
for {index, value} <- [{3, <<255>>}, {2, <<255>>}, {6, [{"trace", <<255>>}]}, {6, [{"trace", "a"}, {"trace", "b"}]}, {6, [{"trace", nil}]}, {3, nil}, {3, ~s({"id":"x","id":"y","quantity":1})}, {3, ~s({"id":"x","quantity":NaN})}] do
  rejects.("wire #{index}", fn -> KafkaCodec.decode(put_elem(valid, index, value), definition) end)
end
{:ok, client} = Events.connect()
if System.argv() == ["broker"] do
  :ok = SendOrder.send(client, payload, headers: headers, key: "key")
  native_producer = :sys.get_state(client).producers["elixir-bound-producer"]
  {:ok, connection} = :brod_client.get_leader_connection(native_producer, definition.wire["topic"], 0)
  assert.(elem(:sys.get_state(connection), 1) == "elixir-bound-producer", "native producer wire client ID")
  {:error, :conflicting_group_binding} = ReceiveOrder.subscribe(client, group: "wrong")
  {:ok, subscription} = ReceiveOrder.subscribe(client)
  wait_record = fn subscription ->
    Enum.reduce_while(1..40, nil, fn _, _ ->
      case KafkaSubscription.poll(subscription, 1000) do
        {:ok, record} -> {:halt, record}
        :timeout -> {:cont, nil}
        error -> raise "poll failed #{inspect(error)}"
      end
    end)
  end
  record = wait_record.(subscription)
  native_consumer = :sys.get_state(subscription).client
  {:ok, connection} = :brod_client.get_leader_connection(native_consumer, definition.wire["topic"], 0)
  assert.(elem(:sys.get_state(connection), 1) == "elixir-bound-consumer", "native consumer wire client ID")
  assert.(record && record.payload.id == "order-1" && record.headers.trace == "trace-1" && record.key == "key", "broker values")
  # Exercise the actual success-ack helper and prove failed-handler redelivery.
  KafkaSubscription.close(subscription)
  {:ok, failed} = ReceiveOrder.subscribe(client)
  rejects.("handler failure", fn -> KafkaSubscription.run(failed, fn item -> assert.(item.payload.id == "order-1", "handler payload"); raise "application failed" end) end)
  KafkaSubscription.close(failed)
  {:ok, replay} = ReceiveOrder.subscribe(client)
  record = wait_record.(replay)
  assert.(record && record.payload.id == "order-1", "failed handler replay")
  :ok = KafkaRecord.ack(record)
  {:error, :foreign_or_already_acknowledged_record} = KafkaRecord.ack(record)
  Process.sleep(1200)
  pending_poll = Task.async(fn -> KafkaSubscription.poll(replay, 5_000) end)
  Process.sleep(50)
  started = System.monotonic_time(:millisecond)
  :ok = KafkaSubscription.cancel(replay)
  {:error, :cancelled} = Task.await(pending_poll, 1_000)
  assert.(System.monotonic_time(:millisecond) - started < 1_000, "pending poll cancellation")
  {:error, :cancelled} = KafkaSubscription.poll(replay)
  KafkaSubscription.close(replay)
  for index <- 0..7 do
    wire = Map.merge(ReceiveOrder.definition().wire, %{"topic" => "#{definition.wire["topic"]}-bad-#{index}", "group_id" => nil})
    invalid_definition = %{definition | wire: wire}
    for _ <- 1..2 do
      {:ok, invalid} = KafkaClient.subscribe(client, invalid_definition, group: "#{wire["topic"]}-consumer")
      error = Enum.reduce_while(1..30, nil, fn _, _ ->
        case KafkaSubscription.poll(invalid, 1000) do
          {:error, reason} -> {:halt, reason}
          :timeout -> {:cont, nil}
          {:ok, _} -> raise "broker accepted malformed record #{index}"
        end
      end)
      assert.(!is_nil(error), "broker malformed #{index}")
      {:error, ^error} = KafkaSubscription.poll(invalid)
      started = System.monotonic_time(:millisecond)
      KafkaSubscription.close(invalid)
      assert.(System.monotonic_time(:millisecond) - started < 5_000, "bounded invalid close")
    end
  end
end
KafkaClient.close(client)
IO.puts("ELIXIR KAFKA CONFORMANCE OK")
