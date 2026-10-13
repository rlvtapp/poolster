<h1><img src="docs/assets/banner.svg" alt="Poolster: one contract, lots of possibilities, your code" width="100%"></h1>

> [!IMPORTANT]
> **Poolster is under active development. OpenAPI generation is ready to use; GraphQL and other native pipelines are expanding.**
> See the [support matrix](docs/plugin-support-matrix.md) for tested outputs and limitations. GraphQL and AsyncAPI/Kafka pipelines are published in [0.5.0-alpha.2](https://github.com/rlvtapp/poolster/releases/tag/v0.5.0-alpha.2).

**One contract. Lots of possibilities. Your code.**

Poolster turns API contracts into clients, models, hooks and tools. Choose input
and output plugins, then regenerate when your contract changes.

## Quick start

Install the published **0.5.0-alpha.2** prerelease explicitly. It is available on
[npm](https://www.npmjs.com/package/poolster),
[crates.io](https://crates.io/crates/poolster/0.5.0-alpha.2) and
[PyPI](https://pypi.org/project/poolster/0.5.0a2/).

```sh
npm install -D @relevate/poolster@alpha @relevate/poolster-plugin-typescript@alpha
```

```js
import { generate } from '@relevate/poolster';
import { pluginTypeScript } from '@relevate/poolster-plugin-typescript';

await generate({
  name: 'Example API',
  version: '1.0.0',
  input: './openapi.yaml',
  output: './generated',
  plugins: [pluginTypeScript({ path: 'client' })],
});
```

[JavaScript quickstart](docs/javascript/quickstart.md) · [Rust SDK quickstart](docs/rust/quickstart.md)

## Documentation

| I want to… | Start here |
| --- | --- |
| Use JavaScript | [JavaScript](docs/javascript/README.md) |
| Use the Rust SDK | [Rust SDK](docs/rust/README.md) |
| Write JavaScript plugins | [JavaScript plugins](docs/plugins/javascript/README.md) |
| Write Rust plugins | [Rust plugins](docs/plugins/rust/README.md) |
| Learn about Poolster | [About](docs/about/README.md) |
| Understand contracts, blocks and lifecycles | [Internals](docs/internals/README.md) |

## Supported outputs

OpenAPI supports the established HTTP SDKs and helpers. GraphQL supports
clients for all ten SDK languages and TypeScript query, validation and testing companions
in alpha.2. Generated GraphQL packages separate models, operations, transport
and client code, with raw, flat and grouped call styles. GraphQL also generates
[Postman collections and executable CLIs](docs/reference/outputs/graphql-tools.md).
AsyncAPI generates [shared models](docs/reference/outputs/shared-models.md) for all ten
SDK languages and [Kafka producer/consumer packages](docs/reference/outputs/kafka.md)
for all ten SDK languages, plus Symfony integration over the PHP runtime. This is
the bounded plaintext JSON Kafka subset, with language-specific limits. See the
[Kafka capability comparison](docs/reference/outputs/kafka-conformance.md).
[Protobuf/gRPC](docs/reference/outputs/protobuf.md) generates Go, Python and TypeScript packages. Python and TypeScript source generation is owned by Poolster and needs no external code generator. Arazzo has a source-resolved TypeScript workflow pipeline.

[Plugin support matrix](docs/plugin-support-matrix.md) ·
[GraphQL package layouts](docs/reference/outputs/graphql-layout.md) ·
[Advanced GraphQL capabilities](docs/reference/outputs/graphql-capabilities.md) ·
[Examples](examples/README.md)

<a id="build-your-own-plugin"></a>

## Extend Poolster

Inputs publish contracts. Outputs consume them. Your plugins can provide whole
contracts, expose optional building blocks and insert transformations.

[JavaScript plugin tutorial](docs/plugins/javascript/output.md) ·
[Rust plugin tutorial](docs/plugins/rust/handlers.md)

<a id="ask-ai-about-poolster"></a>

## Project

[Contributing](CONTRIBUTING.md) · [Changelog](CHANGELOG.md) · [AI context](docs/about/ai.md) · [License](LICENSE)
