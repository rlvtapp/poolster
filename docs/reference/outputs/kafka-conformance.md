# Kafka conformance baseline

The Kafka generators share wire and safety requirements, while retaining native APIs. They do not promise identical retry, acknowledgement or cancellation behavior. The executable baseline is [the case catalog](../../../scripts/fixtures/kafka-conformance.json), run by [the conformance runner](../../../scripts/kafka-conformance.py).

## Compare generated Kafka outputs

This is a **runtime capability and policy comparison**, not an assertion that the clients behave identically. All ten SDK languages generate usable producer/consumer packages for the documented plaintext JSON subset. Their schema/model subsets differ; none has generated TLS/SASL, schema-registry or transaction support. No implementation supplies automatic application-handler retries or exactly-once side effects.

| Output plugin | Producer retry / idempotence | Consumer acknowledgement / failed handler | Cancellation | Operation bindings | Generated layout |
| --- | --- | --- | --- | --- | --- |
| [TypeScript](../languages/typescript.md) | KafkaJS native retries; no exposed generated idempotence/transaction policy | KafkaJS `eachMessage` commit policy; explicit invalid-message callback may skip; no generated manual-ack API | Native consumer stop/disconnect and client lifecycle; no generated signal for pending consumption | Fixed topic/group/client IDs; conflicting group overrides reject | Shared model modules, operation modules, runtime and compatibility `events.ts` facade |
| [Go](../languages/go.md) | franz-go native defaults/options; no generated application-send retry loop or exactly-once promise | Auto-commit off; synchronous commit only after successful handler; failure stops, reopen for replay | `context.Context` bounds send/consume; handler owns respecting its context | Fixed topic/group/client IDs; conflicting group overrides reject | Shared models and `events_operation_*.go`, codec/runtime in one `models` package; one `go.mod` |
| [Rust](../languages/rust.md) | librdkafka native producer behavior; 5 s message/queue timeout settings; no generated idempotence/transaction policy | Explicit `ack(metadata)`; application omits ack on failure, drops/reopens for replay | Drop pending receive future or use Tokio timeout; sends may still reach broker | Fixed topics/generated group/client IDs; lower-level `subscribe` accepts explicit bindings | `models/`, per-operation modules, runtime/codec and bounded exports; one Cargo package |
| [Python](../languages/python.md) | kafka-python retries default **0**, opt in with `retries=N`; idempotence disabled | Explicit `record.ack()` or handler-success commit; pending record blocks next poll; malformed input poisons subscription | `threading.Event` between polls up to 250 ms; send wait has timeout | Fixed topic/group/client IDs; conflicting group overrides reject | Model modules, per-operation modules, bounded client mixins, runtime/codec; installable typed package |
| [Java](../languages/java.md) | Native producer idempotence and `acks=all`; retries within 30 s delivery deadline | Partition-ordered manual ack or handler-success commit; pending batch drains before next poll; malformed input invalidates subscription | Native `cancel()` wakes poll; bounded poll duration; send-future cancellation does not retract publication | Fixed topic/group/client IDs; conflicting group overrides reject | Shared records, per-operation event classes, separate runtime/codec/subscription; one Maven package |
| [C#](../languages/csharp.md) | Native producer idempotence and `acks=all`; 30 s message / 10 s request timeout | Explicit `record.Ack()` or handler-success commit; pending record blocks next poll; malformed input poisons subscription | `CancellationToken`, polling up to 250 ms; send cancellation stops waiting | Fixed topic/group/client IDs; conflicting group overrides reject | Shared `Models/`, per-operation `Events/` classes and separate runtime/codec/subscription; one `.csproj` |
| [PHP](../languages/php.md) | Native idempotent producer, `acks=all`, 30 s delivery deadline | One pending record; synchronous manual or handler-success commit; failure replays after reopen | `cancel()` or predicate between polls up to 250 ms | Fixed topic/group/client IDs; conflicting group overrides reject | Shared PHP models, per-operation classes, runtime/codec/subscription; one Composer package |
| [Ruby](../languages/ruby.md) | Native idempotent producer, `acks=all`, 30 s delivery deadline | One pending record; synchronous manual or handler-success commit; failed handler replays after reopen | Cooperative cancel between polls up to 250 ms; poll/ack/close stay on creator thread | Fixed topic/group/client IDs; native IDs tested; conflicting group overrides reject | Shared model modules, per-operation modules and runtime/codec/subscription; one gem |
| [Swift](../languages/swift.md) | Native idempotent producer and `acks=all` | One pending record; synchronous manual or handler-success commit; failed handler replays after reopen | Task cancellation interrupts receive waits; producer lifetime is separate | Fixed topic/group/client IDs; conflicting group overrides reject | Shared models, operation files, runtime/codec/subscription and C bridge; one SwiftPM package |
| [Elixir](../languages/elixir.md) | Native `required_acks=-1`, three retries; no generated idempotence guarantee | Partition-ordered manual or handler-success acknowledgement schedules asynchronous brod commit; immediate close may replay | Cancel stops subscriber and wakes pending poll; finite close | Fixed topic/group/client IDs; native IDs tested; 64 bounded native client slots | Shared structs, operation modules, runtime/codec/subscription/callback; one Mix package |

Native producer idempotence protects eligible transport retries, not separate application calls. It does not establish end-to-end deduplication or transactional processing. Keep C#, PHP, Ruby, Swift and Elixir Kafka and HTTP/GraphQL in separate output packages. Per-language pages give call examples, dependencies and exact restrictions.

## Compare test evidence

**T** = linked maintained probe tests the case; **P** = tested native acknowledgement policy, different from manual acknowledgement; **—** = missing complete evidence for an implemented transport; **N** = Kafka transport not implemented. Models-only support is separate. This grid is generated from the catalog and checked by the runner's unit tests. A T does not erase language-specific semantics or schema boundaries.

<!-- kafka-evidence:start -->
| Case | TypeScript | Go | Rust | Python | Java | C# | PHP | Swift | Ruby | Elixir |
| --- | :---: | :---: | :---: | :---: | :---: | :---: | :---: | :---: | :---: | :---: |
| `package` | T | T | T | T | T | T | T | T | T | T |
| `roundtrip` | T | T | T | T | T | T | T | T | T | T |
| `outgoing-validation` | T | T | T | T | T | T | T | T | T | T |
| `incoming-validation` | T | T | T | T | T | T | T | T | T | T |
| `utf8` | T | T | T | T | T | T | T | T | T | T |
| `acknowledgement` | P | T | T | T | T | T | T | T | T | P |
| `handler-replay` | — | T | T | T | T | T | T | T | T | T |
| `cancellation` | — | T | T | T | T | T | T | T | T | T |
| `bindings` | — | — | T | T | T | T | T | T | T | T |
| `provenance` | T | T | T | T | T | T | T | T | T | T |
| `regeneration` | T | T | T | T | T | T | T | T | T | T |
<!-- kafka-evidence:end -->

Print the same grid with `python3 scripts/kafka-conformance.py --matrix`. Implemented transport cells link to source probes and executable suites; pending languages have no executable Kafka adapter. Successful adapters are execution evidence at suite level; this is not per-assertion instrumentation. For example, Go tests malformed UTF-8 through the generated codec, while TypeScript, Rust, Python, Java, C#, PHP, Swift, Ruby and Elixir also inject malformed bytes through a broker producer. The TypeScript `handler-replay` gap concerns reopening the same group after a failed handler; native KafkaJS behavior still applies.

## Run the baseline

```sh
# Validate adapter registration and evidence paths without running a broker.
python3 scripts/kafka-conformance.py --check

# Generation, shared-model provenance and regeneration tests.
python3 scripts/kafka-conformance.py

# Build and execute generated packages, including the real broker probes.
python3 scripts/kafka-conformance.py --native --output /tmp/kafka-conformance.json

# A specific native implementation.
python3 scripts/kafka-conformance.py --languages go --native
```

Start the pinned broker from `crates/plugins/typescript/tests/fixtures/kafka-broker.compose.yaml` before native execution. Its default address is `127.0.0.1:29092`. Install the pinned dependencies/toolchains described in [Kafka output](kafka.md). TypeScript requires `POOLSTER_KAFKA_NODE_MODULES` and `POOLSTER_KAFKA_BROKER`; Python requires `POOLSTER_KAFKA_PYTHON`. Java uses `JAVA_HOME` and optional `POOLSTER_MAVEN`. Ruby and Elixir require `POOLSTER_RUBY_EVENTS_RUNNER` / `POOLSTER_ELIXIR_EVENTS_RUNNER`, each taking the generated package path and `wire` or `broker` mode. Their malformed-record injector requires `POOLSTER_JAVA` (Java 17+) and `POOLSTER_KAFKA_JAVA_CLASSPATH` (Kafka clients 3.9.1 and SLF4J API 1.7.36). Native dependencies may need network access even though the outer Cargo invocation is offline. The runner does not install toolchains or start the broker. Native probes provision isolated test topics and consumer groups on the configured local broker.

PHP and Symfony use the [checked-in PHP harness](../../../crates/plugins/php/tests/fixtures/kafka/README.md). Swift requires `POOLSTER_SWIFT_KAFKA_PREFIX` pointing to its pinned librdkafka 2.12.0 installation; current native evidence is macOS only. C# accepts `POOLSTER_DOTNET` and Go accepts `GOCACHE` for locally installed toolchains and writable caches.

The runner executes existing generated-package tests, including tests normally ignored by the workspace suite. It writes separate logs, passed/failed/ignored test counts and adapter outcomes. Missing required environment variables are **skipped**, with a nonzero exit status. A successful command with zero matching tests fails the adapter. Some Rust adapters include both outer harness and nested generated-crate test counts. These counts must not be added to workspace-suite totals. Generation tests may list ignored native tests; those do not become native evidence. Native suites must execute their tests without ignores.

Tests run sequentially because Cargo and native toolchains share dependency caches and some probes own their topic lifecycle. Do not run multiple copies of the runner concurrently. Timeout defaults to 900 seconds per adapter and can be configured with `--timeout`.

## What the cases mean

The common cases cover generated compilation, broker round trips, outgoing and incoming validation, malformed UTF-8 in all three wire fields, acknowledgement policy, failed-handler replay, cancellation, operation bindings, selected-model provenance and regeneration. The catalog records evidence sources and executable suite IDs for each language.

A suite success is **adapter-level evidence**, not instrumentation of every individual assertion. Case mappings should change when probes change. `tested` means the linked maintained probe asserts the behavior; `native-policy` identifies language-specific policy; `gap` identifies incomplete test evidence. A gap does not automatically mean the implementation lacks the feature. Source-path validation alone cannot prove runtime correctness; native execution is required.

TypeScript uses KafkaJS acknowledgement policy and an explicit invalid-message callback that may skip a record. Go commits after a successful handler. Rust exposes acknowledgement to application code; its replay case deliberately omits acknowledgement and reopens the same group. Python, Java, C#, PHP, Ruby and Swift expose manual acknowledgement plus handler helpers. Elixir checks partition order but its acknowledgement schedules native brod asynchronous commit; it does not confirm offset persistence when it returns, and immediate close may replay an acknowledged record. Failed-handler replay and bounded cancellation are tested. Native retries do not imply application-level deduplication. Cancelling a wait does not retract a published record.

Malformed-byte probes deliberately inject records below generated schema/type validation. Rust's probe explicitly skips test-owned malformed offsets after verifying rejection to test all three fields in one topic. This is not an automatic poison-record recovery promise.

### Remaining test evidence gaps

The current catalog leaves TypeScript's dedicated pending-consume cancellation and operation-specific client ID assertions open. Its broker probe tests fixed-group conflicts, validation and explicit invalid-record reporting. Go's broker probe does not directly assert operation client IDs/fixed-group conflict behavior. These are test gaps, separately from the transport features implemented by the generators. Rust's explicit no-ack replay, cancelled receive future and malformed UTF-8 in payload/key/header are now exercised against the local broker. Python, Java, C#, PHP, Swift, Ruby and Elixir have maintained assertions for each listed common case, within their documented native policies.

## Boundaries before broader deployment

| Capability | Current boundary | Work needed |
| --- | --- | --- |
| TLS/SASL | Generated AsyncAPI security configuration is unsupported. Some native clients accept application-supplied options; that is not tested generated security support. | Define security bindings, reject unsupported mechanisms explicitly, test authenticated local brokers and certificates. |
| Schema registry | JSON schemas are bundled locally; no registry negotiation, Avro/Protobuf registry encoding or schema IDs. | Specify wire formats and registry contracts, pin implementations and test compatibility/error behavior. |
| Transactions | No generated transactional producer/consumer workflow. Native producer idempotence varies by language. | Define transaction lifecycle, fencing and abort semantics and test failure/recovery with a real broker. |
| Poison records | Validation errors surface according to each native API; no universal dead-letter queue or automatic retry policy. | Specify explicit handling hooks and offset safety, then test later records cannot accidentally acknowledge an earlier failure. |
| Operation bindings | Static topics, one JSON message per operation, supported string headers and optional string keys. Elixir explicitly requires nonempty declared keys and header values (`minLength >= 1`) because its native protocol library collapses null and empty bytes; optional declared keys are unsupported. | Expand the input/output support matrix only with tested dynamic channels, replies, multiple messages and binding versions. |

Ruby pins rdkafka 0.28.0 and json_schemer 2.5.0. Elixir pins brod 4.6.3, Jesse 1.8.2 and Jason 1.4.4; its validator supports the documented Draft 7-equivalent subset through Draft 6 validation and rejects conditionals and formats. Both reject unsupported schema keywords, duplicate/null headers, tombstones and malformed UTF-8; their broker probes verify poison-record replay without advancing past the failure.

This baseline is a release gate for the supported plaintext JSON subset. It is not a declaration of full AsyncAPI or production-environment compatibility.
