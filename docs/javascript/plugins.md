# Add output plugins

← [JavaScript](README.md)

Choose a client and the helpers you need. Each helper targets a package path.

```js
import { pluginTypeScript } from '@relevate/poolster-plugin-typescript';
import { pluginReactQuery } from '@relevate/poolster-plugin-react-query';
import { pluginZod } from '@relevate/poolster-plugin-zod';

plugins: [
  pluginTypeScript({ path: 'web' }),
  pluginReactQuery({ target: 'web' }),
  pluginZod({ target: 'web' }),
]
```

| Plugin | Produces |
| --- | --- |
| React Query / Vue Query | Query and mutation hooks |
| SWR | Query and mutation hooks |
| Zod | Runtime validation schemas |
| Faker | Synthetic fixtures |
| MSW | Mock request handlers |
| Cypress | Request and interception helpers |

All seven support the documented HTTP and GraphQL subsets. GraphQL helpers use
selected operation types; they do not inherit every HTTP plugin option.
[GraphQL helper usage](../reference/outputs/graphql-integrations.md) gives examples and limits.

Only install the integrations your application needs. Build the emitted package
with its declared dependencies and install framework peers in your application.

## Select and customize query helpers

React Query, Vue Query and SWR accept `queryOptions` in both individual packages
and the plugin bundle:

```js
pluginReactQuery({
  target: 'web',
  queryOptions: {
    includeOperations: ['listWidgets', 'searchWidgets'],
    operationNames: { listWidgets: 'catalog' },
    operationKinds: { searchWidgets: 'query' },
    maxOperationsPerFile: 25,
  },
})
```

Keys refer to original operation IDs. Selection affects helper generation; the
SDK retains its operations. HTTP methods default to GET queries and other-method
mutations; `operationKinds` overrides that classification. Names change helper
exports while preserving the selected SDK operation binding. Chunk counts must
be positive. Unknown operations and options fail before output is written.

For GraphQL, operation selection and helper names are supported. Query/mutation
kinds come from validated documents and cannot be overridden; custom chunk counts
are unsupported because GraphQL helpers currently emit one file.
HTTP infinite queries, React suspense and SSR prefetch usage are described in the
[TypeScript helper guide](../guides/typescript-helpers.md).

**Next:** [Support matrix](../plugin-support-matrix.md) ·
[Write a JavaScript plugin](../plugins/javascript/output.md)
