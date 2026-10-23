import type { NativeAddon, FixtureOptions, CypressOptions, QueryOptions } from '@relevate/poolster';

export interface SwrPluginOptions { target?: string; output?: string; queryOptions?: QueryOptions }
export function pluginSwr(options?: SwrPluginOptions): NativeAddon;
