# Go output capabilities

Audited against the current checkout on **2026-10-11**. This page describes generated code, rather than what the source specifications could express. Features are contract-specific: HTTP retry behavior does not automatically apply to GraphQL or Kafka. Publication status and the overall [support matrix](../../plugin-support-matrix.md) are separate.

In the tables, **built in** means generated Poolster code, **delegated** means the named runtime/toolchain supplies the behavior, **application** means the caller must implement it, and **unsupported** means this generator does not provide it.

## Contracts

| Contract | Current output | Entry point |
| --- | --- | --- |
| HTTP / OpenAPI | Models, operations, client and resource facades | Go SDK plugin; Rust composition `go::sdk()` |
| GraphQL operations | Selected models, variables, operations and transport; optional SSE/incremental capabilities | `go::graphql(Some(input.handle()))`; CLI GraphQL profile; JS `pluginGo` |
| Shared schema models | Models-only package; event payload/header/key models | `go::shared_models(...)`; CLI `shared-models`; JS `mode: 'models'` |
| RPC / Protobuf | Poolster-owned native message bindings, gRPC clients and server interfaces | `go::grpc(module).input(input.handle())`; CLI Protobuf profile |
| Events / AsyncAPI | Kafka JSON producer/consumer subset | `go::asyncapi(Some(input.handle()))`; CLI/JS AsyncAPI profile |
| Workflows / Arazzo | Unsupported runner output | Input parsing does not create a Go workflow runner |
| Cap’n Proto capability RPC | Unsupported | No Go output pipeline |

## HTTP features

| Feature | Behavior |
| --- | --- |
| API styles | Built-in flat and namespaced resource facades. Namespaced is the idiomatic option; direct methods remain available. No HTTP `raw` style selector |
| Errors | Generated typed responses and operation errors; transport and non-success HTTP responses are distinguished |
| Retry/backoff | Built in: default three attempts, 250 ms initial delay, 8 s cap; exponential delay and `Retry-After` / `Retry-After-Ms`; configurable `RetryConfig` |
| Retry safety | Safe methods are eligible; POST/PATCH require a nonempty `Idempotency-Key` header. Do not assume a custom idempotency-header policy enables this same retry gate |
| Idempotency | Declared policy can generate/reuse a key across attempts. Server deduplication and exactly-once effects remain application/server responsibilities |
| Timeout/cancellation | Delegated to `http.Client` and operation `context.Context`; scoped call options exist |
| Auth/security | Generated declared security handling and optional OAuth token-provider machinery; TLS is delegated to the configured HTTP transport |
| Pagination/streaming | Generated helpers for recognized cursor, page, offset and URL pagination; not arbitrary query inference. HTTP request/response behavior is separate from GraphQL subscriptions and gRPC streaming |
| Validation/codecs | Generated HTTP response-validation code, typed JSON models and wire serialization. No claim of validating every possible OpenAPI extension |
| Hooks/middleware | Generated request/final-response/error hooks; retries do not produce a new logical-request hook cycle. Native/bundled Go middleware has a separate registration boundary |
| OAuth companion | Optional `go::oauth()` supplies client-credentials token acquisition/cache/refresh; it is not a generated interactive authorization-code login flow |
| Webhook companion | Optional `go::webhooks()` verifies Standard Webhooks HMAC v1 over raw bytes with timestamp tolerance/rotated keys. Persist webhook IDs for replay deduplication; Ed25519 is unsupported |

Evidence: [runtime assembly](../../../crates/plugins/go/src/runtime.rs), [retry template](../../../crates/plugins/go/templates/retry.go.tmpl), [HTTP tests](../../../crates/plugins/go/src/tests/), [native fixtures](../../../crates/plugins/go/tests/fixtures/).

Companions require explicit plugin composition/configuration; selecting a GraphQL or Kafka input does not activate HTTP OAuth/webhook generation. See [OAuth provider](../../../crates/plugins/go/src/oauth.rs), [webhook verifier](../../../crates/plugins/go/templates/webhooks.go.tmpl), and [bundled middleware](../../../crates/plugins/go/src/bundled_middleware.rs).

### Generated HTTP API styles

Assume an OpenAPI operation `getContact`, tag/resource `Contacts`, and its generated `GetContactRequest`. These are **generated API call shapes**; supply fields required by your schema.

```go
// Flat: go::sdk().flat(), or the SDK's flat style configuration.
result, err := client.GetContact(ctx, &GetContactRequest{})

// Idiomatic resource facade: namespaced configuration.
// The direct GetContact method is still present.
result, err := client.Contacts.Get(ctx, &GetContactRequest{})
```

HTTP raw style is **unsupported as a style choice**. A custom transport/composition plugin is not equivalent to emitting standalone raw operation functions. [Layout assertions](../../../crates/plugins/go/src/tests/layout.rs) verify the signatures above.

## GraphQL features

| Feature | Behavior |
| --- | --- |
| API styles | Built-in raw standalone functions, flat bound methods, idiomatic operation-kind groups; explicit group mappings also supported |
| Errors/partial results | Returns a response envelope plus `GraphQLErrors`; usable partial `Data` is preserved. HTTP, network, JSON and protocol errors remain distinct |
| Retry/backoff | No generated retry policy; application HTTP transport/wrapper responsibility. Mutations are not automatically retried |
| Idempotency | Application/server responsibility; HTTP SDK key generation is not inherited |
| Timeout/cancellation | Operation context propagates to HTTP requests; caller's `http.Client` controls timeouts |
| Auth/security | Caller-supplied client headers/HTTP transport; no GraphQL auth-flow generator |
| Pagination | Application-authored operation documents; no generic connection/page helper inferred from the schema |
| Streaming | Opt-in distinct-connection graphql-sse subscriptions; experimental multipart `deferSpec=20220824` incremental delivery. No WebSocket transport |
| Validation/codecs | Input operations validated before generation; selection-specific result structs; optional/nullable variables preserve absence/null/value. Scalar mapping and runtime callbacks supported. Go JSON decoding does not enforce presence of every required primitive result field |

Assume `query ReadUser($id: ID!) { user(id: $id) { id } }` and `mutation RenameUser(...)`; generated package alias below is `sdk`.

```go
client := sdk.NewClient(endpoint, httpClient)
variables := sdk.ReadUserVariables{ID: "42"}

// Raw: .raw(); no bound operation method required.
response, err := sdk.ReadUser(ctx, client, variables)

// Flat: .flat().
response, err := client.ReadUser(ctx, variables)

// Idiomatic: .idiomatic(); group is based on operation kind.
response, err := client.Query().ReadUser(ctx, variables)
// RenameUser is exposed through client.Mutation().RenameUser(...).

// Explicit .group("users", "read", "ReadUser") changes the group/method.
response, err := client.Users().Read(ctx, variables)
```

These are alternatives selected at generation time. Grouping by `user` is not inferred from arbitrary response nesting. See [GraphQL Go](../outputs/graphql-go.md), [advanced capability configuration](../outputs/graphql-capabilities.md), [rendered signatures](../../../crates/plugins/go/src/graphql/render.rs), and [capability tests](../../../crates/plugins/go/src/graphql/tests.rs).

## RPC features

| Feature | Behavior |
| --- | --- |
| Styles | Native grpc-go API plus selectable raw/flat/idiomatic facades |
| Errors | Delegated grpc-go status errors |
| Retry/backoff | grpc-go connection/RPC policy, configured by application; Poolster does not add its HTTP retry loop |
| Idempotency | Application/server policy; no generated idempotency-key system |
| Timeout/cancellation | Delegated operation contexts and deadlines |
| Auth/security | Application configures transport credentials and metadata |
| Streaming | Unary, client-streaming, server-streaming and bidirectional stubs/interfaces |
| Validation/codecs | Official Protobuf wire serialization, presence, oneofs, maps and field numbers from retained descriptors; no extra business-rule validator |

For a `.proto` service `Store` with unary `Get`, Poolster supplies the native usage shape:

```go
client := storepb.NewStoreClient(conn) // caller constructs grpc.ClientConn
reply, err := client.Get(ctx, &storepb.GetRequest{})
// Implement StoreServer, embedding UnimplementedStoreServer, then:
storepb.RegisterStoreServer(server, implementation)
```

Actual message names/signatures follow the `.proto`. `option go_package` or an explicit file mapping is required. Generation invokes no external generator; Protobuf editions remain unsupported. See [owned renderer](../../../crates/plugins/go/src/grpc.rs) and [native streaming integration tests](../../../crates/plugins/go/tests/grpc_native.rs).

## Events features

| Feature | Behavior |
| --- | --- |
| Styles | Typed `KafkaClient` operation methods plus generated encode/decode helpers; no raw/flat/idiomatic selector |
| Errors | Validation, wire-decoding, broker and handler errors; invalid messages stop consumption without committing |
| Retry/backoff | Broker behavior delegated to pinned franz-go; no generated application handler retry/dead-letter policy |
| Idempotency | No generated exactly-once workflow or idempotency-key abstraction; no Poolster transaction API |
| Timeout/cancellation | Publish and consume accept contexts |
| Auth/security | Documented plaintext broker subset; TLS/SASL/security schemes unsupported by this generation profile |
| Streaming/ack | Consumer loop invokes typed handler and commits after success; failures/cancellation stop the loop |
| Validation/codecs | Supported JSON Schema subset on payload, string headers and optional string key before publish/after receive; strict UTF-8; shared typed model symbols |

For AsyncAPI operations `sendUser` and `receiveUser`, the generated shape is:

```go
record, err := client.SendUser(ctx, SendUserMessage{/* typed fields */})
err = client.ReceiveUser(ctx,
    func(ctx context.Context, message ReceiveUserMessage, record *kgo.Record) error {
        return handle(message) // success permits committing this record
    }, KafkaConsumeOptions{})
```

Topic/group/client bindings come from the supported contract; provision topics separately. This is a Kafka subset, not full AsyncAPI. See the [Kafka guide](../outputs/kafka.md), [operation renderer](../../../crates/plugins/go/src/events/render.rs), [runtime](../../../crates/plugins/go/src/events/runtime.go.tmpl), and [native/broker tests](../../../crates/plugins/go/tests/events.rs).

## Generated files and model sharing

| Shared-model feature | Behavior |
| --- | --- |
| API styles, retry, idempotency, auth, timeout | Not applicable: models-only output has no client/runtime policy |
| Presence/nullability/codecs | Typed Go models and JSON serialization for supported shared schema shapes; no generated universal schema-validation runtime |
| Identity/ownership | Deterministic names and stable schema entity IDs; event composition checks selected parent contract instance/revision |
| Composition | Explicit `asyncapi(...).using_models(models.handle())`; CLI/JS Kafka profiles wire shared-model composition |

Go declarations sharing an import path intentionally live in the same directory/package. HTTP emits `client.go`, `model_*.go`, `operation_*.go`, resource and validation files. GraphQL emits `graphql_runtime.go`, `graphql_client.go`, `graphql_transport.go`, per-model/per-operation/group files and separate capability runtimes. Kafka emits model files, `events.go`, `events_operation_*.go`, `kafka_runtime.go`, retained `asyncapi.json`, schemas and package manifests. Poolster gRPC generation emits `.pb.go` / `_grpc.pb.go` by Protobuf package, plus retained sources under `proto/`.

The layout budget is 128 KiB per source file; indivisible oversized declarations can be diagnosed rather than magically split. Ownership/regeneration removes obsolete unchanged generated files and preserves unrelated files. Shared-model composition uses stable entity IDs plus contract-instance/revision provenance; it does not automatically equate unrelated same-named models. See [shared models](../outputs/shared-models.md) and [model tests](../../../crates/plugins/go/tests/shared_models.rs).

## Remaining gaps

Go workflow runners and capability RPC are absent. GraphQL dynamic typed `fields` selection is planned, not generated. Kafka broker/security and schema support remain bounded. Runtime defaults differ by contract, and package entry points cannot be assumed interchangeable: unsupported combinations follow Poolster's skipped-output diagnostics, while a supported generator rejects malformed/unsupported content. The test links above include gated native tests; an ignored test is not a pass.

## Protobuf conformance audit

The existing gRPC output is partial against the all-language target. No complete
conformance profile passes yet; see requirement-level evidence and defects in the
[Protobuf audit](../outputs/protobuf-conformance.md) and
[requirements](../../specifications/protobuf-conformance.md).
