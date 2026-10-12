# Models without a client

Generate a model package when you need data shapes rather than operations or a transport.
GraphQL schema inputs expose input-object blocks. AsyncAPI exposes payload, header and
key shapes without requiring a broker or routing configuration.

## CLI recipe

```json
{
  "input": { "format": "asyncapi", "path": "messages.yaml" },
  "output": { "path": "generated" },
  "packages": [
    { "language": "typescript", "path": "ts", "plugins": [{ "name": "shared-models" }] },
    { "language": "rust", "path": "rust", "plugins": [{ "name": "shared-models" }] }
  ]
}
```

Run `poolster generate --config poolster.json`. Select any of the ten SDK languages.
For GraphQL, change the input format and path; operation documents are optional in
models-only mode. If supplied, operations are still validated.

## JavaScript SDK

```js
import { generate } from '@relevate/poolster';
import { inputAsyncApi, pluginTypeScript, pluginRust } from '@relevate/poolster-plugins';

await generate({
  input: { path: 'messages.yaml', plugin: inputAsyncApi() },
  output: 'generated',
  plugins: [
    pluginTypeScript({ mode: 'models' }),
    pluginRust({ mode: 'models' }),
  ],
});
```

SDK mode remains the default. Models-only packages reject client styles, transports,
subscriptions and client auxiliaries. Models and clients may use distinct package paths.

## Rust plugins and blocks

```rust
let messages = InputProvider::<poolster_inputs::asyncapi::contracts::MessageBlocks>::new(
    registry, "asyncapi", "messages.yaml",
);
let shapes = poolster_inputs::asyncapi::shared_model_blocks(Some(messages.handle()));
let output = poolster::ts::shared_models(None).input_shapes(shapes.handle());
let package = poolster::ts::package("models").with(messages).with(shapes).with(output);
```

Custom providers can publish `Blocks<ModelBlock>` directly and select them with
`shared_models(Some(provider.handle()))`. Rich model providers publish
`Blocks<SchemaModelBlock>` instead. Existing rich `Blocks<Schema>` are accepted through
`.input_schemas(handle)`. None of these routes constructs an HTTP API or GraphQL operations.

Whole event contracts retain channels, actions, topics and broker bindings. Model extraction
is opt-in and leaves those contracts intact. Extracted shapes link back to message IDs and
keep the native contract's revision. Planning requires a complete collection and closed
named references; a complete empty collection remains valid.

All declarations, including anonymous nested shapes, reserve names before deterministic
resolution. IDs stay stable across revisions. Generated model symbols are published as
`GeneratedModels<Language>` for downstream plugins.

## Boundaries

The common working subset is objects, named references, scalar fields, lists, field presence
and nullability. Supported aliases, enums and literal types vary by language. Unsupported
defaults, unions and other shapes produce diagnostics rather than silently losing their
semantics. See the [support matrix](../../plugin-support-matrix.md#shared-model-packages).

For the dynamic language backends:

| Backend | Enum/literal support | Rejected model features |
| --- | --- | --- |
| Python | String, integer, boolean and null scalar literals/enums | Floating/complex literal values, defaults, unions, intersections and negation |
| PHP | Homogeneous string/integer backed enums | Literals, mixed/non-backed enums, defaults, unions, intersections and negation |
| Ruby, Elixir | No enum/literal declarations yet | Enums, literals, defaults, unions, intersections and negation |

For Rust, Go, Java, C# and Swift, scalar/list root aliases and closed string enums
are supported alongside objects and recursive references. All five reject nullable
named roots, literals, unions and intersections; unsupported defaults are diagnosed.
Field nullability remains supported, including required nullable fields and optional
null-only fields. Go and C# reject typed additional-property bags. Java rejects
additional-property objects and records with more than 200 fields.

The five native backends passed **20 focused tests: 0 failed and 0 ignored**, with
`--include-ignored` explicitly running their compiler/runtime checks. Each backend
passed provider substitution, unsupported-literal diagnostics, neutral model runtime
checks and rich null-only model runtime checks. Generated packages compiled and
exercised recursive references, missing versus explicit null, malformed nonnullable
fields, scalar/list aliases and closed enum wire values. Native checks are ignored
in the default suite because they require installed toolchains and cached dependencies.
These results are separate from the HTTP corpus and full-workspace totals.

Generated object decoders in these four backends check required fields, nullability,
primitive kinds, lists and named references. Direct model construction is not a
replacement for a schema validator. Packages contain model serialization support and
language manifests, with no HTTP or GraphQL transport dependencies.

`.poolster/model-contract.json` retains structural schemas, constraints, extension data,
entity IDs, names and provenance. A model package does not imply JSON Schema runtime
validation or broker support. TypeScript declarations have no runtime validator.
GraphQL model blocks describe input objects, not arbitrary query-result selections.
Native serializer checks enforce the tested presence/nullability and representable
wire types; retaining constraints in the manifest does not enforce every JSON Schema
constraint at runtime. Go and Swift currently preserve optional nullable field presence
through decoding; their direct-construction APIs do not yet offer complete presence
controls for every model.

TypeScript, Go, Rust, Python, Java and C# Kafka event clients can consume these shared models through
`using_models`. CLI and JavaScript SDK AsyncAPI generation compose them automatically.
See [Kafka clients](kafka.md) for the supported transport subset and examples.
