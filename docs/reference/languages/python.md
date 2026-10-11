# Python capabilities and generated APIs

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
| API styles | Built-in flat and namespaced; no public standalone raw style | Built-in raw, flat, idiomatic and explicit custom groups | Model imports; styles do not apply |
| Errors / partial results | Declared HTTP errors/exceptions; transport errors remain separate | `GraphqlResponse` preserves errors, partial data, extensions and data-key presence | Decode failures; no response envelope |
| Retry / backoff | Built-in configurable exponential delay, transient status/transport retries and server delay hints; default `max_retries=2` | No generated retry loop | Not applicable |
| Idempotency | Opt-in header and secure UUID; stable across retries; server deduplication is application-owned | No generated mutation idempotency policy | Not applicable |
| Timeout / cancellation | `for_call` seconds, per attempt; sync urllib is blocking; async httpx follows task/driver cancellation | Client timeout in seconds; stream iteration/cleanup is synchronous | Not applicable |
| Auth / security | API credentials, headers, token-provider/OAuth helpers; native transport TLS | Explicit headers; token refresh/security policy application-owned | No transport |
| Pagination / streaming | Cursor/page/offset/next-URL generators; optional async client; HTTP SSE | Fixed-operation SSE subscriptions and multipart frames; no inferred pager | No transport |
| Validation / codecs | Typed models and presence handling; opt-in response validation | Selected `TypedDict` annotations are not runtime schema validation; runtime scalar callbacks | Generated shape checks for supported models; not complete JSON Schema validation |

The async HTTP client is opt-in (`sdk().async_client(true)`) and uses httpx; it does not make the GraphQL client async. Details: [request controls](../../guides/request-controls.md), [idempotency](../../guides/idempotency.md), [pagination](../../guides/pagination.md), [GraphQL](../outputs/graphql-python.md), [streaming/codecs](../outputs/graphql-capabilities.md).

HTTP exposes runtime middleware plus `before_request`, `after_response` and `on_error` callbacks. Optional OAuth/client-credentials helpers use shared token-provider state with bounded authentication replay; callers still supply server credentials and policy. `webhooks()` emits raw-body signature verification separately; it does not create a webhook server. GraphQL uses its own transport and does not inherit these HTTP middleware/OAuth/webhook features.

See also [runtime middleware](../../guides/runtime-middleware.md).

## What the generated APIs look like

The HTTP shape follows the [Notes fixture](../../../examples/cli-basic/openapi.yaml). HTTP assumes package `example_sdk`, operation `getNote`, required path argument `note_id` (wire name `noteId`), and tag `notes`. Namespaced resource wrappers retain the direct method and may shorten its name.

```python
from example_sdk import Client
client = Client('https://api.example.com')
# Flat
note = client.get_note(note_id='42')
# Idiomatic / namespaced generation
note = client.notes.get(note_id='42')
```

**HTTP raw:** no public standalone raw callable/style is implemented. Internal operation mixins are not an advertised raw API.

GraphQL assumes package `example_graphql_client` and document `query ReadUser($id: ID!) { user(id: $id) { id name } }`. The selected result has `id` and `name`, not every schema field.

```python
from example_graphql_client import Client, operations
client = Client('https://api.example.com/graphql')
# Raw: explicit runtime transport (currently stored as _transport).
response = operations.read_user(client._transport, {'id': '42'})
# Flat
response = client.read_user({'id': '42'})
# Idiomatic: generated query group
response = client.query.read_user({'id': '42'})
# Only with groups={"users": {"read": "ReadUser"}}
response = client.users.read({'id': '42'})
```

These are alternative generation styles; raw generation does not promise bound flat/group methods. `response.errors` can accompany usable `response.data`.

## Files and model boundaries

HTTP emits `pyproject.toml`, `src/<module>/client.py`, `runtime.py`, presence/multipart/OAuth helpers, `models/<model>.py`, bounded model export partitions, operation chunks and optional `resources/` chunks. Optional async generation adds async client/runtime/resource counterparts.

GraphQL emits `client.py`, `runtime.py`, model and operation packages with individual declaration modules, group/mixin parts and scalar-codec shape files. Public reexports keep normal model/operation imports usable. Source layout diagnostics report indivisible declarations exceeding the 128 KiB GraphQL budget.

Shared models emit an installable typed package with individual model modules, codec helpers and bounded exports. Defaults, unions/intersections/negation are rejected in the current shared-model subset; enums/constants must be scalar Python `Literal`-compatible values. This differs from HTTP/GraphQL-specific model support.

## Composition and configuration

HTTP and GraphQL use the same language plugin package, with distinct generators and contracts. CLI recipes and npm language plugins select the input and `contracts.graphql.style`; GraphQL grouping maps supplied operation names, never inferred schema resources. Rust composition exposes `sdk()`, `graphql(Some(input.handle()))` and `shared_models(Some(blocks.handle()))`. Shared-model generation consumes schema-model blocks and publishes language symbols; it does not automatically supply RPC, event or workflow execution.

CLI/JS models-only mode and Rust block composition support the documented neutral model subset. Unsupported input/output execution pairs follow Poolster's skipped-plugin reporting policy; a parsed input alone does not prove generation support. See [provider wiring](../outputs/native-sdk-providers.md), [shared models](../outputs/shared-models.md), [input pipelines](../inputs/native-pipelines.md) and the [support matrix](../../plugin-support-matrix.md).

## Remaining gaps

No RPC, Kafka execution, workflow or capability client is bundled for this language. GraphQL selections remain fixed documents; dynamic typed `fields:` selection and GraphQL-aware automatic pagination, idempotency and retries remain application work. Experimental incremental delivery is the documented `deferSpec=20220824` dialect, not universal server compatibility.

## Source and test evidence

- [HTTP assembly](../../../crates/plugins/python/src/http/assembly.rs), [retry/transport](../../../crates/plugins/python/src/runtime/client.rs), [async runtime](../../../crates/plugins/python/templates/async_runtime.py).
- [HTTP style tests](../../../crates/plugins/python/src/tests/layout_errors.rs), [idempotency tests](../../../crates/plugins/python/src/tests/idempotency.rs), [async probes](../../../crates/plugins/python/src/tests/async_clients.rs).
- [GraphQL rendering](../../../crates/plugins/python/src/graphql/render.rs) and [generated local-server/style tests](../../../crates/plugins/python/src/graphql/tests.rs).
- [Shared-model subset](../../../crates/plugins/python/src/shared_models.rs) and [tests](../../../crates/plugins/python/src/shared_models/tests.rs).
