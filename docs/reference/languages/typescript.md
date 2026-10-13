# TypeScript generated packages

Audited **11 October 2026**, against the alpha.2 implementation. These are implemented
capabilities, not the [SDK specification](../../specifications/generated-sdk.md)'s
future requirements. Compiled ESM packages also work from plain JavaScript; types
are erased at build time. [Compare languages](README.md).

## Contracts

| Contract / input | Output today | Boundary |
| --- | --- | --- |
| HTTP / OpenAPI | Models, Fetch or Axios operations, bound clients, companions | Contract-specific serialization/security; no promise that arbitrary APIs need no annotations |
| GraphQL operations | Selected result/variable types, HTTP queries/mutations, optional SSE | Operation documents required; fixed selections |
| GraphQL incremental | Separate experimental raw multipart generator | `deferSpec=20220824`; not ordinary GraphQL client parity |
| Events / AsyncAPI | Shared message models and Kafka producers/consumers | AsyncAPI 3.0/3.1 static plaintext Kafka JSON subset |
| Workflows / Arazzo | Source-resolved sequential HTTP workflow runners | Explicit local sources; limited expressions/criteria; no actions/retry policies |
| RPC / Protobuf | Poolster-owned message types/schemas and gRPC clients/server interfaces | Native gRPC and optional HTTP adapters; editions unsupported |
| Capabilities / Cap’n Proto | No maintained TypeScript capability generator | Native document retention does not imply output support |
| Shared models | Models-only packages from exposed schema blocks | No transport or runtime validation merely from generating types |

## HTTP features

| Feature | Implemented behavior |
| --- | --- |
| Raw / flat / idiomatic | Standalone operation functions; direct client methods; namespaced resource methods. Raw-only surface omits the bound client |
| Results/errors | Successful decoded bodies by default; `throwOnError: false` returns status/media/data/headers envelopes. Transport/decoding/validation can still throw |
| Retry/backoff | Built-in Fetch/Axios loop; default up to 3 attempts for eligible methods. Exponential delays capped by configuration; `Retry-After` and `retry-after-ms` supported; `retry: false` disables it |
| Retry safety | Safe/idempotent method policy; POST/PATCH need a nonempty configured/conventional idempotency key. This is eligibility, not server deduplication |
| Idempotency | Opt-in operation/package policy generates or forwards a header; secure UUID once per call, reused across attempts. Server retention/deduplication and cross-call key persistence remain application responsibilities |
| Timeout/cancel | Default/per-call `timeoutMs` and `AbortSignal`; logical deadline covers middleware, driver, decoding and retry pauses |
| Authentication | Declared security credentials, static headers/API key and optional OAuth generator; HTTP features do not propagate automatically to GraphQL/Kafka |
| Pagination | Declared cursor, page, offset/limit and next-URL helpers yield pages as async iterables; no inference from arbitrary response field names |
| Streaming | Supported HTTP SSE/binary transport surfaces depend on the selected Fetch/Axios driver and declared response; not GraphQL subscriptions |
| Validation/codecs | Request/response validators, custom codecs and opt-in response checks; Zod companion output is a separate capability. Types alone do not validate JSON |
| Middleware/hooks | Async middleware plus `beforeRequest`, `afterResponse`, `onError`; authors can bundle middleware. Runtime hooks differ from Poolster generation hooks |
| Webhooks | Optional webhook artifact generator; verification/dispatch is not automatically installed on a server by generating an HTTP client |
| Companions | React Query, Vue Query, SWR, Zod, MSW, Faker and Cypress where the companion accepts the generated contract; these are separately configured generators |

The authoritative policy and timeout details are in [idempotency](../../guides/idempotency.md),
[request controls](../../guides/request-controls.md), [pagination](../../guides/pagination.md)
and [middleware](../../guides/runtime-middleware.md).

### What the three HTTP styles generate

These examples use the [Notes OpenAPI fixture](../../../examples/cli-basic/openapi.yaml):
`getNote`, tag `notes`, path parameter `noteId`, successful `Note { id, body }`.
The generated package/class names below assume `@poolster/notes-fetch` and `Notes`.
Each snippet represents its own selected style; imports are from the package's ESM build.

**Raw:** the caller supplies the configured transport to the generated function.

```ts
import { createClient, getNote } from '@poolster/notes-fetch';
const transport = createClient({ baseUrl: 'http://localhost:4010' });
const note = await getNote({ client: transport, path: { noteId: 'n1' } });
console.log(note.body);
```

**Flat:** the bound class preserves the operation name.

```ts
import { Notes } from '@poolster/notes-fetch';
const client = new Notes({ baseUrl: 'http://localhost:4010' });
const note = await client.getNote({ path: { noteId: 'n1' } });
```

**Idiomatic/namespaced:** tag-based resources shorten the operation method.

```ts
import { Notes } from '@poolster/notes-fetch';
const client = new Notes({ baseUrl: 'http://localhost:4010' });
const note = await client.notes.get({ path: { noteId: 'n1' } });
```

The idiomatic path is derived from tags/path and operation naming; it is not a hardcoded
`notes` API. Collisions and unusual names follow the generator's naming policy. Configure
HTTP `client_style: "flat"` or `"namespaced"` in a CLI recipe, or `clientStyle` in the
JavaScript SDK. Raw-only generation uses `--typescript-surface raw` or Rust
`ts::sdk().fetch().raw()`; that surface choice is distinct from facade style.

```ts
const client = new Notes({
  baseUrl: 'http://localhost:4010',
  retry: { maxAttempts: 2, initialDelayMs: 250, maxDelayMs: 2_000 },
});
const result = await client.getNote({
  path: { noteId: 'n1' },
  throwOnError: false,
  requestOptions: { headers: { 'X-Trace-ID': 't1' }, timeoutMs: 5_000 },
});
console.log(result.status, result.data);
```

This control example assumes the **flat** generation above. An idempotency policy on
`createOrder` would expose its header through that operation's `headers`; it would not
change `getNote` into a mutation or enforce deduplication on the server.

## GraphQL features

| Feature | Implemented behavior |
| --- | --- |
| Raw / flat / idiomatic | Raw operations, flat bound methods, grouped bound methods; `idiomatic`/`namespaced` alias grouped. Default grouped |
| Selections/presence | Validated operation documents drive result types; aliases/fragments, nullability and conditional presence retained. No typed runtime `fields` selector |
| Errors/partial data | `GraphqlResult<T>` has `success`, `partial`, `error`; GraphQL errors are data. HTTP/protocol failures throw separately |
| Retry/idempotency | No bundled operation retry loop or HTTP idempotency policy. Caller/custom transport owns safe replay and server mutation semantics |
| Timeout/cancel | Request `signal`, headers and injected Fetch; no built-in numeric HTTP `timeoutMs` policy in this runtime |
| Authentication | Configured/per-request headers or custom transport; no generated OAuth refresh flow inherited from the HTTP SDK |
| Subscriptions | Explicit enablement; injected `SubscriptionTransport` or bundled distinct-connection graphql-sse. No WebSocket, multiplexing, reconnect/replay |
| Incremental | Separate raw generator and contract; experimental multipart patches, with documented limits |
| Scalars | Direction-specific static mappings and supplied encode/decode callbacks; a `Date` type mapping alone does not perform conversion |
| Validation | Operations validated at generation; response envelope/protocol checks at runtime. Core client is not full selected-field JSON Schema validation; Zod companion is separate |
| Pagination | No automatic Relay/page helpers; supply named operations and manage cursors in the application |
| Companions | Query/mutation integrations for React/Vue Query, SWR, Cypress, MSW, Faker and Zod. Subscription/incremental integration support is not implied |

### What the three GraphQL styles generate

Assume operations `ReadUser($id: ID!)` selecting `{ user { id name } }` and
`RenameUser($id: ID!, $name: String!)`. Their selections are fixed in the documents.

**Raw:** both the operation-named export (`ReadUser`) and its camelCase alias (`readUser`) take transport, variables and optional request options.

```ts
import { ReadUser, createGraphqlHttpTransport } from './generated/index.js';
const transport = createGraphqlHttpTransport('http://localhost:4000/graphql');
const result = await ReadUser(transport, { id: '42' });
```

**Flat:** configure `contracts.graphql.style: 'flat'`.

```ts
import { createClient } from './generated/index.js';
const client = createClient({ endpoint: 'http://localhost:4000/graphql' });
const result = await client.readUser({ id: '42' });
await client.renameUser({ id: '42', name: 'Ada' });
```

**Idiomatic/grouped:** without an explicit mapping, groups follow operation kind.

```ts
const result = await client.query.readUser({ id: '42' });
await client.mutation.renameUser({ id: '42', name: 'Ada' });
```

An author can supply `contracts.graphql.groups`:

```js
{ style: 'grouped', groups: {
  user: { read: 'ReadUser', rename: 'RenameUser' },
} }
// Generated: client.user.read({ id: '42' })
//            client.user.rename({ id: '42', name: 'Ada' })
```

GraphQL grouping is explicit metadata; it is not inferred from a `user` selection.
Unmapped operations retain query/mutation/subscription groups. All styles preserve
partial results:

```ts
if (result.kind === 'partial') console.log(result.data, result.errors);
else if (result.kind === 'success') console.log(result.data);
else console.error(result.errors);
```

See [GraphQL TypeScript](../outputs/graphql-typescript.md),
[advanced capabilities](../outputs/graphql-capabilities.md) and
[integrations](../outputs/graphql-integrations.md) for complete configuration.

## Kafka events and workflows

| Feature | Kafka events | Arazzo workflows |
| --- | --- | --- |
| API style | Typed standalone send/consume wrappers taking `KafkaClient`; no raw/flat/idiomatic selector | Standalone `runWorkflow` and generated per-workflow function; no style selector |
| Retry | KafkaJS native connection/producer behavior; no Poolster per-operation HTTP retry policy | No bundled workflow retry/action policy |
| Idempotency | No exposed producer idempotency/transaction policy or exactly-once claim; application deduplication is separate | No synthesized idempotency keys or durable execution journal |
| Errors | Schema/decode failures explicit; handler failures propagate. Optional `onInvalidMessage` can deliberately handle/skip invalid records | `WorkflowExecutionError` retains execution and step context; expected statuses checked |
| Acknowledgement | KafkaJS `eachMessage` offset/auto-commit behavior after successful handler completion; returning from `onInvalidMessage` deliberately permits that record to be skipped. No generated explicit Rust-style ack API | Not a broker consumer |
| Cancellation/security | KafkaJS client lifecycle/configuration; generated input subset plaintext, no validated SASL/TLS lowering | Fetch and `AbortSignal`, source URL overrides and headers; no generated OAuth refresh policy |
| Validation | AJV payload/header/key checks on send/receive; invalid UTF-8/tombstones/unsupported headers rejected | Supported inputs/expressions validated; no universal response-schema validator |

Assume send operation `sendEvent`, receive operation `receiveEvent`, a JSON message
`{ id: string }`, optional string keys, and no required headers. Exports come from the
operation IDs, and the client factory carries the generated default broker configuration.

```ts
import { createKafkaClient, sendEvent, receiveEvent } from './generated/index.js';
const kafka = createKafkaClient({ brokers: ['127.0.0.1:29092'] });
const sent = await sendEvent(kafka, { payload: { id: 'e1' }, key: 'e1' });
const consumer = await receiveEvent(kafka, async (message, metadata) => {
  console.log(message.payload.id, metadata.partition);
}, { groupId: 'example-consumer', fromBeginning: true });
// Later, when shutting down:
await kafka.disconnect();
```

Send returns KafkaJS `RecordMetadata[]`; consume returns a running KafkaJS `Consumer`,
not a promise for every future message. A fixed operation `groupId` cannot be overridden
with a conflicting value. Provision topics separately. Handler failures follow KafkaJS's
consumer run/crash behavior; awaiting initial consumer setup is not a permanent error
monitor for that consumer.
See the [Kafka guide](../outputs/kafka.md) for concrete send/consume setup and limits.

A resolved workflow ID `order` produces a typed `runWorkflow_6f72646572` export
(the suffix is its UTF-8 bytes in hex) and the generic entry point:

```ts
import { runWorkflow } from './generated/index.js';
const execution = await runWorkflow('order', { orderId: 'o1' }, {
  sourceBaseUrls: { orders: 'http://localhost:4010' },
  headers: { Authorization: 'Bearer token' },
});
console.log(execution.outputs);
```

Inputs, source names and outputs above depend on the actual resolved Arazzo document.
See [native workflows](../inputs/native-pipelines.md#arazzo--typescript-workflow-runners).

## Shared models

| Feature | Behavior |
| --- | --- |
| Styles, retry, idempotency, authentication | Not applicable: this output contains models, not a client |
| Types and validation | Supported schema shapes become TypeScript declarations; no validator merely from generating shared types |
| Identity and composition | Stable schema entity IDs and selected contract provenance; event clients resolve actual published model symbols |

See [shared models](../outputs/shared-models.md) for schema limits and generation options.

## Generated files and evidence

| Output | Typical layout |
| --- | --- |
| HTTP | `models/`, per-operation `clients/`, `client.ts`, namespaced `resources/`, runtime/entry barrels and manifest; paths vary with configured plugin directories |
| GraphQL | `graphql.ts`, `graphql/models/`, `graphql/operations/`, `graphql-client.ts`, bounded client parts and `graphql-runtime.ts` |
| Events | Shared `models/`, `events/models/` aliases, `events/operations/`, `events.ts`, `kafka-runtime.ts`, retained `asyncapi.json` and model provenance manifest; legacy standalone factory emits its own models |
| Workflows | `workflows.ts`, `workflow-runtime.ts`, package manifest/build configuration |

Formatting and ownership are generator responsibilities; regeneration preserves unrelated
user files and removes obsolete unchanged owned output. Large indivisible declarations
can produce explicit source-layout diagnostics; do not assume arbitrary contracts yield
uniformly tiny files. See [GraphQL layout](../outputs/graphql-layout.md).

Source evidence: [HTTP runtime](../../../crates/plugins/typescript/src/sdk/runtime/fetch.ts.tmpl),
[HTTP facades](../../../crates/plugins/typescript/src/sdk/facade.rs),
[idempotency probes](../../../crates/plugins/typescript/src/idempotency_tests.rs),
[GraphQL generator](../../../crates/plugins/typescript/src/graphql/generate.rs),
[GraphQL style tests](../../../crates/plugins/typescript/src/graphql/facade_tests.rs),
[Kafka runtime](../../../crates/plugins/typescript/src/events/runtime/kafka.ts.tmpl),
[event wrapper generation](../../../crates/plugins/typescript/src/events/mod.rs),
[workflow runner](../../../crates/plugins/typescript/src/workflow.rs) and
[workflow runtime](../../../crates/plugins/typescript/templates/workflow_runtime.ts.tmpl).
The [support matrix](../../plugin-support-matrix.md) and
[batch verification](../../verification-results/kafka-shared-models-2026-10-11.json)
record tested boundaries; this documentation audit adds no new runtime test passes.


## Protobuf / gRPC

Poolster writes this contract's source itself in Rust; generation invokes no
external protoc, target interpreter or code generator. It consumes the selected
`RpcContract` and the shared owned wire-schema projection, preserving retained
descriptors for native serialization.

Use `ts::grpc().input(source.handle())` in a TypeScript package, or select
the `grpc` plugin in CLI recipes. The JavaScript SDK reuses `pluginTypeScript`
with `inputProtobuf`. Runtime dependencies pin @bufbuild/protobuf 2.16.0 and
@grpc/grpc-js 1.14.6.

Output has per-proto modules containing messages/schema constants and service
interfaces, a separate descriptor registry, and one npm manifest/tsconfig.
Message construction and binary serialization use Buf runtime schemas; source
rendering is owned by Poolster. Int64 fields use bigint, real oneofs use
discriminated unions and explicit field presence is retained.

`<Service>Client`, `<Service>Server` and `<Service>Service` expose unary and all
streaming directions. Metadata, deadlines, status errors and cancellation follow
grpc-js. Separate binary gRPC-web and unary Twirp adapters are opt-in. Proto2 groups/extensions are covered by native fixtures; editions remain unsupported. Naming collisions fail explicitly.

Native gRPC APIs remain available alongside selectable raw, flat and idiomatic facades. See the [Protobuf guide](../outputs/protobuf.md) for recipe,
SDK and call examples.

## Protobuf conformance audit

The existing gRPC output is partial against the all-language target. No complete
conformance profile is certified yet; see current evidence and native boundaries in the
[Protobuf audit](../outputs/protobuf-conformance.md) and
[requirements](../../specifications/protobuf-conformance.md).

Browser consumers use the generated `/browser` entry. The package root includes
native Node gRPC clients. Checked alternate value models expose explicit
`ValueCodec` conversions and provenance-preserving `clone`; external import
mappings currently require native representations.
