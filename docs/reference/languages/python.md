# Python capabilities and generated APIs

Audited **12 October 2026**, against this checkout's unreleased alpha.2 generators. This is implemented behavior, not the target [SDK specification](../../specifications/generated-sdk.md), and not a claim that every native test runs in every environment.

`Built-in` means Poolster emits the behavior. `Driver` means the native library owns it. `Application` means callers must implement the policy. An HTTP transport does not make GraphQL an HTTP contract.

## Accepted contracts

| Contract | Current output |
| --- | --- |
| HTTP / OpenAPI `Api` | SDK, models, declared errors and resource facade |
| GraphQL operations | Fixed-document query/mutation clients; opt-in SSE subscriptions and experimental multipart incremental delivery |
| Shared schema-model blocks | Models-only package, including supported AsyncAPI payload/header/key shapes |
| RPC / Protobuf | Poolster-owned messages and gRPC clients/server interfaces |
| Events / AsyncAPI | Typed Kafka JSON producers/consumers over shared models; static plaintext Kafka subset |
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

## Kafka events

The Kafka contract generator is separate from HTTP and GraphQL. It reuses the selected event revision's shared payload/header/key models and rejects stale or missing role symbols before emission. Native documents and a versioned event-contract manifest are retained.

| Feature | Kafka JSON events |
| --- | --- |
| API styles | Flat typed operation methods; no alternate raw/grouped style switch |
| Producer retries | Driver retries configured explicitly with `KafkaClient(retries=N)`; default zero |
| Idempotency / transactions | Unsupported; retries may duplicate deliveries; application deduplication required |
| Consumer acknowledgement | Explicit `record.ack()`; `subscription.run(handler)` commits only after handler returns successfully |
| Handler retries | Application-owned; handler errors propagate and leave the record unacknowledged |
| Cancellation / cleanup | `threading.Event`, checked between bounded 250 ms polls; context-managed client/subscription cleanup |
| Errors | Driver, JSON/schema and decoding errors propagate; malformed records poison the subscription until close/reopen |
| Validation | Draft-07 constraints before send and after receive; strict UTF-8 payload/key/header decoding, no tombstones or duplicate headers/JSON properties |
| Broker bindings | Fixed operation topic/group/client IDs; conflicting caller group overrides rejected |
| Transport | Plaintext Kafka only; no auth/TLS configuration in this generator |

For message `User` and operations `sendUser` / `receiveUser`, the generated API is:

```python
from example_events import KafkaClient
from example_events.models import UserPayload, UserHeaders

with KafkaClient() as client:
    client.send_user(
        UserPayload.from_dict({"id": "42", "nickname": None}),
        headers=UserHeaders.from_dict({"trace": "request-1"}), key="42",
    )
    with client.receive_user() as subscription:  # operation's declared groupId
        for record in subscription:
            process(record.payload)
            record.ack()  # commit only after successful processing
```

Omitted optional fields stay omitted; explicitly present nulls stay null. Receive iteration requires acknowledgement before polling another record. Provision topics separately. Synchronous send waits for broker acknowledgement and accepts `timeout=`; timing out does not prove that the broker did not receive the record. Closing a subscription never automatically commits.

Dependencies are pinned to [kafka-python 3.0.11](https://pypi.org/project/kafka-python/3.0.11/) and [jsonschema 4.26.0](https://pypi.org/project/jsonschema/4.26.0/). Shared-model defaults, unions/intersections/negation and unsupported literals remain rejected. Schema `format` constraints are rejected rather than silently ignored; only the documented Draft-07 subset is accepted.

## Files and model boundaries

HTTP emits `pyproject.toml`, `src/<module>/client.py`, `runtime.py`, presence/multipart/OAuth helpers, `models/<model>.py`, bounded model export partitions, operation chunks and optional `resources/` chunks. Optional async generation adds async client/runtime/resource counterparts.

GraphQL emits `client.py`, `runtime.py`, model and operation packages with individual declaration modules, group/mixin parts and scalar-codec shape files. Public reexports keep normal model/operation imports usable. Source layout diagnostics report indivisible declarations exceeding the 128 KiB GraphQL budget.

Shared models emit an installable typed package with individual model modules, codec helpers and bounded exports. Defaults, unions/intersections/negation are rejected in the current shared-model subset; enums/constants must be scalar Python `Literal`-compatible values. This differs from HTTP/GraphQL-specific model support.

Kafka adds one operation module per operation, bounded 100-operation mixin chunks, `events/client.py`, cohesive runtime/codec modules, package dependencies and ownership metadata. A package supports at most 10000 event operations; shared declarations and barrel chunks remain shared.

## Composition and configuration

HTTP and GraphQL use the same language plugin package, with distinct generators and contracts. CLI recipes and npm language plugins select the input and `contracts.graphql.style`; GraphQL grouping maps supplied operation names, never inferred schema resources. Rust composition exposes `asyncapi(Some(input.handle())).using_models(models.handle())`, `sdk()`, `graphql(Some(input.handle()))` and `shared_models(Some(blocks.handle()))`. Shared-model generation consumes schema-model blocks and publishes language symbols; it does not automatically supply RPC, event or workflow execution.

CLI/JS models-only mode and Rust block composition support the documented neutral model subset. Unsupported input/output execution pairs follow Poolster's skipped-plugin reporting policy; a parsed input alone does not prove generation support. See [provider wiring](../outputs/native-sdk-providers.md), [shared models](../outputs/shared-models.md), [input pipelines](../inputs/native-pipelines.md) and the [support matrix](../../plugin-support-matrix.md).

## Remaining gaps

Workflow and capability clients remain unsupported. Kafka security, transactions, schema registry, dynamic topics, nonstring keys/headers and automatic handler retries remain unsupported. GraphQL selections remain fixed documents; dynamic typed `fields:` selection and GraphQL-aware automatic pagination, idempotency and retries remain application work. Experimental incremental delivery is the documented `deferSpec=20220824` dialect, not universal server compatibility.

## Source and test evidence

- [HTTP assembly](../../../crates/plugins/python/src/http/assembly.rs), [retry/transport](../../../crates/plugins/python/src/runtime/client.rs), [async runtime](../../../crates/plugins/python/templates/async_runtime.py).
- [HTTP style tests](../../../crates/plugins/python/src/tests/layout_errors.rs), [idempotency tests](../../../crates/plugins/python/src/tests/idempotency.rs), [async probes](../../../crates/plugins/python/src/tests/async_clients.rs).
- [GraphQL rendering](../../../crates/plugins/python/src/graphql/render.rs) and [generated local-server/style tests](../../../crates/plugins/python/src/graphql/tests.rs).
- [Shared-model subset](../../../crates/plugins/python/src/shared_models.rs) and [tests](../../../crates/plugins/python/src/shared_models/tests.rs).

- [Kafka generator and subset checks](../../../crates/plugins/python/src/events/mod.rs), [runtime](../../../crates/plugins/python/src/events/runtime.py.tmpl) and [generated installation/local-broker tests](../../../crates/plugins/python/src/events/tests.rs).


## Protobuf / gRPC

Poolster writes this contract's source itself in Rust; generation invokes no
external protoc, target interpreter or code generator. It consumes the selected
`RpcContract` and the shared owned wire-schema projection, preserving retained
descriptors for native serialization.

Use `python::grpc("example-rpc").input(source.handle())` in a Python package,
or select the `grpc` plugin in CLI recipes. The JavaScript SDK reuses
`pluginPython` with `inputProtobuf`.

Output has per-proto `_pb2.py` message bindings, `.pyi` typing declarations and
`_pb2_grpc.py` service files, plus separate descriptor/runtime modules and one
`pyproject.toml`. Runtime dependencies pin protobuf 6.33.5 and
grpcio 1.78.0. The message classes use the native descriptor/message runtime;
Poolster emits the bindings and client/server source.

Unary and all streaming directions use native channel APIs. Server implementations
subclass `<Service>Servicer` and use `add_<Service>Servicer_to_server`. Metadata,
status errors, timeouts and cancellation follow grpcio. Typing declarations target
the synchronous API. Proto2 defaults/extensions/required presence, proto3 optional
fields, oneofs, maps and well-known types remain native runtime semantics. Custom
files in `google/protobuf/`, module/name collisions and editions are rejected.

These APIs use the native gRPC surface; HTTP/GraphQL raw, flat and idiomatic
settings do not apply. See the [Protobuf guide](../outputs/protobuf.md) for recipe,
SDK and call examples.
