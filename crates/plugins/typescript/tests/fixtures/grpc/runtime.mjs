import assert from 'node:assert/strict';
import { credentials, Metadata, Server, ServerCredentials, status } from '@grpc/grpc-js';
import { StoreClient, StoreService } from './dist/service.js';
import { ItemSchema } from './dist/common.js';
import { create,fromBinary,toBinary } from '@bufbuild/protobuf';
const server = new Server();
server.addService(StoreService, {
  get(call, callback) {
    if (call.request.name === 'error') return callback({code: status.INVALID_ARGUMENT, details: 'bad input'});
    if (call.request.name === 'wait') return setTimeout(() => callback(null, call.request), 200);
    assert.equal(call.metadata.get('x-test')[0], 'ok'); callback(null, call.request);
  },
  watch(call) { call.write(call.request); call.write({...call.request, name:'second'}); call.end(); },
  upload(call, callback) { const items=[]; call.on('data', x=>items.push(x)); call.on('end', ()=>callback(null, {...items[0], name:String(items.length)})); },
  chat(call) { call.on('data', x=>call.write(x)); call.on('end', ()=>call.end()); }
});
const port = await new Promise((resolve, reject)=>server.bindAsync('127.0.0.1:0', ServerCredentials.createInsecure(), (err, port)=>err?reject(err):resolve(port)));
const client = new StoreClient(`127.0.0.1:${port}`, credentials.createInsecure());
const metadata = new Metadata(); metadata.set('x-test','ok');
const item = create(ItemSchema,{name:'one', largeCount: 9007199254740993n, labels:{first:'a'}, value:{case:'text',value:'hello'}, note:''});
const reserved=create(ItemSchema,{toString$:'safe',value:{case:'constructor',value:'safe'}});assert.deepEqual(fromBinary(ItemSchema,toBinary(ItemSchema,reserved)),reserved);
const encoded = fromBinary(ItemSchema,toBinary(ItemSchema,item)); assert.deepEqual(encoded,item);
try {
  assert.deepEqual(await new Promise((res,rej)=>client.get(item, metadata,(err,x)=>err?rej(err):res(x))), item);
  const collect = call=>new Promise((res,rej)=>{const items=[];call.on('data', x=>items.push(x));call.on('error',rej);call.on('end',()=>res(items));});
  assert.equal((await collect(client.watch(item))).length,2);
  const upload = new Promise((res,rej)=>{const call=client.upload((err,x)=>err?rej(err):res(x));call.write(item);call.write(item);call.end();}); assert.equal((await upload).name,'2');
  const chat=client.chat(); const pending=collect(chat);chat.write(item);chat.write({...item,name:'two'});chat.end();assert.equal((await pending).length,2);
  await assert.rejects(new Promise((res,rej)=>client.get({...item,name:'error'},metadata,(err,x)=>err?rej(err):res(x))), e=>e.code===status.INVALID_ARGUMENT);
  await assert.rejects(new Promise((res,rej)=>client.get({...item,name:'wait'},metadata,{deadline:new Date(Date.now()+30)},(err,x)=>err?rej(err):res(x))), e=>e.code===status.DEADLINE_EXCEEDED);
  await assert.rejects(new Promise((res,rej)=>{const call=client.get({...item,name:'wait'},metadata,(err,x)=>err?rej(err):res(x));call.cancel();}),e=>e.code===status.CANCELLED);
  console.log('all grpc checks passed');
} finally { client.close(); await new Promise(res=>server.tryShutdown(res)); }
