# TypeScript helpers

Poolster can emit a TypeScript SDK plus Zod validation, TanStack React/Vue Query, SWR,
Faker, MSW, and Cypress scaffolding. Select only the helpers your app uses.

## Select helpers

```json
{
  "language": "typescript",
  "path": "typescript/fetch",
  "plugins": [
    { "name": "sdk", "transport": "fetch" },
    { "name": "zod", "output": "validation" },
    { "name": "tanstack-react-query", "output": "react-query" },
    { "name": "faker", "output": "faker" },
    { "name": "msw", "output": "mocks" }
  ]
}
```

Keep helpers in the same package as the SDK whose operation functions they call.
Native consumers resolve imports through the selected operation provider. Use
`uses.operations` when the package has multiple providers; relocation and renamed
operation symbols are resolved automatically.

## Validation

`zod` targets Zod 4 and emits component schemas, request/response schema maps, and a
`poolsterSchemas` registry. Validate untrusted values at application boundaries; generated
clients do not silently validate every request/response.

```ts
import { PetSchema } from "@acme/pet-store/validation/zod";
const pet = PetSchema.parse(untrustedValue);
```

## Hooks

TanStack hooks wrap generated operations. React/Vue Query include GET queries
and non-GET mutations; SWR includes GET hooks and mutation helpers. Query/mutation option factories
can be reused for prefetching and native framework observers. Set up providers
and application cache policy in your framework.

```tsx
const query = useGetPet(
  { client: api.transport, path: { petId: "pet_123" }, throwOnError: true },
  { staleTime: 60_000, select: (pet) => pet.name },
  "tenant_123",
);
```

## Add fixtures and test scaffolding

Faker creates schema-shaped samples. MSW and Cypress are editable scaffolding; they are
not complete behavioral mocks. See [testing generated SDKs](testing.md) and the [full
TypeScript example](../../examples/typescript-stack/README.md).

For every option and direct Rust API use, see [auxiliary
generators](../reference/outputs/auxiliary-generators.md).

See the [generator completion backlog](../proposals/generator-backlog.md) for language fixes, output size/splitting, framework APIs and artifact acceptance work. These items are planned, not current capabilities.

TanStack helpers forward cancellation into `requestOptions.signal` and retain a
caller-provided signal. Query keys contain the operation, explicit cache scope
and path/query/body inputs. They omit the client, headers and transport config.
Use a distinct scope when tenants, origins or response-affecting headers differ;
request inputs themselves are still visible in the framework cache. Pass
`throwOnError: true` for SDK errors to reject queries/mutations.

Each GET operation also exports `<operation>QueryKey` and
`<operation>QueryOptions`; each mutation exports `<operation>MutationKey` and
`<operation>MutationOptions`. Hook overrides preserve `select`, stale-time,
retry and mutation callback/context types. SWR accepts its native configuration
as the second argument and a scope as the third.

Native React/Vue/SWR consumers split after 50 operations per module by default,
retaining the configured entrypoint as a barrel. Set
`max_operations_per_file` on the query plugin in `poolster.json`, or use
`.max_operations_per_file(...)` / `.single_file()` in the Rust composition API.
Standalone artifact renderers retain their single-file layout. JavaScript
factories expose these controls through `queryOptions`; see
[JavaScript plugins](../javascript/plugins.md#select-and-customize-query-helpers).

## Pagination, suspense and server rendering

HTTP operations with validated pagination metadata also export
`<operation>InfiniteOptions`, `<operation>InfiniteKey` and `use<Operation>Infinite`.
Cursor, page, offset/limit and URL pagination use the resolved SDK pagination plan.
Repeated continuation values stop traversal; configured page limits bound it.
URL pagination requires `pagination.baseUrl` and rejects continuations outside that
origin. Pass `throwOnError: true` when SDK errors should reject a page fetch.

React Query additionally exports `use<Operation>Suspense` and, for paginated
queries, `use<Operation>SuspenseInfinite`. Vue Query uses its native query APIs;
it does not receive React suspense hooks. SWR uses `swr/infinite` with sequential
page loading and `swr/mutation` for mutations.

For React/Vue server rendering, prefetch into a request-local `QueryClient`, then
use the framework's native dehydration/hydration integration:

```ts
import { QueryClient, dehydrate } from '@tanstack/react-query';
import { listPetsInfinitePrefetch } from './generated/react-query.js';

const cache = new QueryClient();
await listPetsInfinitePrefetch(
  cache,
  { client: api.transport, throwOnError: true },
  'tenant_123',
);
const state = dehydrate(cache);
```

The operation must declare supported pagination metadata. Prefetch helpers also
exist for ordinary queries as `<operation>Prefetch`. Keep authentication-specific
state request-local and use the same cache scope during hydration. These HTTP
helpers do not imply GraphQL infinite-pagination support.
