import assert from 'node:assert/strict';
import {Server,ServerCredentials,credentials,status} from '@grpc/grpc-js';
import {create} from '@bufbuild/protobuf';
import {ReaderService,ReadRequestSchema,ReadManyResponseSchema,EntitySchema} from './dist/batching.js';
import {createRawClient} from './dist/client.js';
import {createBatchScope} from './dist/batch-client.js';
let count=0;const server=new Server();server.addService(ReaderService,{read(call,callback){callback(null,create(EntitySchema,{id:call.request.id,value:'single'}));},readMany(call,callback){count++;if(call.request.ids.includes('error'))return callback({code:status.INVALID_ARGUMENT,details:'bad key'});callback(null,create(ReadManyResponseSchema,{entities:[...call.request.ids].reverse().map(id=>({id,value:id}))}));}});
const port=await new Promise((resolve,reject)=>server.bindAsync('127.0.0.1:0',ServerCredentials.createInsecure(),(error,port)=>error?reject(error):resolve(port)));
const raw=createRawClient(`127.0.0.1:${port}`,credentials.createInsecure());
const key='poolster.conformance.batch.Reader.Read';const input=id=>create(ReadRequestSchema,{id});
try{const scope=createBatchScope(raw);assert.deepEqual((await Promise.all(['a','b','c'].map(id=>scope[key](input(id))))).map(x=>x.id),['a','b','c']);assert.equal(count,2);await scope[key](input('a'));assert.equal(count,2);
const separate=createBatchScope(raw);await separate[key](input('a'));assert.equal(count,3);
const abort=new AbortController();const cancelled=scope[key](input('cancel'),abort.signal);abort.abort();await assert.rejects(cancelled,e=>e.name==='AbortError');await new Promise(resolve=>setTimeout(resolve,0));assert.equal(count,3);
await assert.rejects(scope[key](input('error')),e=>e.code===status.INVALID_ARGUMENT);await assert.rejects(scope[key](input('error')),e=>e.code===status.INVALID_ARGUMENT);assert.equal(count,5);
scope.close();await assert.rejects(scope[key](input('a')),/closed/);separate.close();}finally{raw.reader.close();await new Promise(resolve=>server.tryShutdown(resolve));}
