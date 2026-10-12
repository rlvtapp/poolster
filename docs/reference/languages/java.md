# Java output capabilities

Audited against the current repository on **2026-10-12**. This describes implemented generated APIs, not the intended output specification or a promise about a previously published package. Examples show alternatives from different generation styles; do not paste all alternatives into one scope.

Java 17+, JDK HttpClient and Jackson 2.18.3; Maven and Gradle manifests.

## Contracts

| Contract / input | Generated output today |
| --- | --- |
| HTTP / OpenAPI | SDK, typed models and target-specific runtime helpers |
| GraphQL | Selection-specific client, variables/results and HTTP runtime; optional advanced transports |
| RPC / Protobuf | No service/client/server generator in this language; shared models are a separate subset |
| Events / AsyncAPI | Kafka JSON typed producers/consumers, shared payload/header/key records and runtime JSON Schema validation; documented subset |
| Workflows / Arazzo | No workflow runner |
| Capabilities / Cap’n Proto | No message/capability RPC generator |
| Shared models | Neutral model package; supported schema subset and explicit rejection of unsupported shapes |

See the [cross-language support matrix](../../plugin-support-matrix.md) for the exact shared-model subset. An input parser being available does not imply this language exports its protocol runtime.

## HTTP features

| Feature | Responsibility / current behavior |
| --- | --- |
| Raw | No separately emitted standalone-function HTTP API style; use native client operations or the injected transport |
| Flat | SDK direct operation methods |
| Idiomatic | SDK namespaced resource facade (`SdkClientStyle::Namespaced`); direct methods remain available |
| Errors / partial results | SDK HTTP status errors and native transport/decoding failures; no GraphQL-style partial-result envelope |
| Retry / backoff | SDK `RetryConfig`: defaults to 3 total attempts, 250 ms initial delay and 8 s maximum delay; eligible transport/status failures, exponential delay and server retry headers. Replay is gated by method/key safety. |
| Idempotency | SDK declared OpenAPI idempotency policy can generate/preserve a key; retry gating uses supported headers. This does not guarantee server-side exactly-once behavior |
| Timeout / cancellation | SDK request timeout and `ClientCallOptions`/`forCall`; native thread interruption is preserved. No C#-style per-method cancellation token. |
| Auth / security | SDK API-key/default-header configuration; credentials and transport TLS policy are application-owned. Separate OAuth helpers are opt-in, not an automatic consequence of selecting this language |
| Pagination / streaming | SDK lazy declared cursor, page, offset/limit and same-origin URL helpers; target-specific parameter restrictions. HTTP SSE is a separate streaming surface. |
| Validation / codecs | Jackson model decoding and generated typed models; not a general runtime JSON Schema validator. |
| Middleware / hooks | Application-owned transport wrapper can change execution; observer lifecycle hooks are notifications, not request replacements |

### Optional OAuth and webhook helpers

`oauth()` and `webhooks()` are separately registered HTTP companion plugins. OAuth emits client-credentials/token runtime support; selecting the main SDK does not automatically enable refresh policy or attach an OAuth transport to GraphQL. Webhooks emits a raw-body Standard Webhooks v1 signature verifier. It does not generate a hosted webhook server, subscription broker or exactly-once handler. Applications own secret rotation, routing and replay persistence.

Evidence: [OAuth emitter](../../../crates/plugins/java/src/oauth.rs), [webhook emitter](../../../crates/plugins/java/src/webhooks.rs). Native crypto/HTTP dependencies vary by target and must be present in the consuming application.

### What HTTP styles generate

Assume `GET /contacts/{id}`, `operationId: getContact`, first tag `contacts`, one required string path parameter `id`, and a typed JSON success model. Naming conflicts or extra parameters change the signature. The examples use an already constructed client.

```java
// Flat: operationId getContact, required string path parameter id.
var contact = client.getContact(new Client.GetContactRequest(id));
// Namespaced (idiomatic navigation): first tag contacts.
var contact = client.contacts().get(new Client.GetContactRequest(id));
```

**Raw:** unsupported as a separate generated HTTP function surface in this plugin. Flat and namespaced are configured through the HTTP client-style entry point; GraphQL’s `raw`/`flat`/`idiomatic` setting is a different contract-specific option.

## GraphQL features

| Feature | Responsibility / current behavior |
| --- | --- |
| Raw / flat / idiomatic | SDK supports all three; idiomatic defaults to operation-kind groups; explicit groups provide resource names |
| Errors / partial results | SDK envelope retains data, errors and extensions. Explicit success/data helper rejects application errors; transport and malformed-response failures remain separate |
| Retry / backoff | No built-in GraphQL automatic retry policy. Application-owned transport policy must account for mutation replay |
| Idempotency | No inferred mutation key/deduplication behavior; requires server-specific/application policy |
| Timeout / cancellation | Native JDK HTTP configuration and interruption; close subscription/incremental `Stream` with try-with-resources. |
| Auth / security | Endpoint and headers / injected transport; application-owned credentials, TLS and refresh policy |
| Pagination | No automatic connection/cursor traversal; generate the selected operation and pass cursor variables yourself |
| Subscriptions | Opt-in distinct-connection graphql-sse; not GraphQL-over-WebSocket |
| Incremental delivery | Opt-in experimental multipart `deferSpec=20220824`; not a claim of every defer/stream protocol variant |
| Selection / presence | Operation documents determine result fields; omission and explicit null remain distinct. Abstract variants require selected `__typename`; dynamic runtime field selection is unsupported |
| Custom scalars | `JsonNode`; callbacks stay within the JSON-value domain. |
| Runtime validation | Target-specific decoding/presence checks; operation validation happens in the input provider. See the [capability guide](../outputs/graphql-capabilities.md) for limits |

### What GraphQL styles generate

Assume `query ReadUser($id: ID!) { readUser(id: $id) { name nickname } }`; optional variable fields omitted from these snippets. `variables` is the generated `ReadUserVariables` value where used. Explicit grouping is configured as `groups: { users: { read: 'ReadUser' } }`; GraphQL schema fields alone do not infer that group. A mutation `RenameUser` follows the same style, with `mutation`/`Mutation` replacing the query group.

```java
// Raw
var response = Client.readUser(client, new ReadUserVariables("1"));
// Flat
var response = client.readUser(new ReadUserVariables("1"));
// Idiomatic: operation-kind grouping
var response = client.query().readUser(new ReadUserVariables("1"));
// Explicit group users.read -> ReadUser
var response = client.users().read(new ReadUserVariables("1"));
var data = response.requireData();
```

Selecting raw removes the convenience client facade; it does not remove typed variables/results or expose arbitrary runtime selections. Inspect partial data/errors before invoking the illustrated success helper when partial data is useful.

### Configuration

CLI: `poolster generate schema.graphql --input-format graphql --operation operations.graphql --language java --output generated` selects the pipeline. Recipes select the language output and its contract options. In the JS SDK use `pluginJava({ contracts: { graphql: { style: 'flat' } } })`; operation documents belong to the GraphQL input configuration. In the Rust composition SDK use `java::graphql(Some(input.handle())).flat()` / `.raw()` / `.idiomatic()` as appropriate. These select generated API shape, not the SDK implementation language.

## Kafka event features

| Feature | Current behavior |
| --- | --- |
| Typed API | One `<Operation>Event` class per send/receive operation; shared `<Message>Payload`, `Headers` and `Key` records |
| Raw / flat / idiomatic | No configurable event styles yet. Operation classes expose static typed `send`/`subscribe`; `KafkaCodec.encode/decode` is the strict wire boundary for custom transports |
| Retry / backoff | Official Kafka producer transport retries are bounded by a 30-second delivery deadline. Poolster never replays a separate application send or failed handler automatically |
| Idempotency | Native producer `enable.idempotence=true`, `acks=all`; protection for producer transport retries only. Separate application sends and handler side effects can duplicate; no transactions/exactly-once claim |
| Acknowledgement | Auto commit off; `record.ack()` commits that record after success, in partition order. `consume(timeout, handler)` acks after the handler returns; exceptions leave failed/following records uncommitted |
| Timeout / cancellation | Producer delivery/request/max-block limits are explicit. Consumer `poll(Duration)` bounds waiting. `cancel()` wakes a poll with Kafka `WakeupException`; close on the consumer thread. Cancelling a send future does not retract a sent record |
| Errors | Schema/UTF-8/JSON decode errors throw `IllegalArgumentException`; send transport failures complete the future exceptionally; consumer transport failures use Kafka exceptions. Decode failures invalidate the subscription without committing |
| Auth / security | Plaintext seed brokers only; declared security, TLS/SASL and unsupported Kafka bindings fail generation |
| Bindings | Static topic, per-operation client ID and fixed receive group ID are honored. Conflicting group overrides reject; an unbound group can be supplied explicitly |
| Validation / encoding | networknt 1.5.6 validates the bounded Draft-07 schema subset before send/after receipt. Jackson 2.18.3 preserves supported presence/nullability. Strict UTF-8, duplicate JSON property/header, tombstone and invalid key/header rejection |
| Supported shapes | Java shared-model restrictions still apply: no nullable named roots, inline objects/enums, unions/intersections or additional-properties objects. Header objects are closed and contain nonnullable strings; keys are optional nonnullable strings. `format` keywords and external schema references reject. Embedded schema strings are capped at 60,000 modified UTF-8 bytes to respect JVM constant limits |
| Infrastructure | Official Kafka client 3.9.1; Maven manifest. Provision topics separately. No schema registry, transactions, dead-letter/retry policy or alternate brokers |

### Generated Kafka example

For the checked `sendOrder`/`receiveOrder` fixture, default package `io.poolster.models`:

```java
import io.poolster.models.events.*;
import io.poolster.models.model.*;
import java.time.Duration;

// Optional note omitted; use Presence.of(null) to send an explicit null.
var payload = new OrderCreatedPayload("order-1", 1L, null);
var headers = new OrderCreatedHeaders("trace-1");
var key = new OrderCreatedKey("account-1");
try (var runtime = KafkaDefaults.connect()) {
  var metadata = SendOrderEvent.send(runtime, payload, headers, key)
      .toCompletableFuture().get();
  try (var subscription = ReceiveOrderEvent.subscribe(runtime)) {
    for (var event : subscription.poll(Duration.ofSeconds(1))) {
      process(event.payload());
      event.ack(); // Only after application success.
    }
  }
}
```

`process` is application code. Runtime and subscriptions have independent lifetimes.
Polling/acknowledgement are single-threaded. Do not poll another batch until pending
records are acknowledged; close/reopen the same group to retry unacknowledged records.
Use `KafkaCodec.decode(SendOrderEvent.DEFINITION, consumerRecord)` to reuse strict
models/validation with an application-owned transport rather than re-parsing schemas.

Composition in the Rust SDK is
`java::asyncapi(Some(input.handle())).using_models(models.handle())`, with
`native::events::model_blocks` feeding `java::shared_models(None).input_shapes(...)`.
Standalone `asyncapi` derives the same shared-model plan internally. CLI/JS SDK
composition uses that contract chain automatically; see the [Kafka guide](../outputs/kafka.md).

Evidence: [generator/runtime](../../../crates/plugins/java/src/events/),
[provider/regeneration and generated compiler/broker tests](../../../crates/plugins/java/tests/events.rs).
Pinned upstream contracts: [Kafka producer](https://kafka.apache.org/39/javadoc/org/apache/kafka/clients/producer/KafkaProducer.html),
[Kafka consumer](https://kafka.apache.org/39/javadoc/org/apache/kafka/clients/consumer/KafkaConsumer.html),
[networknt 1.5.6](https://github.com/networknt/json-schema-validator/tree/1.5.6).

## Shared-model contract features

| Feature | Current behavior |
| --- | --- |
| API styles | Model declarations only; raw/flat/idiomatic client styles do not apply |
| Naming / provenance | Shared model plan resolves stable entity names; emitted model manifest retains parent contract/revision |
| Presence / nullability | Supported neutral schema shapes preserve the documented target subset; this does not make every protocol wire encoding interchangeable |
| Validation / codecs | Target-native model encoding/decoding; unsupported schema constructs reject generation. No protocol transport or universal validator is implied |
| Retry / idempotency / auth / timeout | Not applicable to a model-only package |
| Broker/RPC behavior | Not emitted merely because AsyncAPI or Protobuf supplied the models |

See [shared-model output](../outputs/shared-models.md) and the matrix for per-language enum/alias/additional-property restrictions.

## Files, naming and regeneration

HTTP: Java client/config/runtime, model package and resource sources. GraphQL: `src/main/java/<package>/GraphqlRuntime.java`, `Client.java`, `models/`, `operations/`, `groups/`; records are imported from `<package>.models`, not nested in Client.

Sources are split at operation/model/runtime boundaries. GraphQL layout targets 128 KiB per source; indivisible declarations exceeding the budget produce `.poolster/source-layout-diagnostics.json` rather than incompatible silent splitting. Regeneration removes obsolete unchanged owned files and preserves unrelated user files. Customizations tied to previous monolithic paths need retargeting.

Java HTTP inputs use generated request objects; GraphQL uses operation-specific variable records. This differs from native languages with positional HTTP parameters.

## Gaps and evidence

RPC, workflows and capabilities still need language-specific runnable packages. Kafka has a runnable documented subset; broader AsyncAPI broker/security/transaction features remain unsupported. GraphQL retry/idempotency policy, automatic pagination and dynamic typed selections are application work or future features, not enabled by the HTTP runtime. Shared-model support does not imply runtime schema-validation parity.

Kafka generated Java compilation, strict wire checks and a local pinned Kafka broker round trip were explicitly run for this batch. Native generated-code tests can be ignored by the ordinary workspace suite until their toolchains are explicitly enabled.

- [HTTP implementation](../../../crates/plugins/java/src/http/)
- [HTTP operation tests](../../../crates/plugins/java/src/operation_tests.rs)
- [GraphQL implementation](../../../crates/plugins/java/src/graphql/)
- [Generated GraphQL compilation/runtime tests](../../../crates/plugins/java/src/graphql/tests.rs)
- [Shared-model implementation and tests](../../../crates/plugins/java/src/shared_models.rs)
- [GraphQL usage and layout guide](../outputs/graphql-java.md)
