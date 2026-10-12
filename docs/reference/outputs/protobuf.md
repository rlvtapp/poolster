# Protobuf messages and gRPC packages

Poolster accepts `.proto` sources and resolves imports through the Protobuf input
provider. `RpcContract` retains the full descriptor set and source documents.
Services and streaming metadata stay separate from HTTP models.

Python and TypeScript source generation runs inside Poolster in Rust. No external
code generator is invoked. The shared owned wire schema preserves field numbers,
labels, defaults, oneofs, message/enum references and dependency information.
Native libraries perform serialization and gRPC transport in the generated app.
Go retains its existing official-generator integration.

## Choose an output

| Language | Source generation | Runtime | Package structure |
| --- | --- | --- | --- |
| Python | Poolster emits message bindings, typing declarations, stubs and server registration | protobuf 6.33.5, grpcio 1.78.0 | Per-proto `_pb2.py`/`.pyi` and `_pb2_grpc.py`, separate descriptor/runtime modules, one `pyproject.toml` |
| TypeScript | Poolster emits message types/schemas, clients and server interfaces | @bufbuild/protobuf 2.16.0, @grpc/grpc-js 1.14.6 | Per-proto modules under `src/`, separate descriptor registry, one npm manifest/tsconfig |
| Go | Pinned protoc, protoc-gen-go and protoc-gen-go-grpc | protobuf 1.36.11, gRPC 1.83.2 | Official per-proto message/service files, one Go module |

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
`StoreService`. These are gRPC-native APIs, not HTTP/GraphQL raw/flat/grouped styles.

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

These gRPC outputs require at least one service; a messages-only Protobuf output
mode remains follow-up work. Protobuf editions are unsupported. TypeScript rejects groups and extensions;
Python runtime bindings preserve supported proto2 extensions. Target naming and
module collisions are rejected rather than renamed silently. Python custom files
under the reserved `google/protobuf/` runtime namespace are unsupported; imported
well-known runtime types are handled separately. Python typing declarations target
its synchronous client API. TypeScript targets Node gRPC, not browser grpc-web.

See the language pages for target-specific limits and the
[verification record](../../verification-results/protobuf-native-2026-10-12.json)
for compilation/local-server evidence. These additions are unreleased.
