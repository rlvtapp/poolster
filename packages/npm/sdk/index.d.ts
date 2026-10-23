export interface Operation {
  id: string;
  method: string;
  path: string;
  parameters?: Array<Record<string, unknown>>;
  request_body?: Record<string, unknown>;
  responses?: Array<Record<string, unknown>>;
  security?: Array<Record<string, unknown>>;
  annotations?: Record<string, unknown>;
}

export interface Schema {
  name: string;
  value: Record<string, unknown>;
}

export interface Api {
  name: string;
  version: string;
  operations: Operation[];
  schemas: Schema[];
  annotations: Record<string, unknown>;
}

export interface OutputFile {
  path: string;
  contents: string;
  preserveExisting?: boolean;
  owner?: string;
}

export interface PluginContext {
  readonly api: Api | null;
  readonly input?: InputReport;
  readonly securitySchemes: Record<string, unknown>;
  readonly output: string;
  emitFile(file: Pick<OutputFile, 'path' | 'contents' | 'preserveExisting'>): void;
  readFile(path: string): string | undefined;
  replaceFile(path: string, contents: string): void;
  readonly workspace: Map<string, unknown>;
  readonly inputs: {
    get<T>(contract: JsContract<T>): T;
    optional<T>(contract: JsContract<T>): T | undefined;
  };
  publish<T>(contract: JsContract<T>, value: T): void;
}

export interface JsContract<T = unknown> {
  readonly name: string;
  /** Type-only marker; contract identity is the exported object itself. */
  readonly __value?: T;
}

export interface ProviderHandle<T = unknown> {
  readonly provider: JsPlugin;
  readonly contract: JsContract<T>;
}

export interface ContractRequirement<T = unknown> {
  readonly contract: JsContract<T>;
  readonly from?: ProviderHandle<T>;
  readonly optional: boolean;
}

export interface JsPlugin {
  name: string;
  phase?: 'generate' | 'post';
  requires?: Array<string | ContractRequirement>;
  provides?: JsContract[];
  hooks?: {
    transformApi?: JsPlugin['transformApi'];
    generate?: JsPlugin['generate'];
    schema?: JsPlugin['schema'];
    operation?: JsPlugin['operation'];
  };
  transformApi?(api: Api, context: { securitySchemes: Record<string, unknown>; output: string }): Api | void | Promise<Api | void>;
  generate?(context: PluginContext): void | Promise<void>;
  schema?(schema: Schema, context: PluginContext): void | Promise<void>;
  operation?(operation: Operation, context: PluginContext): void | Promise<void>;
}

export type SdkLanguage = 'typescript' | 'rust' | 'go' | 'python' | 'php' | 'symfony' | 'java' | 'csharp' | 'elixir' | 'ruby' | 'swift' | 'postman' | 'rust-cli' | 'typescript-cli';

/** Explicit generated-source formatting policy; commands run from trusted configuration. */
export type SourceQuality =
  | { mode: 'unformatted'; reason: string }
  | { mode: 'formatted'; maxFileBytes?: number; formatter: {
      program: string; arguments: string[]; versionArguments: string[];
      expectedVersion: string; extensions: string[]; timeoutSeconds?: number;
    } };

export interface SdkPackageOptions {
  language: SdkLanguage;
  /** Generate a protocol SDK (default), or standalone models from native GraphQL/AsyncAPI blocks. */
  mode?: 'sdk' | 'models';
  sourceQuality?: SourceQuality;
  /** GraphQL collection/CLI endpoint; CLI command binary name. */
  endpoint?: string;
  commandName?: string;
  /** Options for the selected bundled HTTP or GraphQL exporter. */
  contracts?: { http?: ContractOutputOptions; graphql?: ContractOutputOptions };
  /** Protocol-specific generator settings; not HTTP/GraphQL API styles. */
  contractOptions?: { protobuf?: ProtobufOutputOptions };
  path?: string;
  name?: string;
  version?: string;
  style?: 'raw' | 'flat' | 'idiomatic' | 'namespaced' | 'grouped';
  /** GraphQL output wire types for this language. */
  scalars?: Record<string, GraphqlScalarMapping>;
  /** GraphQL operation names assigned to idiomatic resource groups. */
  groups?: Record<string, Record<string, string>>;
  transport?: 'fetch' | 'axios';
  clientName?: string;
  raw?: boolean;
  /** GraphQL distinct-connection SSE generation capability. */
  subscriptions?: boolean;
  jobs?: number;
}

/** Protocol surfaces and resource policies; native message APIs remain available. */
export interface ProtobufBatchMapping {
  singleMethod: string;
  batchMethod: string;
  inputField: string;
  batchInputField: string;
  batchOutputField: string;
  outputKey?: string;
  cache?: boolean;
  maxBatchSize?: number;
}
export interface ProtobufRepresentations {
  int64?: 'native' | 'string' | 'numberExact';
  bytes?: 'native' | 'base64';
  maps?: 'native' | 'entries' | 'map';
  enums?: 'native' | 'name';
  optional?: 'native' | 'nullable';
  timestamp?: 'native' | 'dateTime' | 'rfc3339';
  duration?: 'native' | 'timeDelta' | 'secondsExact';
}
export interface ProtobufGenerationOptions {
  enumStyle?: 'native' | 'const' | 'literal';
  removeEnumPrefix?: boolean;
  /** Display label; the original unknown wire number is retained. */
  unknownEnumName?: string;
  nestjsRestParameter?: boolean;
  representations?: ProtobufRepresentations;
  typePrefix?: string;
  typeSuffix?: string;
  snakeTypeNames?: boolean;
  lowerCaseServiceMethods?: boolean;
  fileSuffix?: string;
  emitImportedFiles?: boolean;
  /** Externally supplied modules keyed by descriptor filename; mapped files are not emitted. */
  importMappings?: Record<string, string>;
  /** Fully qualified message identities; empty means all. */
  encodeIncludeTypes?: string[];
  decodeIncludeTypes?: string[];
  emitIndex?: boolean;
  comments?: boolean;
  annotateFilesWithVersion?: boolean;
  frameworks?: Array<'native' | 'niceGrpc' | 'nestJs'>;
  immutableViews?: boolean;
  reactiveStreams?: boolean;
  valueAdapters?: boolean;
  batching?: ProtobufBatchMapping[];
  transports?: Array<'grpc' | 'grpcWeb' | 'twirp'>;
  clientStyle?: 'raw' | 'flat' | 'idiomatic' | 'namespaced' | 'grouped';
  emitTypes?: boolean;
  emitBinary?: boolean;
  emitJson?: boolean;
  emitClients?: boolean;
  emitServers?: boolean;
  emitReflection?: boolean;
  unknownFields?: 'preserve' | 'discard';
  maxMessageBytes?: number;
  maxDepth?: number;
}
export interface ProtobufOutputOptions {
  generation?: ProtobufGenerationOptions;
  /** Required Go module import path. */
  module?: string;
  /** Python distribution name, default poolster-rpc. Import paths follow the proto files. */
  packageName?: string;
  /** Go Mfile.proto import mappings. */
  goPackages?: Record<string, string>;
  /** Go-only executables for the pinned official toolchain; Python and TypeScript use Poolster rendering. */
  toolchain?: { protoc?: string; protocGenGo?: string; protocGenGoGrpc?: string };
}

export interface ContractOutputOptions {
  style?: 'raw' | 'flat' | 'idiomatic' | 'namespaced' | 'grouped';
  raw?: boolean;
  /** GraphQL distinct-connection SSE generation capability. */
  subscriptions?: boolean;
  scalars?: Record<string, GraphqlScalarMapping>;
  groups?: Record<string, Record<string, string>>;
  transport?: 'fetch' | 'axios';
  clientName?: string;
  jobs?: number;
}

export interface NativePlugin {
  kind: 'native-sdk';
  name: string;
  package: SdkPackageOptions & { path: string };
}

export interface FixtureOptions { seed?: number; maxDepth?: number; maxAttempts?: number; overrides?: Record<string, unknown>; }
export interface CypressOptions {
  baseUrl?: string; headers?: Record<string, string>; includeMutations?: boolean; timeoutMs?: number;
  operationOverrides?: Record<string, { enabled?: boolean; path?: string; query?: Record<string, unknown>; headers?: Record<string,string>; body?: unknown; expectedStatuses?: number[] }>;
}

export interface QueryOptions {
  includeOperations?: string[];
  operationKinds?: Record<string, 'query' | 'mutation'>;
  operationNames?: Record<string, string>;
  maxOperationsPerFile?: number;
}

export interface NativeAddon {
  kind: 'native-addon';
  name: string;
  plugin: string;
  target: string;
  output?: string;
  fixtureOptions?: FixtureOptions;
  cypressOptions?: CypressOptions;
  queryOptions?: QueryOptions;
}

export interface NativeInputPlugin {
  kind: 'native-input';
  name: string;
  format: string;
  provider: string;
}

export interface JsInputPlugin<TData = unknown> {
  kind: 'js-input';
  name: string;
  format: string;
  load(source: string): InputPluginResult<TData> | Promise<InputPluginResult<TData>>;
}

export interface InputPluginResult<TData = unknown> {
  summary: InputReport['summary'];
  diagnostics?: InputReport['diagnostics'];
  data?: TData;
  /** Publishing an HTTP API enables the existing native SDK renderers. */
  api?: Api;
  securitySchemes?: Record<string, unknown>;
}

export interface InputReport {
  provider: string;
  source: string;
  summary: {
    format: string;
    title: string;
    version: string | null;
    types: string[];
    operations: Array<{ name: string; kind: string }>;
  };
  diagnostics: Array<{ code: string; message: string }>;
  data?: unknown;
}

export interface GraphqlScalarMapping { input: string; output: string; }
export interface NativeInputConfig {
  path: string; plugin: NativeInputPlugin | JsInputPlugin;
  /** GraphQL operation files, resolved beside a loaded config file. */
  operations?: string[];
  importRoots?: string[];
  /** Opt into multipart deferSpec=20220824 incremental operations. */
  incremental?: boolean;
  /** TypeScript application types; does not install callbacks; supply runtime scalarCodecs to the generated client for conversion. */
  scalars?: Record<string, GraphqlScalarMapping>;
  /** Supported Rust wire types; independent of TypeScript scalar mappings. */
  rustScalars?: Record<string, GraphqlScalarMapping>;
  subscriptions?: boolean;
  /** Explicit Kafka connection configuration for AsyncAPI event SDK generation. */
  broker?: { kind: 'kafka'; brokers: string[]; client_id?: string };
}

export interface PoolsterConfig {
  input: string | { artifacts: string } | NativeInputConfig;
  output: string | { path: string };
  name?: string;
  version?: string;
  compiler?: string;
  plugins: Array<NativePlugin | NativeAddon | JsPlugin>;
}

export interface OutputChanges {
  added: string[];
  modified: string[];
  removed: string[];
}

export interface GenerateResult {
  api: Api | null;
  input?: InputReport;
  files: OutputFile[];
  changes: OutputChanges;
  output: string;
  skipped?: Array<{ path: string; language: string; reason: string }>;
}

export function defineConfig<T extends PoolsterConfig>(config: T): T;
export function definePlugin<TOptions extends unknown[]>(factory: (...options: TOptions) => JsPlugin): (...options: TOptions) => JsPlugin;
export function defineInputPlugin<TOptions extends unknown[], TData = unknown>(factory: (...options: TOptions) => JsInputPlugin<TData>): (...options: TOptions) => JsInputPlugin<TData>;
export function defineContract<T = unknown>(name: string): JsContract<T>;
export function providerHandle<T>(provider: JsPlugin, contract: JsContract<T>): ProviderHandle<T>;
export function requireContract<T>(contract: JsContract<T>, options?: { from?: ProviderHandle<T>; optional?: boolean }): ContractRequirement<T>;
export function availableNativePlugins(): string[];
export function availableInputPlugins(): Array<{ provider: string; format: string }>;
export function inspectInput(input: { path: string; plugin: NativeInputPlugin | JsInputPlugin }): Promise<InputReport>;
export function loadConfig(file?: string): Promise<PoolsterConfig>;
export function generate(config: PoolsterConfig, options?: { write?: boolean }): Promise<GenerateResult>;
export function createPoolster(config: PoolsterConfig): { plan(): JavaScriptPlan; generate(options?: { write?: boolean }): Promise<GenerateResult>; inspectInput(): Promise<InputReport> };

/** Execution-free declarations. Native nodes are opaque boundaries in Node. */
export interface JavaScriptPlan {
  version: 1;
  runtime: 'javascript';
  input: { format: string; provider: string };
  plugins: Array<{ id: number; kind: string; phase: string; order: number | null; detail: string; hooks: string[]; provides: string[]; requires: string[] }>;
  edges: Array<{ provider: number | null; consumer: number; contract: string | null; dependency: string | null; optional: boolean; selection: string }>;
  stages: string[];
  limitations: string[];
}
export function plan(config: PoolsterConfig): JavaScriptPlan;
export function formatPlan(plan: JavaScriptPlan): string;

/** Validate and normalize a native SDK/model package configuration. */
export function sdkPackage(options: SdkPackageOptions): SdkPackageOptions & { path: string };
