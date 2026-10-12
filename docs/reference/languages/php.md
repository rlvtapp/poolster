# PHP output capabilities

Audited against the current repository on **2026-10-12**. This describes implemented generated APIs, not the intended output specification or a promise about a previously published package. Examples show alternatives from different generation styles; do not paste all alternatives into one scope.

PHP 8.2+, Composer. HTTP injects PSR-18/PSR-7/PSR-17; portable GraphQL defaults to PHP HTTP streams.

## Contracts

| Contract / input | Generated output today |
| --- | --- |
| HTTP / OpenAPI | SDK, typed models and target-specific runtime helpers |
| GraphQL | Selection-specific client, variables/results and HTTP runtime; optional advanced transports |
| RPC / Protobuf | No service/client/server generator in this language; shared models are a separate subset |
| Events / AsyncAPI | Shared models and Kafka JSON producer/consumer package; Symfony can wrap the PHP package |
| Workflows / Arazzo | No workflow runner |
| Capabilities / Cap’n Proto | No message/capability RPC generator |
| Shared models | Neutral model package; supported schema subset and explicit rejection of unsupported shapes |

See the [cross-language support matrix](../../plugin-support-matrix.md) for the exact shared-model subset. An input parser being available does not imply this language exports its protocol runtime.

## HTTP features

| Feature | Responsibility / current behavior |
| --- | --- |
| Raw | No separately emitted standalone-function HTTP API style; use native client operations or the injected transport |
| Flat | SDK direct operation methods |
| Idiomatic | SDK namespaced resource facade (`SdkClientStyle::Namespaced`); direct methods remain available |
| Errors / partial results | SDK HTTP status errors and native transport/decoding failures; no GraphQL-style partial-result envelope |
| Retry / backoff | SDK HTTP `maxRetries` defaults to 2 retries (3 attempts), with 250 ms initial / 8000 ms maximum delay. Replay safety depends on generated eligibility and method/key guards. Retry headers are handled by the SDK. |
| Idempotency | SDK declared OpenAPI idempotency policy can generate/preserve a key; retry gating uses supported headers. This does not guarantee server-side exactly-once behavior |
| Timeout / cancellation | HTTP timeout/cancellation belongs to the injected PSR-18 driver; `forCall` can select a configured driver. Portable GraphQL has a positive timeout constructor option (default 30 s); no native async cancellation-token API. |
| Auth / security | SDK API-key/default-header configuration; credentials and transport TLS policy are application-owned. Separate OAuth helpers are opt-in, not an automatic consequence of selecting this language |
| Pagination / streaming | SDK lazy Generator helpers for declared cursor, page, offset/limit and same-origin URL continuation; target-specific restrictions and diagnostic files. HTTP SSE behavior depends on the PSR-18 driver. |
| Validation / codecs | Typed immutable models and generated decoding checks; not full JSON Schema validation. |
| Middleware / hooks | Application-owned transport wrapper can change execution; observer lifecycle hooks are notifications, not request replacements |

### Optional OAuth and webhook helpers

`oauth()` and `webhooks()` are separately registered HTTP companion plugins. OAuth emits client-credentials/token runtime support; selecting the main SDK does not automatically enable refresh policy or attach an OAuth transport to GraphQL. Webhooks emits a raw-body Standard Webhooks v1 signature verifier. It does not generate a hosted webhook server, subscription broker or exactly-once handler. Applications own secret rotation, routing and replay persistence.

Evidence: [OAuth emitter](../../../crates/plugins/php/src/oauth.rs), [webhook emitter](../../../crates/plugins/php/src/webhooks.rs). Native crypto/HTTP dependencies vary by target and must be present in the consuming application.

### What HTTP styles generate

Assume `GET /contacts/{id}`, `operationId: getContact`, first tag `contacts`, one required string path parameter `id`, and a typed JSON success model. Naming conflicts or extra parameters change the signature. The examples use an already constructed client.

```php
// Flat: operationId getContact, required string path parameter id.
$contact = $client->getContact($id);
// Namespaced (idiomatic navigation): first tag contacts.
$contact = $client->contacts()->get($id);
```

**Raw:** unsupported as a separate generated HTTP function surface in this plugin. Flat and namespaced are configured through the HTTP client-style entry point; GraphQL’s `raw`/`flat`/`idiomatic` setting is a different contract-specific option.

## GraphQL features

| Feature | Responsibility / current behavior |
| --- | --- |
| Raw / flat / idiomatic | SDK supports all three; idiomatic defaults to operation-kind groups; explicit groups provide resource names |
| Errors / partial results | SDK envelope retains data, errors and extensions. Explicit success/data helper rejects application errors; transport and malformed-response failures remain separate |
| Retry / backoff | No built-in GraphQL automatic retry policy. Application-owned transport policy must account for mutation replay |
| Idempotency | No inferred mutation key/deduplication behavior; requires server-specific/application policy |
| Timeout / cancellation | Portable stream timeout or injected callable transport policy. Subscription iterators are synchronous; no CancellationToken/Task API. |
| Auth / security | Endpoint and headers / injected transport; application-owned credentials, TLS and refresh policy |
| Pagination | No automatic connection/cursor traversal; generate the selected operation and pass cursor variables yourself |
| Subscriptions | Opt-in distinct-connection graphql-sse; not GraphQL-over-WebSocket |
| Incremental delivery | Opt-in experimental multipart `deferSpec=20220824`; not a claim of every defer/stream protocol variant |
| Selection / presence | Operation documents determine result fields; omission and explicit null remain distinct. Abstract variants require selected `__typename`; dynamic runtime field selection is unsupported |
| Custom scalars | `mixed`; per-client scalar callbacks can map custom scalars. |
| Runtime validation | Target-specific decoding/presence checks; operation validation happens in the input provider. See the [capability guide](../outputs/graphql-capabilities.md) for limits |

### What GraphQL styles generate

Assume `query ReadUser($id: ID!) { readUser(id: $id) { name nickname } }`; optional variable fields omitted from these snippets. `variables` is the generated `ReadUserVariables` value where used. Explicit grouping is configured as `groups: { users: { read: 'ReadUser' } }`; GraphQL schema fields alone do not infer that group. A mutation `RenameUser` follows the same style, with `mutation`/`Mutation` replacing the query group.

```php
// Raw (namespace follows package configuration)
$response = \Poolster\Test\readUser($client, $variables);
// Flat
$response = $client->readUser($variables);
// Idiomatic: operation-kind grouping
$response = $client->query()->readUser($variables);
// Explicit group users.read -> ReadUser
$response = $client->users()->read($variables);
$data = $response->requireData();
```

Selecting raw removes the convenience client facade; it does not remove typed variables/results or expose arbitrary runtime selections. Inspect partial data/errors before invoking the illustrated success helper when partial data is useful.

### Configuration

CLI: `poolster generate schema.graphql --input-format graphql --operation operations.graphql --language php --output generated` selects the pipeline. Recipes select the language output and its contract options. In the JS SDK use `pluginPhp({ contracts: { graphql: { style: 'flat' } } })`; operation documents belong to the GraphQL input configuration. In the Rust composition SDK use `php::graphql(Some(input.handle())).flat()` / `.raw()` / `.idiomatic()` as appropriate. These select generated API shape, not the SDK implementation language.

## Shared-model contract features

| Feature | Current behavior |
| --- | --- |
| API styles | Model declarations only; raw/flat/idiomatic client styles do not apply |
| Naming / provenance | Shared model plan resolves stable entity names; emitted model manifest retains parent contract/revision |
| Presence / nullability | Supported neutral schema shapes preserve the documented target subset; this does not make every protocol wire encoding interchangeable |
| Validation / codecs | Target-native model encoding/decoding; unsupported schema constructs reject generation. No protocol transport or universal validator is implied |
| Retry / idempotency / auth / timeout | Not applicable to a model-only package |
| Broker/RPC behavior | Not emitted merely because AsyncAPI or Protobuf supplied the models |

See [shared-model output](../outputs/shared-models.md) and the matrix for per-language enum/alias/additional-property restrictions.

## Kafka event features

PHP 8.2+ packages pin **php-rdkafka 6.0.5** and **Opis JSON Schema 2.6.0**. Install the native extension before Composer. CLI recipes select `asyncapi`; JavaScript uses `pluginPhp()` with an AsyncAPI input. Rust uses `php::asyncapi(Some(input.handle())).using_models(models.handle())`.

| Feature | Generated behavior |
| --- | --- |
| API style | Native operation classes: `SendOrderEvent::send()` and `ReceiveOrderEvent::subscribe()`. Event packages do not accept HTTP/GraphQL style options. |
| Retry / idempotence | Native idempotent producer with `acks=all` and 30 s delivery deadline. Separate calls are not deduplicated; timeout can occur after publication. |
| Acknowledgement | One pending record; explicit `ack()` or `run($handler)` after success. Synchronous commit; failed handlers leave the offset uncommitted. |
| Malformed records | Strict JSON, supported Draft-07 schema validation and payload/key/header UTF-8 checks. Decoding failure poisons the subscription; close and reopen to replay. |
| Cancellation | `cancel()` or a predicate, checked between polls of at most 250 ms. No cross-thread wakeup guarantee. |
| Bindings | Static topics, generated client/group IDs and broker overrides. Conflicting fixed group overrides fail. |
| Auth / broader protocols | Plaintext only. TLS/SASL, transactions, schema registry and other brokers are unsupported. |

For package `acme/orders` and operations `sendOrder`/`receiveOrder`:

```php
use Acme\Orders\Events\{KafkaDefaults, SendOrderEvent, ReceiveOrderEvent};
use Acme\Orders\Models\{OrderCreatedPayload, OrderCreatedHeaders, OrderCreatedKey};

$runtime = KafkaDefaults::connect();
$payload = OrderCreatedPayload::fromArray(['id' => '42', 'quantity' => 1]);
$headers = OrderCreatedHeaders::fromArray(['trace' => 'trace-1']);
$key = OrderCreatedKey::from('order-42');
SendOrderEvent::send($runtime, $payload, $headers, $key);
$subscription = ReceiveOrderEvent::subscribe($runtime);
try {
    $record = $subscription->poll(1000);
    if ($record !== null) {
        processOrder($record->payload); // application handler
        $record->ack();
    }
} finally {
    $subscription->close();
    $runtime->close();
}
```

The sample assumes a fixed consumer group; otherwise supply one to `subscribe()`. Topics must already exist. Missing keys and empty keys remain distinct. Runtime files live in `src/Events/`, one operation per file, shared models in `src/Models/`, with one Composer manifest and retained native document/provenance. PHP's native library exposes headers as an associative map, so duplicate wire header names may be collapsed before validation; detection of that case is unavailable.

See [Kafka output](../outputs/kafka.md) and [conformance](../outputs/kafka-conformance.md).

## Files, naming and regeneration

HTTP Composer package: Client, model/error/resource classes and runtime support. GraphQL: Composer-loaded `src/Graphql.php` entry, split runtime/client plus `Models/`, `Operations/`, `Methods/`, groups and balanced traits.

Sources are split at operation/model/runtime boundaries. GraphQL layout targets 128 KiB per source; indivisible declarations exceeding the budget produce `.poolster/source-layout-diagnostics.json` rather than incompatible silent splitting. Regeneration removes obsolete unchanged owned files and preserves unrelated user files. Customizations tied to previous monolithic paths need retargeting.

HTTP and GraphQL use different transport APIs: PSR-18 for HTTP, a callable or native streams for portable GraphQL. A callable adapter is application-owned, while the Symfony output adds an actual integration bundle.

## Gaps and evidence

RPC, workflows and capabilities still need language-specific runnable packages. GraphQL retry/idempotency policy, automatic pagination and dynamic typed selections are application work or future features, not enabled by the HTTP runtime. Shared-model support does not imply runtime schema-validation parity.

Kafka generated packages have been compiled and executed against the local broker; see the linked conformance evidence. Native generated-code tests can be ignored by the ordinary workspace suite until their toolchains are explicitly enabled.

- [HTTP implementation](../../../crates/plugins/php/src/http/)
- [HTTP operation tests](../../../crates/plugins/php/src/operation_tests.rs)
- [GraphQL implementation](../../../crates/plugins/php/src/graphql/)
- [Generated GraphQL compilation/runtime tests](../../../crates/plugins/php/src/graphql/tests.rs)
- [Shared-model implementation and tests](../../../crates/plugins/php/src/shared_models/)
- [GraphQL usage and layout guide](../outputs/graphql-php.md)

## Symfony output

Symfony is a framework-specific PHP output plugin, not an additional language. Its HTTP integration emits a bundle/configuration around the PHP transport and Symfony services. Its GraphQL integration **is implemented**: Composer package, bundle configuration and a Symfony HttpClient callable adapter inject the portable PHP client. Configure endpoint, headers, timeout and the selected HttpClient service, then autowire the generated Client.

| GraphQL feature | Symfony behavior |
| --- | --- |
| Raw / flat / idiomatic / explicit groups | Reuses the PHP generated API styles |
| Errors, partial data, typed selections and input presence | Reuses PHP SDK behavior |
| Retry / idempotency | Bundle adds no mutation retry or deduplication; supplied Symfony HttpClient may have its own policy |
| Scalar codecs | Configure constructor callbacks with PHP service configuration/compiler pass |
| Subscriptions / incremental delivery | Not enabled by the Symfony GraphQL generator; use the portable PHP output for these transports |

Evidence: [bundle generator](../../../crates/plugins/symfony/src/graphql/mod.rs), [emitted configuration/usage](../../../crates/plugins/symfony/src/graphql/readme.md.tmpl), [generated bundle runtime tests](../../../crates/plugins/symfony/src/graphql/tests.rs), [HTTP integration](../../../crates/plugins/symfony/src/http.rs).

Symfony Kafka uses `symfony::asyncapi()` to produce a self-contained PHP SDK and DI bundle. Register the generated `PoolsterKafkaBundle`, configure its `<package>_kafka.brokers` setting and inject `KafkaRuntime`. Application worker services own consumer loops and shutdown; DI registration starts no consumers. Symfony models in this pipeline are bundled PHP models, not a separate Symfony models-only output.
