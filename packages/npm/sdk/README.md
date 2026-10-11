# Poolster for JavaScript

Generate packages from API contracts using a JavaScript config and selected plugins.

```sh
npm install -D @relevate/poolster @relevate/poolster-plugin-typescript
```

```js
import { defineConfig, generate } from '@relevate/poolster';
import { pluginTypeScript } from '@relevate/poolster-plugin-typescript';

const config = defineConfig({
  name: 'Example API',
  version: '1.0.0',
  input: './openapi.yaml',
  output: './generated',
  plugins: [pluginTypeScript({ path: 'client', name: '@example/client' })],
});
await generate(config);
```

Build the generated package, then import its compiled exports from JavaScript
or TypeScript. The optional `poolster` CLI is a separate npm package.

## Models without a client

Set `mode: 'models'` on a language plugin to emit standalone data models from a
native GraphQL or AsyncAPI input. All ten SDK languages have a model output.
GraphQL uses schema and selection-specific operation model blocks; AsyncAPI uses
message payload/header/key schemas and needs no broker configuration.

```js
import { generate } from '@relevate/poolster';
import { inputGraphql, pluginTypeScript } from '@relevate/poolster/plugins';

await generate({
  input: {
    path: './schema.graphql',
    plugin: inputGraphql(),
    operations: ['./read-user.graphql'],
  },
  output: './generated',
  plugins: [pluginTypeScript({ mode: 'models', path: 'models' })],
});
```

Omitting `mode` keeps SDK generation. Model packages support `language`, `path`,
`name`, `version` and `sourceQuality`; SDK styles, transports, scalar mappings,
groups and auxiliary plugins require SDK mode. You can generate a GraphQL model
package and client package together by giving them different paths. Overlapping
files produce an ownership error. JSON Schema constraints are retained in model
schema manifests; declarations alone do not validate runtime values.

## Documentation

| Task | Guide |
| --- | --- |
| Install and generate | [JavaScript quickstart](../../../docs/javascript/quickstart.md) |
| Configure inputs and packages | [Configuration](../../../docs/javascript/configuration.md) |
| Add hooks, validation and test helpers | [Output plugins](../../../docs/javascript/plugins.md) |
| Generate GraphQL clients | [GraphQL](../../../docs/javascript/graphql.md) |
| Write a JavaScript plugin | [Plugin authoring](../../../docs/plugins/javascript/README.md) |
| Understand lifecycle and contracts | [Internals](../../../docs/internals/README.md) |
| Look up an API | [Type declarations](index.d.ts) |

The docs describe the current checkout. GraphQL client and companion additions
are unreleased and are not all present in published `0.5.0-alpha.1` packages.
[Support matrix](../../../docs/plugin-support-matrix.md).

## Development

From the repository root:

```sh
node packages/npm/sdk/scripts/build-native.mjs
node packages/npm/generate-plugins.mjs --check
npm test --prefix packages/npm/sdk
```

For source OpenAPI input, the addon also needs the Poolster compiler. Set
`POOLSTER_OPENAPI_BIN` to a built compiler or use the bundled platform package.
