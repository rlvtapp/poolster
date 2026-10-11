# Elixir capabilities and generated APIs

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
| API styles | Built-in direct `API` functions and resource modules; no separate public raw style | Raw `Operations`, flat package facade, idiomatic query/mutation/custom modules | Struct modules; styles do not apply |
| Errors / partial results | `{:ok, value}` / `{:error, reason}` including declared API errors | Envelope preserves errors/data/extensions/presence; `require_data` returns an error for GraphQL errors | Invalid decoded fields raise |
| Retry / backoff | Built-in bounded exponential/status retry policy and server delay hints; `max_retries=2` default | No generated retry loop | Not applicable |
| Idempotency | Opt-in header/secure UUID, preserved across retries; server deduplication application-owned | No generated mutation deduplication | Not applicable |
| Timeout / cancellation | `for_call` milliseconds; Finch receive timeout per attempt; task/process cancellation is application/driver-owned | Configurable Finch timeout and streaming task/process lifetime | Not applicable |
| Auth / security | API-key header/prefix and arbitrary headers; optional OAuth client-credentials GenServer/middleware helper | Explicit headers/custom transport; refresh policy application-owned | No transport |
| Pagination / streaming | Lazy cursor/page/offset/absolute same-origin URL `Stream`; HTTP SSE uses Finch streaming | Opt-in graphql-sse subscriptions and experimental multipart streams | No transport |
| Validation / codecs | Struct/presence conversion; not a full schema-validation runtime | Required fields and scalar/list/null shapes; runtime direction-specific scalar callbacks | Supported shape guards; not complete JSON Schema validation |

HTTP pagination does not support body controls or relative continuation URLs. See [request controls](../../guides/request-controls.md), [pagination](../../guides/pagination.md), [GraphQL](../outputs/graphql-elixir.md), [streaming/codecs](../outputs/graphql-capabilities.md).

HTTP exposes middleware and `before_request`, `after_response`, `on_error` callbacks. The separate `oauth()` helper emits a client-credentials GenServer plus same-origin buffered middleware and one safe buffered 401 replay; callers supply a bounded, redirect-disabled token-fetch driver. `webhooks()` separately emits raw-body signature verification, not a server. GraphQL does not inherit this HTTP middleware/OAuth/webhook path.

See also [runtime middleware](../../guides/runtime-middleware.md).

## What the generated APIs look like

The HTTP shape follows the [Notes fixture](../../../examples/cli-basic/openapi.yaml). HTTP assumes Mix package `example_api_sdk`, module `ExampleApiSdk`, operation `getNote`, required path `note_id` (wire name `noteId`), tag `notes`. Resource modules preserve the operation name.

```elixir
{:ok, client} = ExampleApiSdk.client(base_url: "https://api.example.com")
# Flat: explicit client, following Elixir conventions.
{:ok, note} = ExampleApiSdk.API.get_note(client, note_id: "42")
# Idiomatic / namespaced generation
{:ok, note} = ExampleApiSdk.Resources.Notes.get_note(client, note_id: "42")
```

**HTTP raw:** there is no separately selectable public raw API. Internal `API.OperationsNNNN` partitions are implementation details.

GraphQL assumes package module `Example`, operation `ReadUser`, required variable `id`, selecting `user { id name }`.

```elixir
client = Example.Client.new("https://api.example.com/graphql")
variables = %Example.Models.ReadUserVariables{id: "42"}
# Raw
{:ok, response} = Example.Operations.read_user(client, variables)
# Flat
{:ok, response} = Example.read_user(client, variables)
# Idiomatic
{:ok, response} = Example.Query.read_user(client, variables)
# Only with groups: %{users: %{read: "ReadUser"}}
{:ok, response} = Example.Users.read(client, variables)
```

Styles are alternatives. Mutations use `Example.Mutation.<operation>` in idiomatic output. Optional GraphQL fields use `:poolster_absent` for absence and `nil` for explicit null. Inspect `response.errors` with `response.data`; `Example.Runtime.require_data(response)` requires error-free data.

## Files and model boundaries

HTTP emits `mix.exs`, application/client/runtime/error modules, individual model modules, bounded `api/operations_NNNN.ex` partitions and resource chunks. Direct `API` delegates remain available in namespaced output. HTTP currently requests Finch `~> 0.18` and Jason `~> 1.4`; GraphQL requests Finch 0.24.0 and Jason 1.4.5, so the generated manifests are not identical.

GraphQL emits separate runtime/client/application/envelope modules, individual model and operation modules, and bounded facade/group parts. The 128 KiB source budget records indivisible oversized declarations as diagnostics.

Shared models emit structs and wire conversion, accepting supported primitive/object/array/reference shapes. Defaults, enum/literal declarations, unions, intersections and negation are rejected by the current subset.

## Composition and configuration

HTTP and GraphQL use the same language plugin package, with distinct generators and contracts. CLI recipes and npm language plugins select the input and `contracts.graphql.style`; GraphQL grouping maps supplied operation names, never inferred schema resources. Rust composition exposes `sdk()`, `graphql(Some(input.handle()))` and `shared_models(Some(blocks.handle()))`. Shared-model generation consumes schema-model blocks and publishes language symbols; it does not automatically supply RPC, event or workflow execution.

CLI/JS models-only mode and Rust block composition support the documented neutral model subset. Unsupported input/output execution pairs follow Poolster's skipped-plugin reporting policy; a parsed input alone does not prove generation support. See [provider wiring](../outputs/native-sdk-providers.md), [shared models](../outputs/shared-models.md), [input pipelines](../inputs/native-pipelines.md) and the [support matrix](../../plugin-support-matrix.md).

## Remaining gaps

No RPC, Kafka execution, workflow or capability client is bundled for this language. GraphQL selections remain fixed documents; dynamic typed `fields:` selection and GraphQL-aware automatic pagination, idempotency and retries remain application work. Experimental incremental delivery is the documented `deferSpec=20220824` dialect, not universal server compatibility.

## Source and test evidence

- [HTTP assembly](../../../crates/plugins/elixir/src/http/assembly.rs), [public style templates](../../../crates/plugins/elixir/src/http/docs.rs), [Finch/retry runtime](../../../crates/plugins/elixir/templates/http/client.ex.tmpl).
- [OAuth helper](../../../crates/plugins/elixir/src/oauth.rs) and [generated OAuth middleware](../../../crates/plugins/elixir/templates/oauth.ex.tmpl).
- [HTTP native/runtime/style tests](../../../crates/plugins/elixir/src/http/tests.rs).
- [GraphQL emission](../../../crates/plugins/elixir/src/graphql/emission.rs), [tests](../../../crates/plugins/elixir/src/graphql/tests.rs), [generated native probe](../../../crates/plugins/elixir/src/graphql/runtime_test.exs.tmpl).
- [Shared-model subset](../../../crates/plugins/elixir/src/shared_models.rs) and [tests](../../../crates/plugins/elixir/src/shared_models/tests.rs).
