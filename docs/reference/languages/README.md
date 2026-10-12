# Generated language capabilities

Choose a language to see **what Poolster currently generates**, contract by contract:

| Language | Capability page | Native protocol outputs beyond HTTP/GraphQL |
| --- | --- | --- |
| TypeScript / JavaScript | [TypeScript](typescript.md) | Kafka events; gRPC; source-resolved Arazzo workflows |
| Rust | [Rust](rust.md) | Kafka events |
| Go | [Go](go.md) | Protobuf gRPC clients/server interfaces, including streaming; Kafka events |
| Python | [Python](python.md) | Kafka events; gRPC |
| Java | [Java](java.md) | Kafka events |
| C# | [C#](csharp.md) | Kafka events |
| PHP / Symfony | [PHP](php.md) | Kafka events; Symfony bundles the PHP SDK and DI integration |
| Ruby | [Ruby](ruby.md) | Kafka events |
| Swift | [Swift](swift.md) | Kafka events; native macOS checks, Linux unverified |
| Elixir | [Elixir](elixir.md) | Kafka events; required nonempty declared keys/headers and asynchronous offset commits |

Updated **12 October 2026** against the unreleased checkout; each language page
states its own audit date. These pages describe
implemented output, not the already published alpha.1 release. Language support does
not mean every contract, schema feature or runtime option is implemented.

All ten event outputs target the bounded plaintext JSON Kafka subset, not every
AsyncAPI broker or feature. PHP cannot detect duplicate wire header names because
its native client collapses them into a map. Elixir rejects empty declared keys/header
values and schedules native offset commits; an acknowledgement is not proof of a
persisted commit. See the [Kafka comparison](../outputs/kafka-conformance.md).

## Read the feature tables

Each page records contract support, runtime behavior, generated API styles, file layout
and concrete gaps. Responsibilities matter:

- **Built-in:** Poolster emits the behavior into the generated package. Configuration
  may still be required; the page states defaults where relevant.
- **Native/delegated:** the selected runtime library owns the behavior. A broker client's
  retry mechanism is not Poolster's HTTP retry policy.
- **Application-owned:** supply a driver, callback, headers or explicit orchestration.
  This is an extension point, not an implemented automatic feature.
- **Unsupported:** no maintained output or configuration for that capability.
- **Subset/experimental:** supported only within the stated contract and transport limits.

“Idempotency” means different things across contracts. HTTP key generation does not
implement server deduplication; GraphQL mutations do not inherit the HTTP policy; Kafka
retries do not imply exactly-once processing. Look at the **specific contract** row.
Similarly, static models, decoder checks, schema validation and scalar codecs are
separate capabilities.

## Compare raw, flat and idiomatic

| Style | Intended shape | Important boundary |
| --- | --- | --- |
| Raw | Operation function with explicit execution context/transport | Some HTTP backends do not emit this as a separate public API |
| Flat | Bound client method retaining the operation name | Target casing and async conventions differ |
| Idiomatic | Resource/group facade or native module organization | HTTP grouping uses tags/path policy; GraphQL grouping uses operation kind or explicit mappings |

The pages show actual generated call shapes rather than imposing identical APIs on
all languages. Where a style is unsupported, it is marked explicitly. GraphQL's
`idiomatic`/`grouped` settings and HTTP's `namespaced` settings are different
contract-specific options. Events, gRPC and workflows use their own native surfaces;
there is no universal style switch covering every contract.

HTTP examples identify an operation/tag/parameter fixture. GraphQL examples identify
named operation documents and, where used, explicit group mappings. Package names,
constructor configuration and operation signatures depend on the supplied contract.
Examples of different styles are alternatives, not one package exposing every facade
simultaneously.

## Evidence and maintenance

These pages link generator code, runtime templates and focused tests. Existing test
records are evidence for the stated subset; a documentation audit does not create new
runtime passes. The [support matrix](../../plugin-support-matrix.md) tracks broad
coverage and verification; the [SDK specification](../../specifications/generated-sdk.md)
and [language plugin specification](../../specifications/language-plugins.md) describe
implementation targets, which must not be mistaken for current support.

When changing a generator, update its language page for every affected contract:
API style, result/error policy, replay/acknowledgement, timeout/cancellation, security,
validation and emitted files. Keep unsupported and delegated behaviors visible.
