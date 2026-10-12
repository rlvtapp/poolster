'use strict';

const fs = require('node:fs');
const path = require('node:path');
const { pathToFileURL } = require('node:url');

function validateInput(input) {
  if (typeof input === 'string') return;
  if (input && typeof input === 'object' && typeof input.artifacts === 'string') return;
  if (input && typeof input === 'object' && typeof input.path === 'string' && input.path &&
      input.plugin?.kind === 'native-input' && typeof input.plugin.format === 'string' && input.plugin.format.trim() &&
      typeof input.plugin.provider === 'string' && input.plugin.provider.trim()) return;
  if (input && typeof input === 'object' && typeof input.path === 'string' && input.path &&
      input.plugin?.kind === 'js-input' && typeof input.plugin.name === 'string' && input.plugin.name.trim() &&
      typeof input.plugin.format === 'string' && input.plugin.format.trim() &&
      typeof input.plugin.load === 'function') return;
  throw new TypeError('input must be an OpenAPI path, { artifacts: directory }, or { path, plugin: inputPlugin() }');
}

function defineInputPlugin(factory) {
  if (typeof factory !== 'function') throw new TypeError('input plugin factory must be a function');
  return (...args) => {
    const plugin = factory(...args);
    validateInput({ path: '.', plugin });
    if (plugin.kind !== 'js-input') throw new TypeError('defineInputPlugin requires kind: js-input');
    return plugin;
  };
}

function defineConfig(config) {
  if (!config || typeof config !== 'object' || Array.isArray(config)) {
    throw new TypeError('Poolster config must be an object');
  }
  validateInput(config.input);
  if (config.input.plugin?.format === 'graphql') {
    if (config.input.operations != null && (!Array.isArray(config.input.operations) || config.input.operations.some((file) => typeof file !== 'string' || !file))) throw new TypeError('GraphQL operations must be nonempty file paths');
    if (config.input.importRoots != null && (!Array.isArray(config.input.importRoots) || config.input.importRoots.some(p=>typeof p!=='string'||!p))) throw new TypeError('GraphQL importRoots must be directory paths');
    if (config.input.incremental != null && typeof config.input.incremental !== 'boolean') throw new TypeError('GraphQL incremental must be boolean');
    if (config.input.subscriptions != null && typeof config.input.subscriptions !== 'boolean') throw new TypeError('GraphQL subscriptions must be boolean');
  }
  if (config.input.plugin?.format === 'protobuf') {
    if (config.input.importRoots != null && (!Array.isArray(config.input.importRoots) || config.input.importRoots.some(p=>typeof p!=='string'||!p))) throw new TypeError('Protobuf importRoots must be directory paths');
    for (const key of ['operations','incremental','subscriptions','scalars','rustScalars']) {
      if (config.input[key] !== undefined) throw new TypeError(`Protobuf input does not support ${key}`);
    }
  }
  if (config.input.plugin?.format === 'asyncapi') {
    for (const key of ['operations','importRoots','incremental','subscriptions','scalars','rustScalars']) {
      if (config.input[key] !== undefined) throw new TypeError(`AsyncAPI input does not support ${key}; configure broker for event SDKs`);
    }
  }
  if (config.input.broker != null && (config.input.plugin?.format !== 'asyncapi' || typeof config.input.broker !== 'object' || Array.isArray(config.input.broker))) throw new TypeError('broker requires an AsyncAPI input and broker configuration object');
  if (!config.input.plugin && (typeof config.name !== 'string' || !config.name.trim())) throw new TypeError('name is required');
  if (!config.input.plugin && (typeof config.version !== 'string' || !config.version.trim())) throw new TypeError('version is required');
  const destination = typeof config.output === 'string' ? config.output : config.output?.path;
  if (typeof destination !== 'string' || !destination.trim()) throw new TypeError('output or output.path is required');
  if (!Array.isArray(config.plugins) || config.plugins.length === 0) {
    throw new TypeError('plugins must contain at least one SDK or JavaScript plugin');
  }
  return config;
}

function sdkPackage(options) {
  if (!options || typeof options !== 'object' || Array.isArray(options)) {
    throw new TypeError('sdk options must be an object');
  }
  if (typeof options.language !== 'string' || !options.language) {
    throw new TypeError('sdk language is required');
  }
  const allowed = new Set(['language', 'mode', 'path', 'name', 'version', 'style', 'transport', 'clientName', 'raw', 'jobs', 'scalars', 'groups', 'contracts', 'contractOptions', 'endpoint', 'commandName', 'subscriptions', 'sourceQuality']);
  for (const key of Object.keys(options)) {
    if (!allowed.has(key)) throw new TypeError(`unknown sdk option ${key}`);
  }
  if (!['typescript', 'rust', 'go', 'python', 'php', 'symfony', 'java', 'csharp', 'elixir', 'ruby', 'swift', 'postman', 'rust-cli', 'typescript-cli'].includes(options.language)) {
    throw new TypeError(`unsupported SDK language ${options.language}`);
  }
  if (options.mode != null && !['sdk','models'].includes(options.mode)) throw new TypeError('sdk mode must be sdk or models');
  if (options.mode === 'models') {
    const supported = new Set(['language','path','name','version','mode','sourceQuality']);
    for (const [key,value] of Object.entries(options)) if (!supported.has(key) && value !== undefined) throw new TypeError(`models mode does not support ${key}; use mode sdk for styles, transports and runtime options`);
  }
  if (options.contractOptions != null) {
    if (typeof options.contractOptions !== 'object' || Array.isArray(options.contractOptions)) throw new TypeError('contractOptions must be an object');
    for (const key of Object.keys(options.contractOptions)) if (key !== 'protobuf') throw new TypeError(`unknown contractOptions contract ${key}`);
    const proto = options.contractOptions.protobuf;
    if (proto != null) {
      if (typeof proto !== 'object' || Array.isArray(proto)) throw new TypeError('contractOptions.protobuf must be an object');
      const allowed = new Set(['module','packageName','goPackages','toolchain']);
      for (const key of Object.keys(proto)) if (!allowed.has(key)) throw new TypeError(`unknown Protobuf option ${key}`);
      for (const key of ['module','packageName']) if (proto[key] != null && (typeof proto[key] !== 'string' || !proto[key].trim())) throw new TypeError(`Protobuf ${key} must be a nonempty string`);
      for (const key of ['goPackages','toolchain']) if (proto[key] != null) {
        if (typeof proto[key] !== 'object' || Array.isArray(proto[key])) throw new TypeError(`Protobuf ${key} must be an object`);
        for (const [name,value] of Object.entries(proto[key])) {
          if (key === 'toolchain' && !['protoc','protocGenGo','protocGenGoGrpc'].includes(name)) throw new TypeError(`unknown Protobuf tool ${name}`);
          if (!name.trim() || typeof value !== 'string' || !value.trim()) throw new TypeError(`Protobuf ${key} entries must be nonempty strings`);
        }
      }
    }
  }
  const { language, path: outputPath = language, name, version, style, transport, clientName, raw, jobs, scalars, groups, contracts, contractOptions, endpoint, commandName, subscriptions, sourceQuality } = options;
  if (typeof outputPath !== 'string' || !outputPath) throw new TypeError('sdk path must be a nonempty string');
  return { language, ...(options.mode != null ? { mode: options.mode } : {}), path: outputPath, name, version, style, transport, clientName, raw, jobs, scalars, groups, contracts, contractOptions, endpoint, commandName, subscriptions, sourceQuality };
}

function normalizedPackagePath(value) {
  const normalized = path.posix.normalize(value.replaceAll('\\', '/'));
  return normalized.endsWith('/') && normalized !== '/' ? normalized.slice(0, -1) : normalized;
}

function definePlugin(factory) {
  if (typeof factory !== 'function') throw new TypeError('plugin factory must be a function');
  return (...args) => {
    const plugin = factory(...args);
    validatePlugin(plugin);
    return plugin;
  };
}

function validatePlugin(plugin) {
  if (!plugin || typeof plugin !== 'object' || typeof plugin.name !== 'string' || !plugin.name.trim()) {
    throw new TypeError('JavaScript plugins need a nonempty name');
  }
  const hooks = plugin.hooks ?? plugin;
  if (!hooks || typeof hooks !== 'object') throw new TypeError(`plugin ${plugin.name}.hooks must be an object`);
  if (!['transformApi', 'generate', 'schema', 'operation'].some((hook) => typeof hooks[hook] === 'function')) {
    throw new TypeError(`plugin ${plugin.name} needs transformApi, generate, schema, or operation`);
  }
  for (const hook of ['transformApi', 'generate', 'schema', 'operation']) {
    if (hooks[hook] != null && typeof hooks[hook] !== 'function') {
      throw new TypeError(`plugin ${plugin.name}.${hook} must be a function`);
    }
  }
  if (plugin.requires != null && !Array.isArray(plugin.requires)) {
    throw new TypeError(`plugin ${plugin.name}.requires must be an array`);
  }
  return plugin;
}

async function loadConfig(configFile) {
  const candidates = configFile
    ? [path.resolve(configFile)]
    : ['poolster.config.mjs', 'poolster.config.js', 'poolster.config.cjs'].map((file) => path.resolve(file));
  const file = candidates.find((candidate) => fs.existsSync(candidate));
  if (!file) throw new Error(`No Poolster config found. Looked for ${candidates.join(', ')}`);
  const module = await import(pathToFileURL(file).href);
  let config = module.default ?? module.config;
  if (typeof config === 'function') config = config();
  config = await config;
  defineConfig(config);
  const base = path.dirname(file);
  const resolve = (value) => path.resolve(base, value);
  return {
    ...config,
    input: typeof config.input === 'string' ? resolve(config.input) : config.input.plugin
      ? { ...config.input, path: resolve(config.input.path), ...(config.input.operations ? { operations: config.input.operations.map(resolve) } : {}), ...(config.input.importRoots ? {importRoots: config.input.importRoots.map(resolve)} : {}) }
      : { artifacts: resolve(config.input.artifacts) },
    output: typeof config.output === 'string' ? resolve(config.output) : { ...config.output, path: resolve(config.output.path) },
    ...(config.compiler ? { compiler: resolve(config.compiler) } : {}),
  };
}

function nativeAddonOptions(addon) {
  const rename = (value, keys, label) => {
    if (!value || typeof value !== 'object' || Array.isArray(value)) throw new TypeError(`${label} must be an object`);
    const result = {};
    for (const [key, item] of Object.entries(value)) {
      if (!(key in keys)) throw new TypeError(`unknown ${label} option ${key}`);
      result[keys[key]] = item;
    }
    return result;
  };
  const options = {};
  if (addon.fixtureOptions !== undefined) options.fixtureOptions = rename(addon.fixtureOptions,
    { seed:'seed', maxDepth:'max_depth', maxAttempts:'max_attempts', overrides:'overrides' }, 'fixtureOptions');
  if (addon.cypressOptions !== undefined) {
    const mapped = rename(addon.cypressOptions,
      {baseUrl:'base_url', headers:'headers', includeMutations:'include_mutations', timeoutMs:'timeout_ms', operationOverrides:'operation_overrides'}, 'cypressOptions');
    if (mapped.operation_overrides) mapped.operation_overrides = Object.fromEntries(Object.entries(mapped.operation_overrides).map(([name,value]) => [name,rename(value,
      {enabled:'enabled',path:'path',query:'query',headers:'headers',body:'body',expectedStatuses:'expected_statuses'}, 'Cypress operation override')]));
    options.cypressOptions = mapped;
  }
  return options;
}

module.exports = { nativeAddonOptions, validateInput, defineInputPlugin, defineConfig, sdkPackage, normalizedPackagePath, definePlugin, validatePlugin, loadConfig };
