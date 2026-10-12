# C# output capabilities

Audited against the current repository on **2026-10-12**. This describes implemented generated APIs, not the intended output specification or a promise about a previously published package. Examples show alternatives from different generation styles; do not paste all alternatives into one scope.

.NET 8, HttpClient and System.Text.Json.

## Contracts

| Contract / input | Generated output today |
| --- | --- |
| HTTP / OpenAPI | SDK, typed models and target-specific runtime helpers |
| GraphQL | Selection-specific client, variables/results and HTTP runtime; optional advanced transports |
| RPC / Protobuf | No service/client/server generator in this language; shared models are a separate subset |
| Events / AsyncAPI | Plaintext Kafka JSON producer/consumer package with shared models |
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
| Retry / backoff | SDK `PoolsterRetryOptions`: defaults to 3 total attempts, 250 ms initial delay and 8 s maximum delay; exponential backoff and bounded `retry-after-ms`/`Retry-After`. Replayable requests are cloned, with method/idempotency safety gates. Binary and SSE runtime paths do not use the JSON retry loop. |
| Idempotency | SDK declared OpenAPI idempotency policy can generate/preserve a key; retry gating uses supported headers. This does not guarantee server-side exactly-once behavior |
| Timeout / cancellation | Native HttpClient timeout; SDK `CancellationToken` is threaded through operations and backoff. Call-scoped headers/client settings are separate from GraphQL configuration. |
| Auth / security | SDK API-key/default-header configuration; credentials and transport TLS policy are application-owned. Separate OAuth helpers are opt-in, not an automatic consequence of selecting this language |
| Pagination / streaming | SDK declared cursor, page, offset/limit and absolute same-origin URL `IAsyncEnumerable` helpers with cancellation and bounds. URL helpers require buffered JSON responses and reject invalid or repeated continuation URLs. SSE returns `IAsyncEnumerable<string>`. |
| Validation / codecs | System.Text.Json model decoding and typed HTTP models; not full runtime JSON Schema validation. |
| Middleware / hooks | Application-owned transport wrapper can change execution; observer lifecycle hooks are notifications, not request replacements |

### Optional OAuth and webhook helpers

`oauth()` and `webhooks()` are separately registered HTTP companion plugins. OAuth emits client-credentials/token runtime support; selecting the main SDK does not automatically enable refresh policy or attach an OAuth transport to GraphQL. Webhooks emits a raw-body Standard Webhooks v1 signature verifier. It does not generate a hosted webhook server, subscription broker or exactly-once handler. Applications own secret rotation, routing and replay persistence.

Evidence: [OAuth emitter](../../../crates/plugins/csharp/src/oauth.rs), [webhook emitter](../../../crates/plugins/csharp/src/webhooks.rs). Native crypto/HTTP dependencies vary by target and must be present in the consuming application.

### What HTTP styles generate

Assume `GET /contacts/{id}`, `operationId: getContact`, first tag `contacts`, one required string path parameter `id`, and a typed JSON success model. Naming conflicts or extra parameters change the signature. The examples use an already constructed client.

```csharp
// Flat: operationId getContact, required string path parameter id.
var contact = await client.GetContactAsync(id);
// Namespaced (idiomatic navigation): first tag contacts.
var contact = await client.Contacts.GetContactAsync(id);
```

**Raw:** unsupported as a separate generated HTTP function surface in this plugin. Flat and namespaced are configured through the HTTP client-style entry point; GraphQL’s `raw`/`flat`/`idiomatic` setting is a different contract-specific option.

## GraphQL features

| Feature | Responsibility / current behavior |
| --- | --- |
| Raw / flat / idiomatic | SDK supports all three; idiomatic defaults to operation-kind groups; explicit groups provide resource names |
| Errors / partial results | SDK envelope retains data, errors and extensions. Explicit success/data helper rejects application errors; transport and malformed-response failures remain separate |
| Retry / backoff | No built-in GraphQL automatic retry policy. Application-owned transport policy must account for mutation replay |
| Idempotency | No inferred mutation key/deduplication behavior; requires server-specific/application policy |
| Timeout / cancellation | SDK operation `CancellationToken` plus native HttpClient timeout. Async stream enumeration cancellation and disposal close the response. |
| Auth / security | Endpoint and headers / injected transport; application-owned credentials, TLS and refresh policy |
| Pagination | No automatic connection/cursor traversal; generate the selected operation and pass cursor variables yourself |
| Subscriptions | Opt-in distinct-connection graphql-sse; not GraphQL-over-WebSocket |
| Incremental delivery | Opt-in experimental multipart `deferSpec=20220824`; not a claim of every defer/stream protocol variant |
| Selection / presence | Operation documents determine result fields; omission and explicit null remain distinct. Abstract variants require selected `__typename`; dynamic runtime field selection is unsupported |
| Custom scalars | `JsonElement`; scalar callbacks transform `JsonNode`, without changing generated field domain types. |
| Runtime validation | Required JSON field presence is checked; nonnull field values are not revalidated at runtime. Target-specific decoding/presence checks; operation validation happens in the input provider. See the [capability guide](../outputs/graphql-capabilities.md) for limits |

### What GraphQL styles generate

Assume `query ReadUser($id: ID!) { readUser(id: $id) { name nickname } }`; optional variable fields omitted from these snippets. `variables` is the generated `ReadUserVariables` value where used. Explicit grouping is configured as `groups: { users: { read: 'ReadUser' } }`; GraphQL schema fields alone do not infer that group. A mutation `RenameUser` follows the same style, with `mutation`/`Mutation` replacing the query group.

```csharp
// Raw
var response = await GraphqlOperations.ReadUserAsync(client, variables, cancellationToken);
// Flat
var response = await client.ReadUserAsync(variables, cancellationToken);
// Idiomatic: operation-kind grouping
var response = await client.Query.ReadUserAsync(variables, cancellationToken);
// Explicit group users.read -> ReadUser
var response = await client.Users.ReadAsync(variables, cancellationToken);
response.EnsureSuccess();
```

Selecting raw removes the convenience client facade; it does not remove typed variables/results or expose arbitrary runtime selections. Inspect partial data/errors before invoking the illustrated success helper when partial data is useful.

### Configuration

CLI: `poolster generate schema.graphql --input-format graphql --operation operations.graphql --language csharp --output generated` selects the pipeline. Recipes select the language output and its contract options. In the JS SDK use `pluginCSharp({ contracts: { graphql: { style: 'flat' } } })`; operation documents belong to the GraphQL input configuration. In the Rust composition SDK use `csharp::graphql(Some(input.handle())).flat()` / `.raw()` / `.idiomatic()` as appropriate. These select generated API shape, not the SDK implementation language.

## Kafka event features

.NET 8 packages pin **Confluent.Kafka 2.12.0** and **JsonSchema.Net 7.3.4**. Supported AsyncAPI 3.0/3.1 static Kafka JSON operations use shared canonical payload/header/key models; headers are closed nonnullable string objects and keys are optional nonnullable string aliases. Unsupported schema keywords, external references and bindings fail generation.

| Feature | Responsibility / current behavior |
| --- | --- |
| API shape | Per-operation static `SendOrderEvent.SendAsync(runtime, ...)` / `ReceiveOrderEvent.Subscribe(runtime)`; no universal raw/flat/idiomatic switch for events |
| Validation | Bundled Draft7 schemas validate before send and after receive; strict UTF-8/Unicode, duplicate JSON properties/headers, null headers and tombstones reject |
| Send / retry | Native `Task<DeliveryResult<byte[], byte[]>>`; producer idempotence and `acks=all`, 30 s message timeout and 10 s request timeout. No application-call deduplication or transaction promise |
| Acknowledgement | Auto-commit and auto-offset-store disabled. `record.Ack()` commits after processing; next poll requires acknowledgement of the pending record. `Run(handler)` acknowledges only after successful return |
| Handler failure | Exception propagates, pending offset remains uncommitted; dispose and reopen the same group for replay |
| Invalid incoming records | Error poisons the subscription without committing; dispose/reopen before retrying. No automatic skip or dead-letter policy |
| Cancellation | `CancellationToken` for send waits and consumer polling; consumer checks at intervals up to 250 ms. Send cancellation does not retract queued publication |
| Bindings / lifetime | Fixed group override conflicts reject. Producers cache per operation client ID; consumers retain their operation ID. Dispose subscriptions separately from producer runtime; poll/ack/dispose stay on one thread |
| Security / registry / transactions | Generated TLS/SASL, schema registry and transactions unsupported |

### Generated calls

For the maintained `OrderCreated` fixture, generated classes include `OrderCreatedPayload`, `OrderCreatedHeaders`, `OrderCreatedKey`, `SendOrderEvent` and `ReceiveOrderEvent`. Namespace comes from package configuration.

```csharp
var payload = new OrderCreatedPayload { Id = "order-1", Quantity = 1 };
var headers = new OrderCreatedHeaders { Trace = "trace-1" };
var key = new OrderCreatedKey("account-1");
using var runtime = KafkaDefaults.Connect();
var delivery = await SendOrderEvent.SendAsync(runtime, payload, headers, key);
using var subscription = ReceiveOrderEvent.Subscribe(runtime);
var record = subscription.Poll(TimeSpan.FromSeconds(5), cancellationToken);
if (record is not null) {
    Process(record.Payload);
    record.Ack();
}
```

Select CLI/recipe language `csharp` with plugin `asyncapi`, or JS `pluginCSharp()` with AsyncAPI input. Rust composition uses `csharp::asyncapi(Some(input.handle())).using_models(models.handle())`. Files split into `Models/*.cs`, `Events/<Operation>Event.cs`, `Events/KafkaRuntime.cs`, `Events/KafkaSubscription.cs`, `Events/KafkaCodec.cs` and retained `asyncapi.json`; schemas are embedded in operation definitions, under one package `.csproj`. A 250-operation layout fixture checks splitting; operation source above the 128 KiB budget rejects before emission.

Kafka and HTTP/GraphQL generation require separate C# output packages; mixed-package generation rejects rather than merging incompatible workspace assemblies.

The explicitly enabled local-broker test compiles and executes the generated package, checks delivery metadata, operation IDs, fixed groups, exact failed-handler offset replay, duplicate acknowledgement, cross-thread cancellation, and nine invalid wire inputs. See [the conformance baseline](../outputs/kafka-conformance.md) for case evidence and remaining cross-language test gaps.

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

HTTP: `.csproj`, runtime/client sources, models and Resources declaration parts. GraphQL: separate runtime, client, operations, models and groups; partial declarations retain public model/client names.

Sources are split at operation/model/runtime boundaries. GraphQL layout targets 128 KiB per source; indivisible declarations exceeding the budget produce `.poolster/source-layout-diagnostics.json` rather than incompatible silent splitting. Regeneration removes obsolete unchanged owned files and preserves unrelated user files. Customizations tied to previous monolithic paths need retargeting.

The removed `dotnet` plugin is not a separate supported language plugin. Use `csharp`; older GraphQL guides mentioning a dotnet alias are historical.

## Gaps and evidence

RPC, workflows and capabilities still need language-specific runnable packages. GraphQL retry/idempotency policy, automatic pagination and dynamic typed selections are application work or future features, not enabled by the HTTP runtime. Shared-model support does not imply runtime schema-validation parity.

Kafka generated compilation and local-broker execution were explicitly run on 12 October 2026; HTTP/GraphQL evidence remains linked below. Native generated-code tests can be ignored by the ordinary workspace suite until their toolchains are explicitly enabled.

- [HTTP implementation](../../../crates/plugins/csharp/src/http/)
- [HTTP operation tests](../../../crates/plugins/csharp/src/operation_tests.rs)
- [GraphQL implementation](../../../crates/plugins/csharp/src/graphql/)
- [Generated GraphQL compilation/runtime tests](../../../crates/plugins/csharp/src/graphql/tests.rs)
- [Shared-model implementation and tests](../../../crates/plugins/csharp/src/shared_models.rs)
- [GraphQL usage and layout guide](../outputs/graphql-csharp.md)

- [Kafka event implementation](../../../crates/plugins/csharp/src/events/)
- [Kafka generation and native tests](../../../crates/plugins/csharp/tests/events.rs)
- [Kafka runtime probe](../../../crates/plugins/csharp/tests/fixtures/events-wire.cs)

## Protobuf conformance audit

Protobuf output is not implemented in this language. All 29 requirements remain
unsupported; the rollout and per-language assessments are recorded in the
[Protobuf audit](../outputs/protobuf-conformance.md) and
[requirements](../../specifications/protobuf-conformance.md).
