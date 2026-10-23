import type { NativeAddon, FixtureOptions, CypressOptions, QueryOptions } from '@relevate/poolster';

export interface CypressPluginOptions { target?: string; output?: string; fixtureOptions?: FixtureOptions; cypressOptions?: CypressOptions }
export function pluginCypress(options?: CypressPluginOptions): NativeAddon;
