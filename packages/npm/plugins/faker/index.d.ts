import type { NativeAddon, FixtureOptions, CypressOptions, QueryOptions } from '@relevate/poolster';

export interface FakerPluginOptions { target?: string; output?: string; fixtureOptions?: FixtureOptions }
export function pluginFaker(options?: FakerPluginOptions): NativeAddon;
