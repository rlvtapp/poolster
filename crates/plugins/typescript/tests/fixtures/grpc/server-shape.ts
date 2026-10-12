import type { StoreServer } from './service.js';
import { create } from '@bufbuild/protobuf';
import { ItemSchema } from './common.js';
const implementation: StoreServer = {
  get(call, callback) { callback(null, call.request); },
  watch(call) { call.write(call.request); call.end(); },
  upload(call, callback) { call.on('end', () => callback(null, create(ItemSchema))); },
  chat(call) { call.on('data', item => call.write(item)); call.on('end', () => call.end()); },
};
const reserved = create(ItemSchema, {toString$:'safe',value:{case:'constructor',value:'safe'}});
void reserved;
void implementation;

// Exercise every native grpc-js overload without issuing network requests.
import type {StoreClient} from './service.js';
import {Metadata, type CallOptions} from '@grpc/grpc-js';
function overloads(client: StoreClient) {
  const request = create(ItemSchema);
  const metadata = new Metadata();
  const options: CallOptions = {deadline: new Date()};
  const callback = () => {};
  client.get(request, callback);
  client.get(request, options, callback);
  client.get(request, metadata, callback);
  client.get(request, metadata, options, callback);
  client.upload(callback);
  client.upload(options, callback);
  client.upload(metadata, callback);
  client.upload(metadata, options, callback);
  client.watch(request);
  client.watch(request, options);
  client.watch(request, metadata);
  client.watch(request, metadata, options);
  client.chat();
  client.chat(options);
  client.chat(metadata);
  client.chat(metadata, options);
  // @ts-expect-error request is a Protobuf message, not a string
  client.get('invalid', callback);
  // @ts-expect-error response callback must accept an Item
  client.get(request, (error, response: string) => {});
}
void overloads;
