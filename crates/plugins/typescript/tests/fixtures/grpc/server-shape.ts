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
