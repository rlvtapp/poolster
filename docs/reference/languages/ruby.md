# Ruby capabilities and generated APIs

Audited **11 October 2026**, against this checkout's unreleased alpha.2 generators. This is implemented behavior, not the target [SDK specification](../../specifications/generated-sdk.md), and not a claim that every native test runs in every environment.

`Built-in` means Poolster emits the behavior. `Driver` means the native library owns it. `Application` means callers must implement the policy. An HTTP transport does not make GraphQL an HTTP contract.

## Accepted contracts

| Contract | Current output |
| --- | --- |
| HTTP / OpenAPI `Api` | SDK, models, declared errors and resource facade |
| GraphQL operations | Fixed-document query/mutation clients; opt-in SSE subscriptions and experimental multipart incremental delivery |
| Shared schema-model blocks | Models-only package, including supported AsyncAPI payload/header/key shapes; no broker client |
| RPC / Protobuf | Unsupported |
| Events / AsyncAPI | No producer/consumer implementation; models-only is supported separately |
| Workflows / Arazzo | Unsupported |
| Capabilities / Cap’n Proto | Unsupported |

## Features by implemented contract

| Feature | HTTP | GraphQL | Shared models |
| --- | --- | --- | --- |
| API styles | Built-in flat and resource/namespaced methods; no standalone raw style | Built-in raw module functions, flat client, idiomatic groups and explicit groups | Model classes; styles do not apply |
| Errors / partial results | HTTP `ApiError`, transport errors and optional validation errors | Envelope preserves errors/data/extensions/presence; `require_data` raises on GraphQL errors | Invalid field decoding raises |
| Retry / backoff | Built-in opt-in retry loop, exponential bounded delay, server delay hints; `max_attempts=1` default | No generated retry loop | Not applicable |
| Idempotency | Opt-in key header/secure UUID; same logical-call key through retries; server deduplication application-owned | No generated mutation deduplication | Not applicable |
| Timeout / cancellation | Logical timeout in seconds, Net::HTTP open/read timeout; callback checks around attempts/waits, not active-driver cancellation | Net::HTTP timeout; enumerator lifetime/cleanup governs streams | Not applicable |
| Auth / security | Static credentials, headers and managed token-provider helpers | Explicit headers; credential refresh policy application-owned | No transport |
| Pagination / streaming | Page-number `Enumerator`; cursor/offset/URL declarations unsupported; no generalized HTTP SSE claim | Distinct-connection graphql-sse enumerators and multipart frames | No transport |
| Validation / codecs | Model conversion/presence; opt-in response validation | Selected models validate required/null/scalar/list shapes; RBS; runtime scalar callbacks | Supported shape checks; not complete JSON Schema validation |

The current generated retry implementation is authoritative: Ruby retries are available when `max_attempts` exceeds one. Details: [request controls](../../guides/request-controls.md), [pagination](../../guides/pagination.md), [GraphQL](../outputs/graphql-ruby.md), [streaming/codecs](../outputs/graphql-capabilities.md).

HTTP middleware wraps native requests and runs per attempt. The generated client does not expose the same named lifecycle callbacks as Python/Elixir. Optional OAuth helpers provide a managed token provider and bounded safe authentication replay. `webhooks()` separately emits raw-body signature verification; server routing remains application-owned. GraphQL does not inherit the HTTP middleware/OAuth/webhook pipeline.

See also [runtime middleware](../../guides/runtime-middleware.md).

## What the generated APIs look like

The HTTP shape follows the [Notes fixture](../../../examples/cli-basic/openapi.yaml). HTTP assumes gem `example-sdk`, module `ExampleSdk`, operation `getNote`, required path `note_id` (wire name `noteId`), tag `notes`.

```ruby
require 'example_sdk'
client = ExampleSdk::Client.new(base_url: 'https://api.example.com')
# Flat
note = client.get_note(note_id: '42')
# Idiomatic / namespaced generation
note = client.notes.get(note_id: '42')
```

**HTTP raw:** no standalone public raw generator is implemented. Both supported styles use the configured client; resource wrappers retain direct methods.

GraphQL assumes gem `example_graphql`, module `ExampleGraphql`, and operation `ReadUser` selecting `user { id name }` with required variable `id`.

```ruby
require 'example_graphql'
client = ExampleGraphql::Client.new('https://api.example.com/graphql')
# Raw
response = ExampleGraphql.read_user(client.transport, 'id' => '42')
# Flat
response = client.read_user('id' => '42')
# Idiomatic
response = client.query.read_user('id' => '42')
# Only with groups={users: {read: 'ReadUser'}}
response = client.users.read('id' => '42')
```

These are alternative generation styles. Mutation groups use `client.mutation.<operation>`; custom resource names require explicit mappings. Responses expose `data`, `errors`, `status` and `require_data`; nested models expose selected getters and `to_h`.

## Files and model boundaries

HTTP emits a gemspec, `lib/<gem>.rb` loader, model/client runtime files and bounded model/operation/resource chunks for large APIs. It does not emit exactly one HTTP file per operation at every size.

GraphQL emits individual model, operation, client and group Ruby files with corresponding split RBS signatures. The loader and public constants remain stable. The 128 KiB source budget has diagnostics for indivisible oversized declarations.

Shared models emit classes and wire conversion with stable symbol metadata. The current subset rejects schema defaults, literal/enum declarations, unions, intersections and negation; primitive/object/array/reference shapes are supported. Do not infer this subset from the richer GraphQL renderer.

## Composition and configuration

HTTP and GraphQL use the same language plugin package, with distinct generators and contracts. CLI recipes and npm language plugins select the input and `contracts.graphql.style`; GraphQL grouping maps supplied operation names, never inferred schema resources. Rust composition exposes `sdk()`, `graphql(Some(input.handle()))` and `shared_models(Some(blocks.handle()))`. Shared-model generation consumes schema-model blocks and publishes language symbols; it does not automatically supply RPC, event or workflow execution.

CLI/JS models-only mode and Rust block composition support the documented neutral model subset. Unsupported input/output execution pairs follow Poolster's skipped-plugin reporting policy; a parsed input alone does not prove generation support. See [provider wiring](../outputs/native-sdk-providers.md), [shared models](../outputs/shared-models.md), [input pipelines](../inputs/native-pipelines.md) and the [support matrix](../../plugin-support-matrix.md).

## Remaining gaps

No RPC, Kafka execution, workflow or capability client is bundled for this language. GraphQL selections remain fixed documents; dynamic typed `fields:` selection and GraphQL-aware automatic pagination, idempotency and retries remain application work. Experimental incremental delivery is the documented `deferSpec=20220824` dialect, not universal server compatibility.

## Source and test evidence

- [HTTP assembly/styles](../../../crates/plugins/ruby/src/http/assembly.rs), [client controls](../../../crates/plugins/ruby/src/http/client.rs), [actual retry implementation](../../../crates/plugins/ruby/templates/retry_runtime.rb.txt).
- [HTTP generated runtime tests](../../../crates/plugins/ruby/src/tests.rs) and [operation tests](../../../crates/plugins/ruby/src/operation_tests.rs).
- [GraphQL rendering](../../../crates/plugins/ruby/src/graphql/render.rs), [style/model/local-server probes](../../../crates/plugins/ruby/src/graphql/tests.rs).
- [Shared-model subset](../../../crates/plugins/ruby/src/shared_models.rs) and [tests](../../../crates/plugins/ruby/src/shared_models/tests.rs).
