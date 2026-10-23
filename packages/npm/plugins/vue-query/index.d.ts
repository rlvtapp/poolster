import type { NativeAddon, FixtureOptions, CypressOptions, QueryOptions } from '@relevate/poolster';

export interface VueQueryPluginOptions { target?: string; output?: string; queryOptions?: QueryOptions }
export function pluginVueQuery(options?: VueQueryPluginOptions): NativeAddon;
