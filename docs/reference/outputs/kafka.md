# Kafka event packages

AsyncAPI describes messages, channels and send/receive operations. Poolster's Kafka
outputs generate data models and broker clients; they do not turn events into HTTP
endpoints. Broker-independent [models-only packages](shared-models.md) remain available.

## Generate a package

```json
{
  "input": { "format": "asyncapi", "path": "messages.yaml" },
  "output": { "path": "generated" },
  "packages": [
    { "language": "typescript", "path": "ts", "plugins": [{ "name": "asyncapi" }] },
    { "language": "go", "path": "go", "name": "example.com/acme/events", "plugins": [{ "name": "asyncapi" }] },
    { "language": "rust", "path": "rust", "name": "acme-events", "plugins": [{ "name": "asyncapi" }] },
    { "language": "python", "path": "python", "name": "acme-events", "plugins": [{ "name": "asyncapi" }] },
    { "language": "java", "path": "java", "name": "com.acme.events", "plugins": [{ "name": "asyncapi" }] },
    { "language": "csharp", "path": "csharp", "name": "Acme.Events", "plugins": [{ "name": "asyncapi" }] }
  ]
}
```

Run `poolster generate --config poolster.json`. The input supplies the server addresses,
static topic bindings and operation names. Generation writes files without connecting
to Kafka; running the generated client needs a broker and provisioned topics. Clients
do not create broker infrastructure.

Use `input.options.broker` to override the native broker configuration:

```json
{ "kind": "kafka", "brokers": ["127.0.0.1:29092"], "client_id": "acme" }
```

## JavaScript SDK

```js
import { generate } from '@relevate/poolster';
import { inputAsyncApi, pluginTypeScript, pluginGo, pluginRust, pluginPython, pluginJava, pluginCSharp } from '@relevate/poolster-plugins';

await generate({
  input: {
    path: 'messages.yaml',
    plugin: inputAsyncApi(),
    broker: { kind: 'kafka', brokers: ['127.0.0.1:29092'], client_id: 'acme' },
  },
  output: 'generated',
  plugins: [pluginTypeScript(), pluginGo(), pluginRust(), pluginPython(), pluginJava(), pluginCSharp()],
});
```

Broker settings belong to the input. Event packages accept metadata and source-quality
options; GraphQL styles, HTTP transports and companion plugins do not apply here.
Models-only packages can be included at distinct output paths.

## Share models in Rust plugins

```rust
let input = InputProvider::<EventOperations>::new(registry, "asyncapi", "messages.yaml");
let shapes = poolster_core::native::events::model_blocks(Some(input.handle()), "messages.yaml");
let models = poolster::ts::shared_models(None).input_shapes(shapes.handle());
let events = poolster::ts::asyncapi(Some(input.handle())).using_models(models.handle());
let package = poolster::ts::package("client")
    .with(input).with(shapes).with(models).with(events);
```

The same composition uses `poolster::go`, `poolster::rust`, `poolster::python` or
`poolster::java` or `poolster::csharp`. Models and operations
bind to the same selected event contract. Payload/header/key roles resolve through
stable native message IDs, rather than generated names. A changed contract revision,
missing role or incompatible model provider fails generation. Keep the source identity
consistent when the event provider publishes explicit provenance.

CLI event recipes assemble this shared-model path automatically. Direct Rust event
factories also support explicit `using_models` selection. TypeScript's existing
operation type aliases and `events.ts` facade remain available.

## Boundaries

The initial transport is Kafka, with AsyncAPI 3.0/3.1 JSON messages on static topics,
one message per operation, UTF-8 string keys and supported string headers. Schema and
broker subsets are checked before generation. Security declarations, dynamic routing,
traits, replies, other brokers and unsupported schema features remain separate work.
Model support and JSON Schema runtime validation differ by target; see the
[support matrix](../../plugin-support-matrix.md).

Generated packages retain `asyncapi.json` and their model contract manifest. Broker
clients validate the supported wire schema before publishing and after receiving;
handlers control application processing. Compilation/delegate tests and actual broker
round trips are different checks, recorded in the [verification log](../../verification/verification.md).

Rust generated packages use bundled librdkafka through rust-rdkafka. Building them
requires a C toolchain and CMake in addition to Cargo. Go uses franz-go; TypeScript
uses KafkaJS. These clients currently implement the documented plaintext subset.
Go commits only after a successful handler; Rust returns received metadata and requires
explicit acknowledgement. Invalid wire messages are rejected before application decoding.

## Go package details

Go emits one `models` package with `events.go`, `kafka_runtime.go`, one
`events_operation_<operation>.go` file per operation and separate `model_<name>.go`
files. The recipe's `name` sets the Go module path in `go.mod`; it does not rename
the package. Shared models and event operations contribute to one package manifest.
Dependencies are pinned to `github.com/twmb/franz-go v1.18.1` and
`github.com/santhosh-tekuri/jsonschema/v5 v5.3.1`. Franz-go is an established
community Kafka client, rather than an official Apache Go client.

Each send method accepts a typed message and returns the produced Kafka record or
an error. The key is optional: an absent key differs from an empty UTF-8 string.
Declared key schemas must be nonnullable strings. Declared headers must be closed,
nonnullable objects whose fields are nonnullable strings; wire headers use UTF-8
values. Duplicate headers, null header values, invalid UTF-8 and tombstones are
rejected. Payload, header and key schemas use the supported draft-07 vocabulary;
external schema references and unsupported keywords are rejected. Model-shape
limits from [shared models](shared-models.md#boundaries) still apply.

Receive methods create dedicated consumer connections and require a group ID from
the operation binding or call options. A conflicting override fails. They start at
the end for a new group unless `FromBeginning` is enabled. Automatic commits are
disabled: decoding or handler errors stop consumption without committing that
record, while successful handlers are followed by a synchronous commit. Commit
errors stop consumption too. Cancellation returns the context error. This supports
at-least-once processing; handlers must tolerate redelivery, and cancelling inside a
handler can prevent its subsequent commit. There is no generated dead-letter or
invalid-message skip policy.

## Python package details

Python emits a typed installable package with shared `models/`, an `events/` client,
per-operation modules, bounded mixin/export chunks, and separate runtime/codec helpers.
Dependencies are pinned to `kafka-python==3.0.11` and `jsonschema==4.26.0`. See the
[Python capability page](../languages/python.md#kafka-events) for generated calls.

Send methods synchronously wait for broker acknowledgement. A timeout does not prove
non-delivery. Producer retries default to zero; `KafkaClient(retries=N)` opts into
native transport retries with idempotence disabled. There is no transaction or
application exactly-once guarantee.

Receive methods return context-managed subscription iterators with auto-commit disabled.
Call `record.ack()` after processing, or use `subscription.run(handler)` to commit only
after the handler succeeds. A new group starts at the earliest retained offset. Fixed
group bindings reject conflicting overrides. A `threading.Event` provides cooperative
cancellation during polling. Close subscriptions separately from their producer client.
Poll/decode failures invalidate the subscription; close/reopen instead of resuming and
accidentally committing past an invalid record.

Closed, nonnullable string headers and optional nonnullable string keys are supported.
Payload/header/key schemas use the documented Draft-07 vocabulary; schema `format` is
rejected rather than silently ignored. Shared-model schema limits still apply. Native
package tests install the generated wheel and exercise strict UTF-8, optional/null
presence, acknowledgements, redelivery, cancellation and malformed records locally.

## Java package details

Java 17 emits shared model records, per-operation `<Operation>Event` classes and separate
`KafkaRuntime`, `KafkaCodec`, `KafkaSubscription` and broker-default helpers. The recipe
name determines the Java package. The Maven manifest pins Apache Kafka client **3.9.1**,
networknt JSON Schema validator **1.5.6** and Jackson **2.18.3**. See the
[Java capability page](../languages/java.md#kafka-event-features) for generated calls.

Static send wrappers return `CompletionStage<RecordMetadata>` for broker acknowledgement.
Native producers use `acks=all` and idempotence, bounded by a 30-second delivery deadline.
That protects producer transport retries, not separate application calls or consumer
side effects; there is no generated transaction or exactly-once workflow. Cancelling a
future does not retract a record already sent. Operation client IDs select separate producers.

Subscription polling returns typed records with explicit `ack()`. Auto-commit is disabled;
acknowledge in partition order before polling another batch. `consume(timeout, handler)`
commits after successful processing. A new group starts at the earliest retained offset.
Fixed group bindings reject conflicts. Decode failures invalidate the subscription
without committing; close/reopen to retry unacknowledged records. Poll/ack/close run on
one thread; `cancel()` wakes a poll with Kafka's `WakeupException`.

The Java model subset requires closed objects and named representable nested shapes.
Payload/header/key JSON Schema constraints are validated before send and after receipt;
unsupported keywords, external references, dialects and bindings fail generation.
UTF-8, duplicate JSON fields/headers and tombstones are checked explicitly. Runtime and
subscriptions have independent ownership: close both.

## Conformance and deployment boundaries

See the [executable Kafka conformance baseline](kafka-conformance.md) for common cases, language-specific acknowledgement policies, evidence gaps, and TLS/SASL, schema registry and transaction follow-up work.

## C# package details

.NET 8 emits canonical shared models and per-operation event classes, with separate runtime, subscription and codec files. Dependencies pin **Confluent.Kafka 2.12.0** and **JsonSchema.Net 7.3.4**. Native sends return delivery metadata with idempotent producer retries; separate application calls are not deduplicated. Consumer offsets commit explicitly after processing, or after a successful handler helper. Failed decoding poisons the subscription without committing; dispose and reopen for replay. Consumer cancellation is cooperative, with polling intervals up to 250 ms. Kafka and HTTP/GraphQL require separate C# output packages. See [C# event features and generated calls](../languages/csharp.md#kafka-event-features).
