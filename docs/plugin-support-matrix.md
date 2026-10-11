# Poolster plugin support matrix

Audited **11 October 2026**, against the unreleased alpha.2 implementation in this checkout.

For runtime behavior and raw/flat/idiomatic examples, see the
[per-language capability pages](reference/languages/README.md).
Manifests still use `0.5.0-alpha.1`; the additions below are unreleased alpha.2
work. This matrix describes implemented generation, not parser availability.
The [generated SDK specification](specifications/generated-sdk.md) defines target
requirements for capabilities, API styles, source layout, naming and formatting;
it is separate from current implementation coverage.
For remaining work, start with the [native pipeline backlog](reference/inputs/native-pipelines.md#remaining-work).

GraphQL React Query, Vue Query, SWR, Zod, Faker, MSW and Cypress integrations now
have separate generated compilation and runtime checks. These checks establish
the documented GraphQL subset, rather than every existing HTTP plugin option.
See the [integration guide](reference/outputs/graphql-integrations.md) for API and limitations.

GraphQL clients are implemented for all ten SDK languages: TypeScript, Rust, Go,
Python, PHP, Java, C#, Ruby, Swift and Elixir. All use the existing language plugin packages through Rust, CLI and npm entry points.
All ten SDKs provide opt-in distinct-connection SSE subscriptions, direction-specific
runtime scalar callbacks and experimental multipart deferSpec=20220824 incremental
delivery. TypeScript incremental generation is raw only; scalar value boundaries
vary by language. All selections remain fixed operation documents. See the
[advanced capability guide](reference/outputs/graphql-capabilities.md).
See the [language guides](reference/README.md) and [verification](verification/verification.md#advanced-graphql-checks-10-october-2026)
for tested features and per-language limits. These additions are not yet published.

✅ = implemented for the stated contract; — = no bundled generation support.
A check does not imply support for every feature of a specification. Target-specific
limitations still apply. GraphQL is a separate contract even when its transport is HTTP.

## Output plugins and accepted inputs

| Output plugin | Generated output | OpenAPI / HTTP `Api` | GraphQL operations | Protobuf RPC | AsyncAPI events | Arazzo workflows | Cap’n Proto |
| --- | --- | :---: | :---: | :---: | :---: | :---: | :---: |
| TypeScript | HTTP SDK/models; GraphQL client; Kafka message/client package; workflow runner | ✅ | ✅ | — | ✅ | ✅ | — |
| Zod | HTTP and selected GraphQL validators | ✅ | ✅ | — | — | — | — |
| Faker | HTTP and selected GraphQL fixtures | ✅ | ✅ | — | — | — | — |
| MSW | HTTP and GraphQL mock handlers | ✅ | ✅ | — | — | — | — |
| Cypress | HTTP and GraphQL request/interception helpers | ✅ | ✅ | — | — | — | — |
| React Query | TypeScript query/mutation hooks | ✅ | ✅ | — | — | — | — |
| Vue Query | TypeScript query/mutation hooks | ✅ | ✅ | — | — | — | — |
| SWR | TypeScript query/mutation hooks | ✅ | ✅ | — | — | — | — |
| Go | HTTP/GraphQL SDK; Protobuf gRPC; Kafka JSON client | ✅ | ✅ | ✅ | ✅ | — | — |
| Rust | HTTP/GraphQL SDK; Kafka JSON client | ✅ | ✅ | — | ✅ | — | — |
| Python | HTTP SDK; synchronous GraphQL client | ✅ | ✅ | — | — | — | — |
| Java | Java HTTP SDK; GraphQL client | ✅ | ✅ | — | — | — | — |
| C# | C# HTTP SDK; GraphQL client | ✅ | ✅ | — | — | — | — |
| PHP | PHP HTTP SDK; GraphQL client | ✅ | ✅ | — | — | — | — |
| Symfony | HTTP integration; PHP GraphQL SDK, HttpClient transport and bundle | ✅ | ✅ | — | — | — | — |
| Ruby | Ruby HTTP SDK; GraphQL client with RBS | ✅ | ✅ | — | — | — | — |
| Elixir | Elixir HTTP SDK; GraphQL client | ✅ | ✅ | — | — | — | — |
| Swift | Swift HTTP SDK; GraphQL client | ✅ | ✅ | — | — | — | — |
| Rust CLI | HTTP and fixed-operation GraphQL command-line client | ✅ | ✅ | — | — | — | — |
| TypeScript CLI | HTTP and fixed-operation GraphQL command-line client | ✅ | ✅ | — | — | — | — |
| Postman | HTTP collections/examples; GraphQL collections and environments | ✅ | ✅ | — | — | — | — |
| Terraform | Terraform provider scaffolding | ✅ | — | — | — | — | — |

The TypeScript output package contains several generators: `ts::sdk()` /
`ts::types()` for HTTP, `ts::graphql()` for `GraphqlOperations`, `ts::asyncapi()`
for Kafka `EventOperations`, and `ts::workflow()` for `WorkflowOperations`.
These are generators within one plugin package, not separate language packages.
Go similarly provides its HTTP SDK and `go::grpc()` for `RpcContract`. Rust provides
`rust::graphql()` for GraphQL and its existing HTTP generators in one language package.

GraphQL companions consume the selected `GraphqlClient` contract, including
operation shapes and actual symbols; HTTP companions retain their HTTP contracts.
GraphQL hooks currently use single-file output and fixed query/mutation selections;
subscriptions and infinite-pagination helpers are unsupported. Zod/Faker runtime
scalar mappings support primitive wire types. Cypress mutation helpers require
explicit enablement. Input parsing alone does not establish companion support.
A Kafka package contains message types **and** producer/consumer code. Generation
writes files without contacting a broker; a broker is needed for runtime use and
integration tests. Broker-independent models-only packages are also available; their
coverage is separate from event transports, as described below.

See [output migration coverage](verification/output-contract-migration.md) for typed HTTP
selection, block consumption, finalization and compatibility boundaries.

## Shared model packages

All ten language packages expose `shared_models` through Rust, the CLI `shared-models`
recipe plugin and JavaScript `mode: 'models'`. GraphQL schema input objects and AsyncAPI
payload, header and key shapes can generate packages without operation documents or a
broker connection. See [models-only usage](reference/outputs/shared-models.md).

| Output | Common model subset | Additional boundaries |
| --- | --- | --- |
| TypeScript | Objects, references, scalar fields, lists, presence and nullability | Structural types; no runtime schema validator |
| Rust, Swift | Same common subset | Scalar/list root aliases and closed string enums; nullable named roots, literals, unions, intersections and unsupported defaults are diagnosed |
| Go, C# | Same common subset | Same native alias/enum subset; also reject typed additional-property bags |
| Java | Same common subset | Same native alias/enum subset; also rejects additional-property objects and records over 200 fields |
| Python | Same common subset | String, integer, boolean and null literals/enums; floating/complex literals, defaults, unions, intersections and negation are rejected |
| PHP | Same common subset | Homogeneous string/integer backed enums; literals, defaults, unions, intersections and negation are rejected |
| Ruby, Elixir | Same common subset | Enums, literals, defaults, unions, intersections and negation are rejected |

Constraints and extension data remain in `.poolster/model-contract.json`; retaining
those schemas does not mean every serializer enforces every JSON Schema constraint.
Blocks carry stable entity IDs and parent contract revisions. Names are resolved before
emission, independently of plugin execution order. Complete empty collections are valid;
partial collections and unresolved model references are rejected.

Python, PHP, Ruby and Elixir object decoders check required fields, nullability,
primitive kinds, lists and named references before decoding. These checks do not
establish full JSON Schema validation or validate every direct model constructor.
Focused models-only checks passed **21 tests** across those four backends, including
four native runtime tests for recursive references, omission versus explicit null and
malformed nonnullable nulls. Those native tests are ignored by default unless their
language toolchains are available; they were explicitly executed. This count is
separate from the historical HTTP corpus and full-workspace test totals.

Rust, Go, Java, C# and Swift passed **20 focused tests: 0 failed and 0 ignored** with
`--include-ignored`. All five compiled and executed generated neutral and rich null-only
model packages, checking recursive references, required/optional nullability, scalar/list
root aliases and closed enum wire values. Provider substitution and unsupported literal
diagnostics passed for every backend. Their native tests are ignored in the default suite
and were explicitly executed with local toolchains and cached dependencies. Serializer
checks do not establish general JSON Schema runtime validation; constraints remain in
the semantic manifest. Go and Swift direct-construction APIs still have limited controls
for optional nullable field presence. See the [detailed boundaries](reference/outputs/shared-models.md#boundaries).

Event transports are implemented for **TypeScript, Go and Rust**, separately from
models-only coverage. CLI and JS event generation assemble event-derived shared models;
Rust plugin authors select them with `asyncapi(provider).using_models(models.handle())`.
Stable message IDs bind payload/header/key roles, and stale contract revisions are rejected.
The direct TypeScript factory retains its legacy standalone rendering path for compatibility.

All three have generated compilation/runtime and real local Kafka round-trip checks.
TypeScript uses KafkaJS 2.2.4/AJV 8.17.1, Go franz-go 1.18.1/jsonschema v5.3.1,
and Rust rust-rdkafka 0.38.0/jsonschema 0.33.0. The broker is pinned Redpanda 25.3.11.
Generation does not contact a broker. Tests provision unique topics; clients do not
provision broker infrastructure. See [Kafka usage](reference/outputs/kafka.md).

The supported transport subset is plaintext Kafka, static topics, one JSON message per
operation and nonnullable UTF-8 string keys. Go requires closed string-header objects;
Rust accepts supported open string-header objects, with runtime values validated.
Supported schema keywords/model shapes vary by target. Go uses the supported draft-07
vocabulary, rejects external schema references, and retains the shared-model shape
limits above. Its runtime rejects duplicate/null headers, invalid UTF-8 and tombstones.
Go disables automatic commits and synchronously commits after successful handlers;
decoding, handler or commit errors stop consumption. This is at-least-once processing,
with no generated invalid-message skip or dead-letter policy. Rust requires explicit
acknowledgement. Native documents and model manifests remain available. Go's facade,
runtime, operations and models occupy separate files in one `models` package; the
configured name sets its module path. See [Go package details](reference/outputs/kafka.md#go-package-details).

More brokers, declared security/authentication, dynamic channels, traits, replies,
broader schema coverage and event transports for the other seven languages remain work.
This is not full AsyncAPI support. Broker/delegate/native tests overlap ordinary suites
and are reported separately in the [verification log](verification/verification.md).

## GraphQL capability chart

✅ = implemented within the documented boundary; — = unsupported.
Incremental delivery is experimental multipart `deferSpec=20220824`. Subscriptions
use distinct-connection SSE. These columns describe separate opt-in capabilities.

| Output | Queries / mutations | SSE subscriptions | Runtime scalar codecs | Incremental delivery |
| --- | :---: | :---: | :---: | :---: |
| TypeScript SDK | ✅ | ✅ | ✅ | ✅ Raw only |
| Rust SDK | ✅ | ✅ | ✅ | ✅ |
| Go SDK | ✅ | ✅ | ✅ | ✅ |
| Python SDK | ✅ | ✅ | ✅ | ✅ |
| PHP SDK | ✅ | ✅ | ✅ | ✅ |
| Symfony bundle | ✅ | — | ✅ PHP container configuration | — |
| Java SDK | ✅ | ✅ | ✅ | ✅ |
| C# SDK | ✅ | ✅ | ✅ | ✅ |
| Ruby SDK | ✅ | ✅ | ✅ | ✅ |
| Swift SDK | ✅ | ✅ | ✅ | ✅ |
| Elixir SDK | ✅ | ✅ | ✅ | ✅ |
| React Query / Vue Query / SWR | ✅ | — | Via selected SDK client | — |
| Zod / Faker / MSW / Cypress | ✅ Selected operation helpers | — | Primitive wire mappings only | — |
| Rust CLI / TypeScript CLI | ✅ | — | — | — |
| Postman | ✅ | — | — | — |

Scalar callback value domains and abstract selection requirements vary by language;
see [language boundaries](#graphql-language-boundaries) and the
[advanced capability guide](reference/outputs/graphql-capabilities.md). No WebSocket,
multiplexing, reconnect/replay or newer ID-based incremental dialect is claimed.

## Input providers

| Input format | Parsing / inspection | Usable bundled output |
| --- | :---: | --- |
| OpenAPI | ✅ | HTTP output plugins above |
| GraphQL SDL / local introspection JSON + operation documents | ✅ | All ten SDK languages through Rust API, CLI and npm entry points |
| Protobuf proto2/proto3 | ✅ | Go messages and gRPC clients/server interfaces |
| AsyncAPI 2.6 / 3.0 / 3.1 | ✅ | TS/Go/Rust Kafka for the supported 3.0/3.1 subsets; ten-language models-only subsets |
| Arazzo 1.0.0 / 1.0.1 / 1.1.0 | ✅ | TypeScript sequential runners with resolved local OpenAPI sources |
| Cap’n Proto | ✅ | — |

The npm input wrappers expose parsing/inspection and GraphQL client generation
through the existing language packages. Other native package
pipelines are exposed through Rust / CLI; general typed Node hooks remain open. CLI recipes currently
select one input source; Rust plugins can compose multiple typed contracts.

## GraphQL checks

- [x] Validate schema and operation documents together.
- [x] Selection-specific results, aliases, fragments and concrete abstract alternatives.
- [x] Preserve nullability, optional presence, lists and input defaults.
- [x] Generate variables and query/mutation functions with TypeScript Fetch and Rust Reqwest transports.
- [x] Represent GraphQL errors and partial results explicitly.
- [x] Compile generated packages for all ten SDK languages and execute against a local GraphQL server.
- [x] Modular GraphQL source layouts, deterministic filenames and regeneration checks.
- [x] Separate TypeScript subscription capability with an injected async-iterable transport.
- [x] Bundle distinct-connection graphql-sse subscription transports for all ten SDKs.
- [ ] Bundle WebSocket, multiplexing or reconnect/replay support.
- [x] Separate input/output TypeScript scalar mappings.
- [x] Separate input/output Rust scalar mappings for validated self-contained wire types.
- [x] Direction-specific runtime scalar callbacks with target-specific value boundaries.
- [x] Local introspection JSON and quoted full-file schema/operation imports.
- [ ] Remote introspection and selective named imports.
- [x] Experimental `@defer` / `@stream` with multipart deferSpec=20220824.
- [ ] Newer ID-based incremental dialects and custom executable directives.
- [x] GraphQL support in Zod, Faker, MSW, Cypress and query-hook outputs (current checkout; unreleased).
- [x] GraphQL generation through the npm configuration engine (current checkout; unreleased).

Unsupported output packages succeed with warnings and a structured skipped-plugin
report. An all-skipped run leaves existing files untouched. Invalid input and
unsupported features in an otherwise supported pipeline still fail explicitly.

## Verification and release checks

Kafka shared-model batch: **966 workspace tests passed, 0 failed, 192 ignored**.
Generated-package compilation and local Kafka runtime checks passed for TypeScript, Go
and Rust. Strict workspace Clippy, formatting and the source-layout audit passed.
The full npm run passed 88 tests with 2 gated skips; after the final fixes, the refreshed
addon passed 10 focused checks and the explicitly enabled Kafka broker test. Ignored
tests and overlapping explicit runs are not added to workspace totals. See the
[scoped verification record](verification-results/kafka-shared-models-2026-10-11.json).

Symfony GraphQL follow-up: **4 Symfony checks passed**, including compiled container
and local-server execution; PHP/CLI regressions **158 passed, 0 failed, 26 ignored**;
npm SDK **79 passed, 0 skipped**. See the [scoped verification record](verification-results/graphql-symfony-2026-10-10.json).


Latest advanced GraphQL workspace check: **895 passed, 0 failed, 170 ignored**.
The npm SDK suite passes **78 tests, 0 failures, 0 skips**; release configuration
passes **4 tests**. Formatting and Clippy for all changed crates and targets with
warnings denied pass. Explicit generated compilation and local-server probes pass
for all ten SDK languages. Ignored tests are not passes. See the
[batch verification record](verification-results/graphql-advanced-2026-10-10.json).
The earlier alpha.2 workspace check recorded 816 passes and 143 ignored tests.
Selected external GraphQL, gRPC, Kafka and workflow integration tests were also
run explicitly and passed; commands and boundaries are in [verification](verification/verification.md).

The pre-output-migration frozen-build **205 specs × 10 HTTP SDK targets** sweep
is complete: **2,050 effective passes**, including deterministic regeneration,
after audited infrastructure retries. PHP used native syntax lint and emitted
deprecation warnings in 140 contracts. These results do not establish endpoint
runtime behavior or corpus coverage of the later output migration. The migrated
workspace passed separately; final migrated-binary corpus coverage and clean
installation checks of release artifacts remain open. Original infrastructure
failures and successful retries remain in the audit trail. See
[current verification](verification/verification.md#earlier-alpha2-verification-9-october-2026).

- [x] Existing OpenAPI snapshots and workspace tests pass.
- [x] Provider substitution, typed downstream hooks and regeneration have focused tests.
- [x] GraphQL → TypeScript and Rust generation, clean packaged consumers and local-server execution.
- [x] Protobuf → Go official messages, gRPC clients/server interfaces and all streaming modes.
- [x] AsyncAPI → TypeScript message models and Kafka producer/consumer for the documented subset.
- [x] Arazzo → source-resolved sequential TypeScript runners and local integration tests.
- [x] All six inputs expose whole contracts and optional standard block collections.
- [x] Revision/provenance checks, completeness checks, typed hooks and opt-in deterministic symbol planning.
- [x] Complete the frozen pre-output-migration OpenAPI corpus: 2,050 effective passes.
- [ ] Run the corpus against final migrated binaries.
- [x] GraphQL generation through all ten existing npm SDK language plugin packages.
- [ ] Broaden native protocol features and expose other pipelines/general typed hooks through npm.
- [ ] Cap’n Proto → Rust messages/capability RPC using the official toolchain.
- [ ] Schema-level JSONPath overlays.
- [ ] Audited Forge migration and a tested compatibility matrix.
- [ ] Confirm Cap’n Web scope, then implement and test output.
- [ ] Alpha.2 versioning, final packaging, publication and install verification.

See [native pipelines](reference/inputs/native-pipelines.md) for exact feature limits and concrete
implementation/verification requirements for each follow-up.

## Generation planning (unreleased)

The [CLI planner](cli/plan.md) renders terminal lines or exports JSON/HTML without
loading protocol inputs. [Rust planning](rust/planning.md) exposes the resolved
typed package graph and handler declarations. [Node planning](javascript/planning.md)
exposes JavaScript dependencies and callback names; native nodes are opaque
configuration boundaries. Planning does not predict runtime revisions, block
completeness, emitted files or timings. Execution tracing and a full Node native
graph bridge remain follow-up work.

Go and Python GraphQL boundaries: [Go](reference/outputs/graphql-go.md) · [Python](reference/outputs/graphql-python.md).

## GraphQL language boundaries

All SDK languages provide raw operations, flat client APIs and grouped APIs with
explicit custom group mappings. Grouping follows language conventions: Elixir uses
modules, while object-oriented outputs use client methods/accessors. No schema-field
heuristic invents resource groups.

| Language | Custom scalar fallback | Abstract result variants | Runtime model validation |
| --- | --- | --- | --- |
| TypeScript | Configurable input/output types | Selected concrete alternatives | Static types; optional Zod validators |
| Rust | JSON or configured wire types | Selected concrete alternatives | Serde decoding and presence wrappers |
| Go | JSON or supported mapped types | Requires selected nonnull typename | JSON decoding; required primitives can default to zero |
| Python | Any | Selected typed dictionary alternatives | Static TypedDict annotations |
| PHP | mixed / JSON values | Requires selected typename | Generated model decoding/validation |
| Java | JsonNode | Requires selected typename | Generated record decoding/validation |
| C# | JsonElement | Requires selected nonnull typename | Required presence; nonnull values are not revalidated |
| Ruby | Untyped JSON | Selected variants | Generated model validation and RBS signatures |
| Swift | GraphqlJSON | Requires selected nonnull typename | Codable plus presence/null checks |
| Elixir | JSON terms | Requires selected typename | Generated struct/scalar decoding/validation |

Subscriptions are a separately enabled capability with bundled distinct-connection
graphql-sse transports in all ten SDKs. Runtime callbacks preserve absence and null;
Python/PHP/Ruby remain statically Any/mixed/untyped, and Java/C#/Swift codecs stay
within their JSON value domain. Experimental incremental inputs use a separate
contract and only the tested multipart deferSpec=20220824 dialect. TypeScript's
incremental output is raw only. No bundled WebSocket transport, multiplexing,
reconnect/replay, newer ID-based incremental protocol or dynamic fields are claimed.
See the [advanced guide](reference/outputs/graphql-capabilities.md).
Symfony now generates a PHP GraphQL SDK with Symfony HttpClient transport and DI
bundle configuration for queries/mutations. Its streaming capabilities remain
unsupported. See the [Symfony guide](reference/outputs/graphql-symfony.md).

## GraphQL source organization

All ten SDK client outputs now separate model, operation, client/facade and runtime
code using native module/package conventions. Per-entity files, bounded facade/export
parts and stable filename checks prevent large operation sets from accumulating
in a monolith. The source grouping budget is **128 KiB**; oversized indivisible
declarations are retained with explicit layout diagnostics. This is a grouping
budget rather than a hard limit on all valid GraphQL selections.

Raw, flat and grouped call paths are preserved. Java model imports move from nested
`Client` records to `<package>.models` types. Generated-source customizations using
old paths require migration. See the [layout overview](reference/outputs/graphql-layout.md)
and each language guide for its actual file tree.

Earlier source-layout verification: npm SDK **76 passed, 0 skipped**; CLI **127 passed,
9 ignored**; TypeScript selected GraphQL/integration checks **22 passed**. All ten
languages have generated compilation and local-server checks. Ignored full-suite
tests are recorded separately. That source-layout batch did not rerun the full workspace or the
205-spec HTTP corpus. See the [verification record](verification-results/graphql-source-layout-2026-10-09.json).

## GraphQL collections and executable CLIs

Postman and both CLI output plugins consume native `GraphqlOperations`. Named
queries/mutations retain their fixed documents and variables. Generated Rust and
TypeScript commands accept JSON variables or a variables file, endpoint and
authentication headers; they print envelopes and return nonzero status for errors
or partial results. Postman uses native GraphQL request bodies and editable
environments. These outputs do not include subscriptions, incremental delivery,
GraphQL OAuth login discovery or dynamically selected fields.

See [configuration and usage](reference/outputs/graphql-tools.md). These additions
are unreleased; HTTP generators retain their existing behavior.

The [GraphQL tool verification record](verification-results/graphql-tools-2026-10-10.json)
records generated compilation/runtime probes, actual Newman execution and preserved
HTTP checks. Full-workspace and 205-spec HTTP corpus checks were not rerun for this addition.
