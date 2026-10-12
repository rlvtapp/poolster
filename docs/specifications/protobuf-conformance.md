# Protobuf output conformance requirements

**Version 1 · 12 October 2026 · planned target for all ten SDK languages.**

Poolster uses ts-proto's feature set as a reference for its own Protobuf outputs.
This is a requirements document, not a claim of current parity. Poolster owns the
input contracts and source renderers; native libraries may provide codecs and
transport. Invoking another source generator does not satisfy the owned-renderer
requirement. The existing Go subprocess route remains available during migration.

This extends the [generated SDK](generated-sdk.md) and
[language plugin](language-plugins.md) specifications. Current support remains in
the [matrix](../plugin-support-matrix.md) and [Protobuf guide](../reference/outputs/protobuf.md).

The [12 October implementation audit](../reference/outputs/protobuf-conformance.md)
assesses every requirement and the local option inventory. Full profile certification remains pending; the audit separates the historical
generation defects from current implementations and executed verification.

## Reference and scope

Reference pinned on 12 October 2026:

- [ts-proto feature documentation](https://github.com/stephenh/ts-proto/blob/d484e1ed15f68d5f1c21e8647834216a5bc82fce/README.markdown#supported-options).
- [Protobuf language semantics](https://protobuf.dev/programming-guides/proto3/).
- [ProtoJSON semantics](https://protobuf.dev/programming-guides/json/).

The option baseline is **ts-proto v2.12.4**, commit
`d484e1ed15f68d5f1c21e8647834216a5bc82fce`. The
[machine-readable inventory](protobuf-reference.json) records every option and
its source SHA-256. Changes require a reviewed specification revision. Independently
generated oracle fixtures and runtime versions must also be pinned before parity
can be claimed. Protocol semantics take precedence over historical defaults.

All ten languages MUST eventually implement the portable requirements below.
TypeScript-specific syntax needs a language-native equivalent, not identical code.
Every upstream option MUST map to a requirement, an adapter or a documented
exception. No option may disappear from the audit because it is inconvenient.

An unavailable runtime or unfinished implementation is **unsupported**, not
**not applicable**. Framework-specific features can be not applicable outside
that framework, with a reason. Semantic substitutions require equivalent tests.

## Conformance profiles

| Profile | Required scope | Applies to |
| --- | --- | --- |
| Messages | W01–W08, M01–M04, G01–G05 | All ten languages; standalone messages without services |
| JSON | J01–J03 plus Messages | All ten languages |
| Native RPC | R01–R05 plus Messages | All ten languages; native gRPC client and server APIs |
| Advanced | A01–A04 | Every applicable target and adapter; remaining gaps must be listed |

Profiles permit staged delivery. A Messages or Native RPC pass is not full
feature-set parity. A full parity claim requires all applicable profiles and the
complete pinned option audit. No language is certified by this document today.

## Portable requirements

MUST requirements are release gates for the relevant profile. An explicit
generation option can disable emitted functionality, but its enabled form must
still pass conformance. Defaults and option conflicts MUST be documented.

New configuration belongs under the Protobuf contract namespace. Existing recipes
and other protocol defaults MUST remain compatible; an intentional breaking change
needs a versioned migration and regression evidence.

| ID | Requirement | Acceptance evidence |
| --- | --- | --- |
| W01 | Support proto2 and proto3 scalar wire kinds with exact signed/unsigned integer ranges, floating-point special values, strings and bytes. Lossy numeric conversions must fail explicitly. | Boundary-value vectors decoded by independent implementations in both directions. |
| W02 | Preserve presence separately from values: absent, explicitly set default, required, optional, implicit defaults and message presence. | Decode/encode and construction tests for each presence case; invalid required messages diagnosed. |
| W03 | Preserve recursive/nested messages, repeated values, maps, packed/unpacked encodings and merge behavior. | Cross-language fixtures with empty and populated collections, duplicate occurrences and recursive values. |
| W04 | Enforce oneof exclusivity through a native tagged alternative or validated equivalent. | Construction checks and wire cases with multiple alternatives; no silently inconsistent public state. |
| W05 | Preserve enum numbers, aliases, unknown numeric values and configured public representations. | Future enum values survive binary roundtrip; naming and JSON policies tested separately. |
| W06 | Define unknown-field retention/discard policy and provide a retention mode. | New-schema → old-schema → new-schema roundtrip retains unknown data when enabled. |
| W07 | Support proto2 extensions and legacy groups; preserve custom options and expose registered extension access. | Independent fixtures cover registered/unregistered extensions, groups and descriptor retention. |
| W08 | Reject malformed/truncated input with bounded decoding and actionable errors. Editions require a separately declared, tested version capability. | Malformed vectors, size/depth limits, invalid references and unsupported syntax tests. |
| M01 | Provide native message construction, partial construction and default initialization without shared mutable defaults. | Compile/type-check invalid examples where supported; mutation-isolation tests elsewhere. |
| M02 | Expose binary encode/decode and independent emission controls for codecs, types, clients and servers. A service-free schema MUST generate a usable messages package. | Clean consumers compile messages-only, codec-only, client-only and server-only configurations. |
| M03 | Cover well-known types, including Timestamp, Duration, wrappers, Empty, Any, Struct, Value, ListValue and FieldMask. Native convenience mappings must preserve presence and precision. | Nanosecond, timezone, negative-duration, wrapper-presence and Any registry fixtures. |
| M04 | Offer explicit representations for large integers, bytes, maps, enums, optional values and date/time values. Native defaults must be lossless; conversions must be checked. | Configuration-specific compilation and binary/JSON roundtrips with documented bounds. |
| J01 | Expose ProtoJSON parsing/serialization independently from binary codecs and honor wire JSON names. | Shared canonical vectors plus original-name input aliases and custom-name cases. |
| J02 | Define strict/lenient input, unknown names, enum formatting, default emission, null handling and duplicate-field policy. | Invalid/ambiguous input tests and explicit configuration outcomes. |
| J03 | Preserve special-value and well-known-type JSON behavior, including 64-bit values, base64 and Any resolution. | Cross-runtime JSON corpus; expected semantic comparisons, not incidental object order. |
| R01 | Emit native clients, server contracts/registration and service definitions for every streaming direction. | Generated client ↔ independent server and independent client ↔ generated server for unary and three streaming modes. |
| R02 | Preserve metadata/trailers, statuses, details, deadlines, cancellation, credentials and channel lifecycle. | Local-server failures, timeout/cancel tests, authenticated transport and cleanup evidence. |
| R03 | Expose native asynchronous/stream APIs with backpressure and explicit end-of-stream/error behavior; document synchronous alternatives. | Slow producer/consumer, cancellation during streams and bounded buffering checks. |
| R04 | Provide injectable transport/context and runtime request, response and error interception. Keep these distinct from generation-time plugin hooks. | Ordering, error propagation and stream interception tests with a substituted transport. |
| R05 | Supply raw, flat and idiomatic operation surfaces using the shared SDK definitions, with one execution path and preserved RPC identity. | Each style compiles, sends equivalent requests and preserves metadata/errors; native low-level interfaces remain available. |
| G01 | Split messages and services semantically, support native namespaces, stable imports and public entry points, and honor configured source-size budgets. | Large-schema compilation, deterministic layout and clean installed-package tests. |
| G02 | Resolve names and collisions deterministically, with configurable casing, prefixes/suffixes, file suffixes and import mappings. | Reordered input/plugin tests; collisions either resolve predictably or fail before emission. |
| G03 | Expose owned descriptors, fully qualified type identity, opt-in reflection/registries and companion-plugin symbol contracts. | Any resolution and a downstream plugin consume the emitted registry/symbols without reparsing sources. |
| G04 | Support import roots, imported-file emission policy, comments/deprecation, generation metadata and identical configuration semantics across Rust, CLI and npm. | Imported/mapped package consumers and entry-point equivalence tests. |
| G05 | Reuse typed planning, optional blocks, provenance/revisions, ownership, customization and regeneration. Unsupported pairs succeed empty with a skipped report; invalid supported input/options fail. | Provider substitution, transformed-contract selection, stale-block diagnostics, regeneration and edit-conflict tests. |

Retries MUST NOT be inferred from a method name. If offered, require an explicit
idempotency policy and preserve deadlines/cancellation. This is an additional
Poolster runtime requirement, not an asserted ts-proto feature.

## Advanced adapters and option coverage

| ID | Planned requirement | Applicability and evidence |
| --- | --- | --- |
| A01 | Additional transport adapters: browser gRPC-web and Twirp. | Each declared target needs its own compile/runtime suite and advertised streaming boundary. Never relabel native gRPC as either transport. |
| A02 | Framework adapters and service metadata. | TypeScript tracks grpc-js, nice-grpc and NestJS integration; other languages use explicitly selected native frameworks. Symfony extends PHP rather than duplicating its codec. |
| A03 | Opt-in batching and request-scoped caching. | Test association/order, errors, isolation and cancellation; require a declared batch mapping and avoid accidental application-level caching. |
| A04 | Target conveniences: immutable models, custom scalar/value adapters and reactive stream surfaces. | Native equivalents or reasoned exceptions; codecs must remain wire compatible. |

The upstream option audit MUST include these families, including alternative
values and conflicting combinations:

| Family | Reference option identifiers |
| --- | --- |
| Representation | `forceLong`, `useJsTypeOverride`, `env`, `useOptionals`, `oneof`, `useMapType`, `useReadonlyTypes`, `useNullAsOptional`, `useDate`, `useDuration`, `useMongoObjectId` |
| Enums and names | `unrecognizedEnum*`, `removeEnumPrefix`, `stringEnums`, `enumsAsLiterals`, `snakeToCamel`, `useSnakeTypeName`, `typePrefix`, `typeSuffix`, `lowerCaseServiceMethods` |
| Emission | `onlyTypes`, `outputEncodeMethods`, `outputEncodeIncludeTypes`, `outputDecodeIncludeTypes`, `outputJsonMethods`, `outputPartialMethods`, `outputClientImpl`, `outputServices`, `outputExtensions` |
| JSON and presence | `protoJsonFormat`, `useJsonName`, `useJsonWireFormat`, `useNumericEnumForJson`, `emitDefaultValues`, `usePrototypeForDefaults`, `initializeFieldsAsUndefined`, `noDefaultsForOptionals`, `disableProto2Optionals`, `disableProto2DefaultValues` |
| Metadata and packaging | `outputSchema`, `outputTypeAnnotations`, `outputTypeRegistry`, `unknownFields`, `exportCommonSymbols`, `outputIndex`, `emitImportedFiles`, `M…`, `fileSuffix`, `importSuffix`, `comments`, `annotateFilesWithVersion` |
| Runtime and framework | `context`, `useContextDataloaders`, `useAbortSignal`, `useAsyncIterable`, `returnObservable`, `rpcBeforeRequest`, `rpcAfterResponse`, `rpcErrorHandler`, `metadataType`, `nestJs`, `addGrpcMetadata`, `addNestjsRestParameter` |
| JavaScript compatibility | `esModuleInterop`, `globalThisPolyfill`, `bigIntLiteral` |

The pinned definitions contain **70 options**. The table groups the original
review inventory; the machine-readable baseline is authoritative and also includes
`constEnums`, `useExactTypes`, and `useJsonTimestamp`. Each must remain in the audit.
Legacy compatibility modes must be named and opt-in; they must not silently weaken
the protocol-correct default. JavaScript-only syntax controls can be not applicable
elsewhere, while their portable purpose still needs an equivalent where relevant.

## Per-language rollout

Every language MUST receive the same requirement inventory. A mature external
runtime does not certify the Poolster renderer. States below describe current
pipeline availability, not individual requirement passes.

| Language | Current Protobuf starting point | Required follow-up |
| --- | --- | --- |
| TypeScript / JavaScript | Partial, Poolster-owned Node gRPC renderer | Audit every ID; complete profiles; browser/framework adapters separately. |
| Python | Owned renderer, sync/aio declarations, checked DTOs and typed immutable views | Complete broad profile certification; retain independent native evidence. |
| Go | Poolster-owned native renderer and checked DTOs; legacy generator route separate | Complete broad profile certification; retain independent interoperability. |
| Rust | Planned | Owned rendering, native package/runtime selection, every portable profile. |
| Java | Planned | Owned rendering, native package/runtime selection, every portable profile. |
| C# | Planned | Owned rendering, native package/runtime selection, every portable profile. |
| PHP | Planned | Owned rendering, native package/runtime selection, every portable profile. |
| Ruby | Planned | Owned rendering, native package/runtime selection, every portable profile. |
| Swift | Planned | Owned rendering and every portable profile; verify each advertised platform. |
| Elixir | Planned | Owned rendering and every portable profile using native modules/stream semantics. |

Symfony follows PHP and adds an independently tested integration adapter. It is
not an eleventh codec language. No additional .NET alias plugin is required.

For each language, maintain one row per requirement and audited option with:
**tested / partial / unsupported / not applicable**, selected configuration,
native API example, limitation, fixture, pinned dependencies and evidence link.
Not applicable requires a specific rationale. Unrun/ignored tests cannot earn a
tested status. Add these records to the existing language capability pages and
summarize them in the support matrix when implementation lands.

## Implementation order and completion gate

- [x] Freeze the 70-option reference inventory and pinned independent oracle corpus.
- [x] Audit current TypeScript/Python/Go outputs against every requirement ID; [results and gaps](../reference/outputs/protobuf-conformance.md). The 70-option immutable upstream baseline is pinned.
- [x] Add the owned Go renderer and Messages/JSON implementation in TypeScript/Python/Go. Broad profile certification remains separate.
- [ ] Roll Messages and JSON through Rust, Java, C#, PHP, Ruby, Swift and Elixir.
- [ ] Complete Native RPC in all ten languages, including package/server interoperability.
- [ ] Implement and verify applicable Advanced adapters; record every remaining exception.
- [ ] Publish the per-language evidence matrix and reconcile all recipe/SDK entry points.

Each batch MUST compile/install generated packages, execute local protocol servers,
test malformed inputs and unsupported features, substitute providers and regenerate.
Use pinned upstream schemas and independent runtimes as oracles. Compare decoded
semantics unless a fixture explicitly requires canonical bytes; valid Protobuf
encodings need not have identical byte ordering. No self-roundtrip-only parity claim.

Run the full workspace suite at integration gates. Record initial failures,
successful retries, ignored tests and opt-in native results separately. Upstream
generators may produce test oracles but MUST NOT become production generation
dependencies. Keep OpenAPI, GraphQL and AsyncAPI behavior covered by regressions.
