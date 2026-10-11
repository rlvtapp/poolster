# Rust output capabilities

Audited against the current checkout on **2026-10-11**. These are implemented generator capabilities, not a promise that every feature of each source format is supported. Publication status and the overall [support matrix](../../plugin-support-matrix.md) are separate. HTTP behavior is not automatically inherited by GraphQL or Kafka.

**Built in** means emitted Poolster code; **delegated** means a named native runtime supplies the behavior; **application** means caller/server responsibility; **unsupported** means no corresponding generator capability.

## Contracts

| Contract | Current output | Entry point |
| --- | --- | --- |
| HTTP / OpenAPI | Models, typed operations, Reqwest client and resource facades | Rust SDK plugin; Rust composition `rust::sdk()` |
| GraphQL operations | Selected models/variables, operations, client and transport; optional SSE/incremental capabilities | `rust::graphql(Some(input.handle()))`; CLI GraphQL profile; JS `pluginRust` |
| Shared schema models | Serde models-only crate and Kafka payload/header/key models | `rust::shared_models(...)`; CLI `shared-models`; JS `mode: 'models'` |
| Events / AsyncAPI | Kafka JSON producer/consumer subset | `rust::asyncapi(Some(input.handle()))`; CLI/JS AsyncAPI profile |
| RPC / Protobuf | Unsupported Rust client/server output | Parsed RPC input is not a Rust SDK |
| Workflows / Arazzo | Unsupported Rust runner | No runner output pipeline |
| Cap’n Proto capability RPC | Planned; unsupported output | Retained input/metadata does not constitute official-toolchain Rust generation |

## HTTP features

| Feature | Behavior |
| --- | --- |
| API styles | Flat methods and idiomatic namespaced resource accessors; namespaced default. Direct methods remain available. No HTTP `raw` style selector |
| Errors | Per-operation typed error enums distinguish transport and HTTP failures, retaining status/headers/body on HTTP errors |
| Retry/backoff | Built in: default three attempts, 250 ms initial delay, 8 s maximum; exponential delay and `Retry-After` / `Retry-After-Ms`. `with_retry(RetryConfig)` controls policy |
| Retry safety | Safe methods eligible; POST/PATCH/custom methods require a declared, supplied nonempty idempotency header |
| Idempotency | Resolved policy can auto-generate a stable key reused across attempts; server-side deduplication remains application/server responsibility |
| Timeout/cancellation | Reqwest/Tokio runtime; scoped `CallOptions.timeout` applies per attempt. Dropping the request future cancels that caller's work |
| Auth/security | Declared bearer/security handling and optional token-provider/OAuth support; transport TLS follows Reqwest configuration |
| Pagination/streaming | Recognized pagination patterns emit lazy page streams; no generic inference for every custom paging format |
| Validation/codecs | Serde wire models and typed request/response decoding. Do not equate Serde decoding with complete JSON Schema validation |
| Hooks | Request, final response and error hooks; transport middleware can run per attempt |
| OAuth companion | Optional `rust::oauth()` emits client-credentials token acquisition/cache/refresh; no generated interactive authorization-code login flow |
| Webhook companion | Optional `rust::webhooks()` verifies Standard Webhooks HMAC v1 over raw bytes with timestamp tolerance/rotated keys; application persists IDs for replay deduplication. Ed25519 unsupported |

Evidence: [HTTP orchestration](../../../crates/plugins/rust/src/http.rs), [retry/hooks runtime](../../../crates/plugins/rust/src/render_client_runtime.rs), [retry safety](../../../crates/plugins/rust/src/render_responses.rs), [operation emission](../../../crates/plugins/rust/src/render_operations.rs), [idempotency tests](../../../crates/plugins/rust/src/render_tests.rs).

Companions require explicit composition/configuration. HTTP companion support does not imply those plugins consume GraphQL/events contracts. See [OAuth template](../../../crates/plugins/rust/templates/oauth.rs.tmpl), [webhook template](../../../crates/plugins/rust/templates/webhooks.rs.tmpl) and [transport middleware contract](../../../crates/plugins/rust/src/composition.rs).

### Generated HTTP API styles

Assume an operation `getContact`, a `Contacts` resource and generated request parameters. Actual request fields/body arguments follow your schema.

```rust
// Flat: rust::sdk().flat().
let contact = client.get_contact(request).await?;

// Idiomatic: rust::sdk().namespaced() (default).
let contact = client.contacts().get(request).await?;
// Direct client.get_contact(...) remains available.
```

HTTP raw is **unsupported as a style selector**. Composing custom model/transport/operation plugins is a separate extension mechanism. Resource methods delegate to the same direct operation implementation, rather than creating a different wire request. See [resource renderer](../../../crates/plugins/rust/src/render_resources.rs).

## GraphQL features

| Feature | Behavior |
| --- | --- |
| API styles | Raw free functions, flat bound methods, idiomatic operation-kind groups and explicit group mappings |
| Errors/partial results | `GraphqlResponse::{Success, Partial, Error}` preserves usable data and application errors; transport/HTTP/protocol/decode failures use a separate error type |
| Retry/backoff | No generated retry loop; application wrapper responsibility. Mutations are not automatically replayed |
| Idempotency | Application/server policy; HTTP SDK key generation is not inherited |
| Timeout/cancellation | Caller configures Reqwest timeout/headers; dropping the operation future cancels caller work |
| Auth/security | Caller-supplied HTTP client/transport configuration; no generated GraphQL auth flow |
| Pagination | Operation document responsibility; no generic Relay/page helper inferred |
| Streaming | Opt-in distinct-connection graphql-sse; experimental multipart `deferSpec=20220824`; no WebSocket transport |
| Validation/codecs | Operations validated before generation, result types contain selected fields; omission/null/value wrappers retained. Built-in scalar mappings and direction-specific runtime callbacks; required nullable results must be present |

Assume `query ReadUser($id: ID!) { user(id: $id) { id } }` and a mutation named `RenameUser`. `sdk::graphql::*` below refers to the generated crate's public GraphQL API.

```rust
use sdk::graphql::*;
let http = reqwest::Client::new();
let transport = GraphqlHttpTransport::new(endpoint, http.clone());
let variables = ReadUserVariables { id: "42".into() };

// Raw: .raw(); free operation functions also survive in other styles.
let response = read_user(&transport, &variables).await?;

// Flat: .flat().
let client = Client::new(endpoint, http);
let response = client.read_user(&variables).await?;

// Idiomatic: .idiomatic() / .namespaced() (default).
let response = client.query().read_user(&variables).await?;
// RenameUser appears through client.mutation().rename_user(...).

// With explicit .group("user", "read", "ReadUser"):
let response = client.user().read(&variables).await?;
```

These snippets show alternative generated styles. No-variable bound operations omit the variables argument; raw free functions still receive the generated empty variables struct. Grouping by resource is explicit rather than inferred from nested GraphQL selections. See [GraphQL Rust configuration](../outputs/graphql-rust.md), [advanced capabilities](../outputs/graphql-capabilities.md), [style renderer](../../../crates/plugins/rust/src/graphql/styles.rs), and [compiled runtime consumer](../../../crates/plugins/rust/tests/fixtures/graphql_native/consumer.rs).

## Events features

| Feature | Behavior |
| --- | --- |
| API styles | Typed client send methods, subscription receive methods and generic free operations over `Publisher`/`Subscriber`; no raw/flat/idiomatic selector |
| Errors | Schema/codec/broker failures are explicit generated errors; validation occurs before publish and after receive |
| Retry/backoff | Native broker behavior delegated to rdkafka/librdkafka; no generated application handler retry/dead-letter policy |
| Idempotency | No Poolster transaction/exactly-once/idempotency-key API |
| Timeout/cancellation | Async caller/runtime lifetime; no shared HTTP retry/timeout policy applied to events |
| Auth/security | Supported plaintext profile; generated TLS/SASL/security configuration unsupported |
| Streaming/ack | `Subscription` receives typed records with metadata; explicit acknowledgement after successful application processing |
| Validation/codecs | Supported JSON Schema subset validates JSON payload, string headers and optional string key; strict UTF-8. Serde models preserve supported null/presence semantics |

For AsyncAPI operations `sendUser` and `receiveUser`, generated usage has this shape:

```rust
// `client` is a configured generated KafkaClient.
let delivery = client.send_user(&message).await?; // SendUserMessage
let subscription = client.subscribe_receive_user()?;
let received = subscription.receive_user().await?;
// Process its typed message and metadata, then acknowledge explicitly.
// Generic send_user(&publisher, &message) is also generated.
```

Topic/group/client bindings are retained for the supported subset. Topics must be provisioned separately. The package pins rdkafka with bundled librdkafka, requiring a C toolchain and CMake when building; this is not a pure-Rust Kafka implementation. Transactions, schema registry, tombstones and application retry workflows are not supplied. See [Kafka guide](../outputs/kafka.md), [operation renderer](../../../crates/plugins/rust/src/events/render.rs), [runtime](../../../crates/plugins/rust/src/events/runtime/), and [native/broker tests](../../../crates/plugins/rust/src/events/tests.rs).

## Generated files and model sharing

| Shared-model feature | Behavior |
| --- | --- |
| API styles, retry, idempotency, timeout, auth | Not applicable: models-only output does not run requests |
| Presence/nullability/codecs | Serde models for supported shared schema shapes; no universal business/schema validator automatically attached to model-only crates |
| Identity/ownership | Deterministic names/stable entity IDs; event consumers require matching selected contract-instance/revision provenance |
| Composition | Explicit `asyncapi(...).using_models(models.handle())`; CLI/JS Kafka profiles wire shared-model composition |

HTTP uses `src/models/`, split client/operation/resource modules, public `src/lib.rs`, `Cargo.toml` and a style guide. GraphQL preserves public `src/graphql.rs` while placing declarations under `src/graphql/models/`, operations under `operations/`, facade methods under `client/`, and transport under `src/graphql_runtime.rs` with separate capability files. Kafka emits `src/models/`, `src/events/operations/`, schema JSON, split runtime modules, bounded export indexes, retained `asyncapi.json`, and crate manifests.

Source layout targets 128 KiB files at semantic boundaries; indivisible oversize declarations receive diagnostics. Regeneration removes obsolete unchanged owned files and preserves unrelated files. Shared models bind by stable entity ID and parent contract-instance/revision, not merely a name match; `using_models(...)` supplies the selected model output. Native HTTP/GraphQL/event package assembly combinations have restrictions: use separate packages rather than assuming every runtime composes in one crate.

See [shared model guide](../outputs/shared-models.md), [shared-model native tests](../../../crates/plugins/rust/tests/shared_models.rs), [HTTP source-layout checks](../../../crates/plugins/rust/src/source_layout_tests.rs), and [GraphQL native tests](../../../crates/plugins/rust/tests/graphql_native.rs).

## Remaining gaps

Protobuf RPC, Arazzo runners and Cap’n Proto capability RPC have no Rust output pipeline. Dynamic typed GraphQL `fields` selection remains planned. Kafka security/other brokers and unsupported JSON Schema features require further work. Runtime features differ by contract and are not inferred from a language's HTTP support. Test links include gated native tests; ignored tests must not be counted as passes.
