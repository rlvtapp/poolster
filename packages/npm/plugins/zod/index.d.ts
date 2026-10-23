import type { NativeAddon, FixtureOptions, CypressOptions, QueryOptions } from '@relevate/poolster';

export interface ZodPluginOptions { target?: string; output?: string }
export function pluginZod(options?: ZodPluginOptions): NativeAddon;
