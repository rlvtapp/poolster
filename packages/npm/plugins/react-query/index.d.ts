import type { NativeAddon, FixtureOptions, CypressOptions, QueryOptions } from '@relevate/poolster';

export interface ReactQueryPluginOptions { target?: string; output?: string; queryOptions?: QueryOptions }
export function pluginReactQuery(options?: ReactQueryPluginOptions): NativeAddon;
