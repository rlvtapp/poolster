# Protobuf conformance audit

**12 October 2026 · current working-tree assessment · requirements version 1.**

TypeScript, Python and Go now have Poolster-owned Protobuf renderers, native codecs,
RPC surfaces and explicit alternate value models. This assessment uses the pinned
[70-option reference](../../specifications/protobuf-reference.json) and the
[current machine record](../../verification-results/protobuf-current-native-audit-2026-10-12.json).
It does not certify full ts-proto parity or every acceptance facet of a profile.

The [original audit](../../verification-results/protobuf-conformance-audit-2026-10-12.json)
and [original pinned-option assessment](../../verification-results/protobuf-pinned-option-audit-2026-10-12.json)
are historical baselines. Their rejected messages-only cases, TypeScript naming
and overload defects, Python async declaration gap, and Go subprocess ownership
gap describe the earlier implementation, not the current capabilities below.

## Current native capabilities

| Area | TypeScript | Python | Go |
| --- | --- | --- | --- |
| Messages and emission | Standalone messages; independent types/binary/JSON/client/server/reflection controls; codec filters | Standalone messages; independent surfaces and codec filters | Owned reflection-capable messages and independent surfaces; legacy subprocess route retained separately |
| Semantics | Native bigint/presence/tagged oneofs; groups/extensions; retained descriptors; bounded codecs | Native integers/HasField/validated oneofs; groups/extensions; private descriptor runtime; bounded codecs | Native integer widths/pointers/oneof wrappers; groups/extensions; bounded native codecs |
| Representations | Explicit checked ValueCodec DTOs for integer, bytes, maps, enums, nullable fields and time values | Typed dataclass DTOs with UNSET/None, checked conversions and native provenance | Typed Values DTOs with checked conversions, pointer presence and cloned native provenance |
| RPC | Native grpc-js overloads and raw/flat/idiomatic facades; stream/cancel/context APIs | Separate synchronous/aio declarations and runtimes; raw/flat/idiomatic surfaces | Native grpc clients/server registration and contexts; raw/flat/idiomatic surfaces |
| Adapters | Browser gRPC-web/Twirp subpaths; NiceGrpc/NestJS; reactive/immutable/value helpers; declared batching | HTTP transport/server adapters; asyncio reactive/immutable/value helpers; declared batching | HTTP adapters, reactive/immutable/value helpers and declared batching |
| Layout and integration | Semantic messages/services split, naming checks, mappings, comments and typed symbols | Native modules, aliases/mappings, comments and typed symbols | Native packages, explicit mappings/naming, comments and typed symbols |

These are implementation capabilities, not a blanket “tested” designation. Python's
latest supplied record reports **5 focused library and 16 integration passes**, with
no failures or ignored cases in those focused gates. Go's supplied record cites
**15 conformance and 2 native passes**, plus fresh warning-free clippy evidence.
TypeScript's final focused snapshot reports **10 conformance and 8 native passes**,
with no failures or ignored cases, including enum/browser/DTO repairs, all four RPC
directions and TLS. The external-mapping/alternate-representation rejection regression
landed after that snapshot and additionally passed from the fresh workspace binary
(one pass, no failures or ignored cases). The [local final log](/private/tmp/poolster-ts-final-completion.log) records
the 18-pass snapshot.
The rebuilt SDK also passed [six independent peer cases](../../verification-results/protobuf-independent-peer-final-2026-10-12.json), covering all four RPC modes in both client/server directions for each language.
Dependency pins, source evidence and prior failed attempts are retained in the
machine record. Temporary local logs are evidence references, not distributable artifacts.

The [final workspace gate](../../verification-results/protobuf-workspace-final-2026-10-12.json)
completed 116 fresh executables: **1,050 passed, zero remaining failures, 239 ignored**.
Three initial PHP GraphQL failures came from a paused test container and passed
on retry; the record retains both attempts without counting repeated passes twice.
The rebuilt JavaScript SDK suite had **97 passes, zero failures, two skipped**.
All-target warning-denied lint passed for the changed entry points and language
plugins, and the layout audit found zero violations among 839 authored files.
Documentation testing completed 24 crate groups containing no executable cases.

## Profile assessment and remaining work

Messages, JSON, Native RPC and Advanced remain **partial as certification profiles**
for all three languages: a runtime subset or individual native equivalent does not
prove every compound acceptance facet. Current focused tests cover substantially
more than the historical baseline. No absent or unrun test earns a tested status.

| Requirements | Current implementation | Remaining acceptance evidence |
| --- | --- | --- |
| W01–W08 | Native wire semantics, presence, collections, oneofs, enums, unknown retention, groups/extensions and bounded decoding | Full independent boundary/merge/malformed corpus and selected-policy permutations |
| M01–M04 | Construction, standalone emission controls, WKT support and checked alternate representations | Clean consumers for all surface combinations; complete WKT precision/presence/Any permutations |
| J01–J03 | Independent canonical ProtoJSON helpers with explicit native per-call policies | Full independent JSON corpus, aliases/duplicates/null/default policies and configuration permutations |
| R01–R05 | Native all-direction RPC, metadata/context/cancel/interception and three operation styles | Complete independent peer directions and stream backpressure/security/lifecycle permutations |
| G01–G05 | Semantic layout, naming/import controls, retained identity/symbol contracts and shared generation options | Installed mapped-package consumers, source budgets and full generation-graph/regeneration conflict evidence |
| A01–A04 | Selected HTTP/framework, batching, immutable/reactive/custom-value adapters | Each adapter's declared streaming boundary and complete isolation/cancellation/error permutations |

Rust, Java, C#, PHP, Ruby, Swift and Elixir still lack Protobuf output. Their portable
requirements remain unsupported; Symfony follows PHP. Completing the three current
renderers cannot certify the all-ten-language specification.

## Native boundaries and audited exceptions

Native APIs take precedence over reproducing TypeScript syntax. Go/Python integer
widths or arbitrary-precision integers, pointer/HasField/UNSET presence, tagged or
validated oneofs, native async iteration and defensive immutable views fulfill the
portable purpose without adopting JavaScript object layouts.

`constEnums`, literal enum declaration syntax, CommonJS interop, bigint literals,
legacy global polyfills and prototype allocation are target-specific controls.
Their absent exact spelling is not a portable wire failure. Native proto2 defaults
remain separate from presence; suppressing native getters or requiredness belongs
to an explicitly named legacy compatibility profile. Unknown enum numbers remain
intact rather than being replaced by one sentinel.

The TypeScript package root targets Node. Browser consumers must use the isolated
messages/browser or HTTP adapter entry points; their runtime import graph needs a
clean browser consumer gate. Alternate DTOs convert explicitly into native messages.
Python/Go mapped external message fields remain native instead of assuming an external
provider supplies Poolster-specific DTO classes. TypeScript native mappings and local
alternate DTOs are supported separately; combining alternate representations with
`importMappings` is rejected before emission because external modules need not export
compatible `Value` types. A future explicit external value-adapter contract is needed
to enable that combination. DTO copies must preserve provenance
through the documented clone API when retaining unknown fields; arbitrary object
reconstruction cannot preserve hidden unknown data.

A missing automatic Mongo ObjectId adapter remains an optional integration gap;
application-owned checked value adapters are available. Fixed native import suffixes
and per-call JSON policies can be equivalent where their portable purpose is covered.
Accepted non-default generation controls must either work or fail explicitly.

## Pinned option assessment

The authoritative inventory contains **70 options**. The following table preserves
all options and the supplying language audits' distinctions. “Implemented” identifies
a control, “native equivalent” its native purpose, and “partial/remaining/unsupported”
a narrower or absent exact control. These labels are separate from profile
certification and must be read with the native-boundary rationale above.

| Reference option | TypeScript | Python | Go |
| --- | --- | --- | --- |
| `M` | implemented control | implemented | implemented |
| `addGrpcMetadata` | native equivalent | native equivalent | native equivalent |
| `addNestjsRestParameter` | implemented control | unsupported | not applicable |
| `annotateFilesWithVersion` | implemented control | implemented | implemented |
| `bigIntLiteral` | native equivalent | not applicable | not applicable |
| `comments` | implemented control | implemented | implemented |
| `constEnums` | implemented control | unsupported | not applicable |
| `context` | native equivalent | native equivalent | native equivalent |
| `disableProto2DefaultValues` | remaining | unsupported | unsupported |
| `disableProto2Optionals` | remaining | unsupported | unsupported |
| `emitDefaultValues` | native equivalent | native equivalent | partial |
| `emitImportedFiles` | implemented control | implemented | implemented |
| `enumsAsLiterals` | implemented control | unsupported | partial |
| `env` | native equivalent | not applicable | not applicable |
| `esModuleInterop` | native equivalent | not applicable | not applicable |
| `exportCommonSymbols` | native equivalent | partial | unsupported |
| `fileSuffix` | implemented control | implemented | implemented |
| `forceLong` | implemented control | partial | partial |
| `globalThisPolyfill` | native equivalent | not applicable | not applicable |
| `importSuffix` | native equivalent | partial | unsupported |
| `initializeFieldsAsUndefined` | native equivalent | unsupported | native equivalent |
| `lowerCaseServiceMethods` | implemented control | implemented | native equivalent |
| `metadataType` | native equivalent | native equivalent | native equivalent |
| `nestJs` | implemented control | unsupported | not applicable |
| `noDefaultsForOptionals` | remaining | unsupported | native equivalent |
| `oneof` | native equivalent | native equivalent | native equivalent |
| `onlyTypes` | implemented control | implemented | implemented |
| `outputClientImpl` | implemented control | implemented | implemented |
| `outputDecodeIncludeTypes` | implemented control | implemented | implemented |
| `outputEncodeIncludeTypes` | implemented control | implemented | implemented |
| `outputEncodeMethods` | implemented control | implemented | implemented |
| `outputExtensions` | native equivalent | native equivalent | partial |
| `outputIndex` | implemented control | implemented | implemented |
| `outputJsonMethods` | implemented control | implemented | implemented |
| `outputPartialMethods` | native equivalent | native equivalent | native equivalent |
| `outputSchema` | implemented control | implemented | native equivalent |
| `outputServices` | native equivalent | implemented | implemented |
| `outputTypeAnnotations` | native equivalent | native equivalent | native equivalent |
| `outputTypeRegistry` | native equivalent | native equivalent | implemented |
| `protoJsonFormat` | native equivalent | native equivalent | partial |
| `removeEnumPrefix` | implemented control | implemented | implemented |
| `returnObservable` | implemented control | partial | native equivalent |
| `rpcAfterResponse` | native equivalent | native equivalent | native equivalent |
| `rpcBeforeRequest` | native equivalent | native equivalent | native equivalent |
| `rpcErrorHandler` | native equivalent | native equivalent | native equivalent |
| `snakeToCamel` | native equivalent | partial | native equivalent |
| `stringEnums` | implemented control | partial | partial |
| `typePrefix` | implemented control | implemented | implemented |
| `typeSuffix` | implemented control | implemented | implemented |
| `unknownFields` | implemented control | implemented | implemented |
| `unrecognizedEnum` | native equivalent | partial | native equivalent |
| `unrecognizedEnumName` | implemented control | partial | native equivalent |
| `unrecognizedEnumValue` | remaining | partial | native equivalent |
| `useAbortSignal` | native equivalent | native equivalent | native equivalent |
| `useAsyncIterable` | native equivalent | native equivalent | native equivalent |
| `useDate` | implemented control | partial | partial |
| `useDuration` | implemented control | partial | partial |
| `useExactTypes` | native equivalent | native equivalent | native equivalent |
| `useJsTypeOverride` | implemented control | unsupported | not applicable |
| `useJsonName` | native equivalent | native equivalent | partial |
| `useJsonTimestamp` | native equivalent | partial | partial |
| `useJsonWireFormat` | native equivalent | partial | partial |
| `useMapType` | implemented control | partial | partial |
| `useMongoObjectId` | remaining | unsupported | unsupported |
| `useNullAsOptional` | implemented control | partial | native equivalent |
| `useNumericEnumForJson` | native equivalent | native equivalent | partial |
| `useOptionals` | implemented control | partial | native equivalent |
| `usePrototypeForDefaults` | native equivalent | unsupported | unsupported |
| `useReadonlyTypes` | implemented control | implemented | implemented |
| `useSnakeTypeName` | implemented control | implemented | implemented |

Individual explanations, exact source-audit assessments, pinned dependencies and
verification checkpoints are in the current machine record. The historical
`useContextDataloaders` alias remains covered by declared request-scoped batching;
it is not an additional option in the immutable 70-option inventory.

## Completion gate

Run and record the complete workspace integration suite after the final focused
gates, including the subsequent TypeScript external-mapping rejection regression. Preserve initial failures and retries separately.
Extend independent peer and installed-package evidence rather than treating
self-roundtrip success as parity. Reconcile each requirement and every option with
executed evidence before upgrading profile status, and roll portable output through
the remaining seven languages.
