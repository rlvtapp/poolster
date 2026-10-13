# Protobuf messages and gRPC packages

Poolster accepts `.proto` sources and resolves imports through the Protobuf input
provider. `RpcContract` retains the full descriptor set and source documents.
Services and streaming metadata stay separate from HTTP models.

Python, TypeScript and Go source generation runs inside Poolster in Rust. No external
code generator is invoked. The shared owned wire schema preserves field numbers,
labels, defaults, oneofs, message/enum references and dependency information.
Native libraries perform serialization and gRPC transport in the generated app.
Upstream generators are used only as independent test peers. The retained Go toolchain configuration is accepted for backward compatibility and is not invoked during generation.

## Choose an output

| Language | Source generation | Runtime | Package structure |
| --- | --- | --- | --- |
| Python | Poolster emits message bindings, typing declarations, stubs and server registration | protobuf 6.33.5, grpcio 1.78.0 | Per-proto `_pb2.py`/`.pyi` and `_pb2_grpc.py`, separate descriptor/runtime modules, one `pyproject.toml` |
| TypeScript | Poolster emits message types/schemas, clients and server interfaces | @bufbuild/protobuf 2.16.0, @grpc/grpc-js 1.14.6 | Per-proto modules under `src/`, separate descriptor registry, one npm manifest/tsconfig |
| Go | Poolster emits native message bindings, clients and server registration | protobuf 1.36.11, gRPC 1.83.2 | Per-proto message/service files, bounded descriptor parts, one Go module |

All three expose unary, client-streaming, server-streaming and bidirectional calls.
Applications implement the generated server interfaces and configure channels,
credentials, metadata, deadlines and cancellation through the native runtime.
Generation does not start a server or connect to one.

## CLI recipe

```json
{
  "input": {
    "format": "protobuf",
    "path": "proto/service.proto",
    "options": { "import_roots": ["proto"] }
  },
  "output": { "path": "generated" },
  "packages": [
    { "language": "python", "path": "python", "name": "example-rpc",
      "plugins": [{ "name": "grpc" }] },
    { "language": "typescript", "path": "typescript", "name": "@example/rpc",
      "plugins": [{ "name": "grpc" }] }
  ]
}
```

Run `poolster generate --config poolster.json`, or use
`poolster generate proto/service.proto --input-format protobuf --import-root proto -l python,typescript -o generated`.
Python/TypeScript recipes reject external generator toolchains and Go-only mappings.
The existing Go `module`, `go_packages` and toolchain options remain available.
Unsupported output languages produce an empty result with a warning/skipped report.
Malformed supported inputs and incompatible options fail clearly.

## Rust and JavaScript SDKs

Rust packages use the existing typed graph:

```rust
let output = python::grpc("example-rpc").input(source.handle());
let package = python::package("python").with(source).with(output);
// TypeScript: ts::grpc().input(source.handle()) in ts::package("typescript").
```

Here `source` is an `InputProvider<RpcContract>`. A custom provider can publish the
same owned contract; output plugins do not consume parser-library types.

JavaScript uses the existing `inputProtobuf`, `pluginPython` and
`pluginTypeScript` factories. See the [JavaScript SDK](../../../packages/npm/sdk/README.md)
for `contractOptions.protobuf`, package configuration and regeneration.

## Native call shapes

For a `Store` service, Python emits `StoreStub`, `StoreServicer` and
`add_StoreServicer_to_server`. TypeScript emits `StoreClient`, `StoreServer` and
`StoreService`. These native APIs remain available. Generated facades also offer selectable raw, flat and idiomatic styles over the same native call path. Python exposes separate synchronous and asynchronous stubs and declarations.

```python
import grpc
from service_pb2_grpc import StoreStub
from common_pb2 import Item

with grpc.insecure_channel("127.0.0.1:50051") as channel:
    client = StoreStub(channel)
    item = client.Get(Item(name="example"), timeout=5)
```

```ts
import { credentials } from "@grpc/grpc-js";
import { create } from "@bufbuild/protobuf";
import { StoreClient } from "./service.js";
import { ItemSchema } from "./common.js";

const client = new StoreClient("127.0.0.1:50051", credentials.createInsecure());
client.get(create(ItemSchema, { name: "example" }), (error, item) => {
  if (error) throw error;
  console.log(item);
});
```

Use appropriate native credentials for your deployment. These examples use the
local-test transport. Retained source and descriptors support ordinary Poolster
ownership, customization, regeneration and release metadata.

## Boundaries

The [Protobuf conformance requirements](../../specifications/protobuf-conformance.md)
define the planned feature-set target for all ten languages. The current outputs
do not claim full ts-proto parity.

The [implementation audit](protobuf-conformance.md) records the remaining
requirements, current implementation evidence and historical defect fixes.

Message-only inputs and independent type, binary, JSON, client, server and public
reflection surfaces are supported. Protobuf editions remain unsupported. Proto2
groups/extensions, unknown-field policies and the well-known types have dedicated
conformance fixtures. Python imports built-in `google.protobuf` runtime modules;
custom files under that reserved namespace are rejected.

Native gRPC supports all four streaming modes. Optional gRPC-web adapters support
unary and server-streaming calls; Twirp supports unary calls. Unsupported adapter
methods fail explicitly before network I/O. Compression, browser text framing and
other protocol variants must not be inferred from binary-adapter tests.

## Generation controls

JavaScript passes protocol controls through `contractOptions.protobuf.generation`:

```js
pluginTypeScript({
  style: "flat",
  contractOptions: {
    protobuf: {
      generation: {
        transports: ["grpc", "grpcWeb", "twirp"],
        unknownFields: "preserve",
        maxMessageBytes: 16 * 1024 * 1024,
        maxDepth: 100,
        emitClients: true,
        emitServers: true,
      },
    },
  },
});
```

Rust uses `RpcGenerationOptions` with `.options(options)` on the output plugin.
CLI recipes set `generation` on the `grpc` plugin. Defaults preserve the native
message ABI and descriptor identities. A disabled helper does not remove native
runtime methods from message values. Resource limits apply to generated codecs
and adapters; directly invoking runtime APIs uses that runtime's own policies.

Batching requires explicit single/batch method and field mappings. Request scopes
keep caches isolated, clone cached responses and propagate cancellation; no batch
relationship is inferred from method names. Opt-in immutable views, value adapters
and reactive helpers preserve the underlying native message and transport APIs.
Their target-specific boundaries and executed gates are recorded in the audit.

Portable naming, file/import policy and selective-codec controls are implemented.
The [current audit](protobuf-conformance.md) assesses all 70 pinned options and
separates native equivalents, target-specific exceptions and historical defects.
No complete-profile or full ts-proto parity claim is made yet.

TypeScript exports a Node package root and an isolated `/browser` entry for
messages and HTTP adapters. Alternate value models use `<Message>ValueCodec`;
use its `clone` method to retain unknown wire fields. Ordinary object copies do
not retain hidden provenance. Combining alternate representations with external
`importMappings` fails before emission; native external mappings remain supported.

See the language pages for target-specific limits and the
[earlier verification record](../../verification-results/protobuf-native-2026-10-12.json).
These additions are published in `0.5.0-alpha.2`.
