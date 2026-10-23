import type { NativeAddon, FixtureOptions, CypressOptions, QueryOptions } from '@relevate/poolster';

export interface MswPluginOptions { target?: string; output?: string; fixtureOptions?: FixtureOptions }
export function pluginMsw(options?: MswPluginOptions): NativeAddon;
