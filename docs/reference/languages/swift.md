# Swift output capabilities

Audited against the current repository on **2026-10-11**. This describes implemented generated APIs, not the intended output specification or a promise about a previously published package. Examples show alternatives from different generation styles; do not paste all alternatives into one scope.

Swift 5.9+ / Swift Package Manager; Foundation URLSession, async/await and Codable. GraphQL packages target macOS 12+ and iOS 15+.

## Contracts

| Contract / input | Generated output today |
| --- | --- |
| HTTP / OpenAPI | SDK, typed models and target-specific runtime helpers |
| GraphQL | Selection-specific client, variables/results and HTTP runtime; optional advanced transports |
| RPC / Protobuf | No service/client/server generator in this language; shared models are a separate subset |
| Events / AsyncAPI | Shared model subset only; no producer/consumer runtime in this language |
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
| Retry / backoff | SDK opt-in: `maxAttempts` defaults to 1 (at most 10), delays bounded to 60 s. Exponential backoff and server retry headers; cancellation and decoding failures are not retried. POST/PATCH require a nonblank standard or declared idempotency key. |
| Idempotency | SDK declared OpenAPI idempotency policy can generate/preserve a key; retry gating uses supported headers. This does not guarantee server-side exactly-once behavior |
| Timeout / cancellation | SDK options/`forCall` request timeout; native Task cancellation through URLSession, including SDK backoff. |
| Auth / security | SDK API-key/default-header configuration; credentials and transport TLS policy are application-owned. Separate OAuth helpers are opt-in, not an automatic consequence of selecting this language |
| Pagination / streaming | SDK lazy declared cursor, page, offset/limit and same-origin URL AsyncSequences; supported controls vary. SSE has a bounded lazy event sequence and no automatic reconnect. |
| Validation / codecs | Codable decoding; generated JSON models preserve their supported nullability subset. This is not a universal JSON Schema validator. |
| Middleware / hooks | Application-owned transport wrapper can change execution; observer lifecycle hooks are notifications, not request replacements |

### Optional OAuth and webhook helpers

`oauth()` and `webhooks()` are separately registered HTTP companion plugins. OAuth emits client-credentials/token runtime support; selecting the main SDK does not automatically enable refresh policy or attach an OAuth transport to GraphQL. Webhooks emits a raw-body Standard Webhooks v1 signature verifier. It does not generate a hosted webhook server, subscription broker or exactly-once handler. Applications own secret rotation, routing and replay persistence.

Evidence: [OAuth emitter](../../../crates/plugins/swift/src/oauth.rs), [webhook emitter](../../../crates/plugins/swift/src/webhooks.rs). Native crypto/HTTP dependencies vary by target and must be present in the consuming application.

### What HTTP styles generate

Assume `GET /contacts/{id}`, `operationId: getContact`, first tag `contacts`, one required string path parameter `id`, and a typed JSON success model. Naming conflicts or extra parameters change the signature. The examples use an already constructed client.

```swift
// Flat: operationId getContact, required string path parameter id.
let contact = try await client.getContact(id: id)
// Namespaced (idiomatic navigation): first tag contacts.
let contact = try await client.contacts.getContact(id: id)
```

**Raw:** unsupported as a separate generated HTTP function surface in this plugin. Flat and namespaced are configured through the HTTP client-style entry point; GraphQL’s `raw`/`flat`/`idiomatic` setting is a different contract-specific option.

## GraphQL features

| Feature | Responsibility / current behavior |
| --- | --- |
| Raw / flat / idiomatic | SDK supports all three; idiomatic defaults to operation-kind groups; explicit groups provide resource names |
| Errors / partial results | SDK envelope retains data, errors and extensions. Explicit success/data helper rejects application errors; transport and malformed-response failures remain separate |
| Retry / backoff | No built-in GraphQL automatic retry policy. Application-owned transport policy must account for mutation replay |
| Idempotency | No inferred mutation key/deduplication behavior; requires server-specific/application policy |
| Timeout / cancellation | Native Task cancellation through URLSession; streaming lifetime is separate from buffered HTTP. |
| Auth / security | Endpoint and headers / injected transport; application-owned credentials, TLS and refresh policy |
| Pagination | No automatic connection/cursor traversal; generate the selected operation and pass cursor variables yourself |
| Subscriptions | Opt-in distinct-connection graphql-sse; not GraphQL-over-WebSocket |
| Incremental delivery | Opt-in experimental multipart `deferSpec=20220824`; not a claim of every defer/stream protocol variant |
| Selection / presence | Operation documents determine result fields; omission and explicit null remain distinct. Abstract variants require selected `__typename`; dynamic runtime field selection is unsupported |
| Custom scalars | `GraphqlJSON`; callbacks preserve JSON-value domain types. |
| Runtime validation | Target-specific decoding/presence checks; operation validation happens in the input provider. See the [capability guide](../outputs/graphql-capabilities.md) for limits |

### What GraphQL styles generate

Assume `query ReadUser($id: ID!) { readUser(id: $id) { name nickname } }`; optional variable fields omitted from these snippets. `variables` is the generated `ReadUserVariables` value where used. Explicit grouping is configured as `groups: { users: { read: 'ReadUser' } }`; GraphQL schema fields alone do not infer that group. A mutation `RenameUser` follows the same style, with `mutation`/`Mutation` replacing the query group.

```swift
// Raw
let response = try await readUser(client: client, variables: variables)
// Flat
let response = try await client.readUser(variables: variables)
// Idiomatic: operation-kind grouping
let response = try await client.query.readUser(variables: variables)
// Explicit group users.read -> ReadUser
let response = try await client.users.read(variables: variables)
try response.ensureSuccess()
```

Selecting raw removes the convenience client facade; it does not remove typed variables/results or expose arbitrary runtime selections. Inspect partial data/errors before invoking the illustrated success helper when partial data is useful.

### Configuration

CLI: `poolster generate schema.graphql --input-format graphql --operation operations.graphql --language swift --output generated` selects the pipeline. Recipes select the language output and its contract options. In the JS SDK use `pluginSwift({ contracts: { graphql: { style: 'flat' } } })`; operation documents belong to the GraphQL input configuration. In the Rust composition SDK use `swift::graphql(Some(input.handle())).flat()` / `.raw()` / `.idiomatic()` as appropriate. These select generated API shape, not the SDK implementation language.

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

HTTP Swift package: client/runtime, model, operation and resource files. GraphQL: `Sources/<module>/Runtime/`, `Client/`, `Operations/`, `Models/`, `Groups/`; SwiftPM discovers the sources.

Sources are split at operation/model/runtime boundaries. GraphQL layout targets 128 KiB per source; indivisible declarations exceeding the budget produce `.poolster/source-layout-diagnostics.json` rather than incompatible silent splitting. Regeneration removes obsolete unchanged owned files and preserves unrelated user files. Customizations tied to previous monolithic paths need retargeting.

HTTP resource methods retain full operation names (`contacts.getContact`); Java/PHP resource facades may shorten them. Streaming middleware is separate from buffered middleware.

## Gaps and evidence

Events, RPC, workflows and capabilities still need language-specific runnable packages. GraphQL retry/idempotency policy, automatic pagination and dynamic typed selections are application work or future features, not enabled by the HTTP runtime. Shared-model support does not imply runtime schema-validation parity.

This page audits code and existing tests; it does not report a new compiler or broker run. Native generated-code tests can be ignored by the ordinary workspace suite until their toolchains are explicitly enabled.

- [HTTP implementation](../../../crates/plugins/swift/src/http/)
- [HTTP operation tests](../../../crates/plugins/swift/src/operation_tests.rs)
- [GraphQL implementation](../../../crates/plugins/swift/src/graphql/)
- [Generated GraphQL compilation/runtime tests](../../../crates/plugins/swift/src/graphql/tests.rs)
- [Shared-model implementation and tests](../../../crates/plugins/swift/src/shared_models.rs)
- [GraphQL usage and layout guide](../outputs/graphql-swift.md)
