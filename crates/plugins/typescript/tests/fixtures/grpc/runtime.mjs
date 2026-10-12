import {createChannel,Metadata as NiceMetadata,createServer as niceServer} from 'nice-grpc';
import {createStoreNiceGrpcClient,StoreNiceGrpcDefinition} from './dist/nice-grpc.js';
import {createNestGrpcClient,getStoreNestClient} from './dist/nestjs.js';
import {firstValueFrom,toArray,from} from 'rxjs';
import {observeUnary,observeStream} from './dist/reactive.js';
import {immutableMessage} from './dist/immutable.js';
import {safeNumber,dateToTimestamp,timestampToDate,durationToMilliseconds,adaptMessage} from './dist/values.js';
import {readFileSync} from 'node:fs';
import {createServer} from 'node:http';
import {createStoreTwirpClient} from './dist/twirp.js';
import {createStoreGrpcWebClient} from './dist/grpcWeb.js';
import assert from 'node:assert/strict';
import { credentials, Metadata, Server, ServerCredentials, status, InterceptingCall } from '@grpc/grpc-js';
import {createFlatClient,createIdiomaticClient,createRawClient} from './dist/client.js';
import { StoreClient, StoreService } from './dist/service.js';
import { ItemSchema } from './dist/common.js';
import { create,fromBinary,toBinary } from '@bufbuild/protobuf';
const server = new Server();
server.addService(StoreService, {
  get(call, callback) {
    if (call.request.name === 'error') return callback({code: status.INVALID_ARGUMENT, details: 'bad input'});
    if (call.request.name === 'wait') return setTimeout(() => callback(null, call.request), 200);
    assert.equal(call.metadata.get('x-test')[0], 'ok'); const headers=new Metadata();headers.set('x-header','received');call.sendMetadata(headers);const trailers=new Metadata();trailers.set('x-trailer','complete'); callback(null, call.request,trailers);
  },
  watch(call) { call.write(call.request); call.write({...call.request, name:'second'}); call.end(); },
  upload(call, callback) { const items=[]; call.on('data', x=>{items.push(x);if(x.name==='slow'){call.pause();setTimeout(()=>call.resume(),1);}}); call.on('end', ()=>callback(null, {...items[0], name:String(items.length)})); },
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
  const collect = call=>new Promise((res,rej)=>{const items=[];call.on('data', x=>{items.push(x);if(x.name==='slow'){call.pause();setTimeout(()=>call.resume(),1);}});call.on('error',rej);call.on('end',()=>res(items));});
  assert.equal((await collect(client.watch(item))).length,2);
  const upload = new Promise((res,rej)=>{const call=client.upload((err,x)=>err?rej(err):res(x));call.write(item);call.write(item);call.end();}); assert.equal((await upload).name,'2');
  const chat=client.chat(); const pending=collect(chat);chat.write(item);chat.write({...item,name:'two'});chat.end();assert.equal((await pending).length,2);
  await assert.rejects(new Promise((res,rej)=>client.get({...item,name:'error'},metadata,(err,x)=>err?rej(err):res(x))), e=>e.code===status.INVALID_ARGUMENT);
  await assert.rejects(new Promise((res,rej)=>client.get({...item,name:'wait'},metadata,{deadline:new Date(Date.now()+30)},(err,x)=>err?rej(err):res(x))), e=>e.code===status.DEADLINE_EXCEEDED);
  await assert.rejects(new Promise((res,rej)=>{const call=client.get({...item,name:'wait'},metadata,(err,x)=>err?rej(err):res(x));call.cancel();}),e=>e.code===status.CANCELLED);
  const raw=createRawClient(`127.0.0.1:${port}`,credentials.createInsecure());
  const flat=createFlatClient('',credentials.createInsecure(),{transport:raw});
  const grouped=createIdiomaticClient('',credentials.createInsecure(),{transport:raw});
  assert.equal(flat.raw,raw); assert.equal(grouped.raw,raw);
  const response=flat.get(item,{metadata});assert.deepEqual(await response,item);
  assert.equal((await response.headers).get('x-header')[0],'received');
  assert.equal((await response.status).metadata.get('x-trailer')[0],'complete');
  assert.deepEqual(await grouped.store.get(item,{metadata}),item);
  const values=[];for await(const x of flat.watch(item))values.push(x);assert.equal(values.length,2);
  assert.equal((await flat.upload([item,item])).name,'2');
  const duplexValues=[];for await(const x of grouped.store.chat([item,item]))duplexValues.push(x);assert.equal(duplexValues.length,2);
  const controller=new AbortController();const cancelled=flat.get({...item,name:'wait'},{metadata,signal:controller.signal});controller.abort();
  await assert.rejects(cancelled,e=>e.code===status.CANCELLED);assert.equal((await cancelled.status).code,status.CANCELLED);
  const slow={...item,name:'slow',note:'x'.repeat(8192)};assert.equal((await flat.upload((async function*(){for(let i=0;i<128;i++)yield slow;})())).name,'128');
  const original=new Error('input producer failed');
  await assert.rejects(flat.upload((async function*(){yield item;throw original;})()),e=>e===original);
  flat.close();
  const events=[];
  const interceptor=(options,nextCall)=>new InterceptingCall(nextCall(options),{start(metadata,listener,next){events.push('start');next(metadata,{onReceiveMetadata(value,next){events.push('headers');next(value);},onReceiveMessage(value,next){events.push('message');next(value);},onReceiveStatus(value,next){events.push('status');next(value);}});}});
  const intercepted=createFlatClient(`127.0.0.1:${port}`,credentials.createInsecure(),{channelOptions:{interceptors:[interceptor]}});
  try {const receipt=intercepted.get(item,{metadata});await receipt;await receipt.status;assert.deepEqual(events,['start','headers','message','status']);events.length=0;for await(const value of intercepted.watch(item)){}assert.deepEqual(events,['start','headers','message','message','status']);events.length=0;await assert.rejects(intercepted.get({...item,name:'error'},{metadata}),error=>error.code===status.INVALID_ARGUMENT);assert.equal(events[0],'start');assert.equal(events.at(-1),'status');}finally{intercepted.close();}
  const channel=createChannel(`127.0.0.1:${port}`,credentials.createInsecure());const nice=createStoreNiceGrpcClient(channel);
  try{assert.deepEqual(await nice.get(item,{metadata:new NiceMetadata({'x-test':'ok'})}),item);const received=[];for await(const value of nice.watch(item))received.push(value);assert.equal(received.length,2);assert.equal((await nice.upload((async function*(){yield item;yield item;})())).name,'2');const echoed=[];for await(const value of nice.chat((async function*(){yield item;})()))echoed.push(value);assert.deepEqual(echoed,[item]);}finally{channel.close();}
  const nestProxy=createNestGrpcClient(`127.0.0.1:${port}`);const nest=getStoreNestClient(nestProxy);
  try{assert.deepEqual(await firstValueFrom(nest.get(item,metadata)),item);assert.equal((await firstValueFrom(nest.watch(item).pipe(toArray()))).length,2);assert.equal((await firstValueFrom(nest.upload(from([item,item])))).name,'2');assert.equal((await firstValueFrom(nest.chat(from([item,item])).pipe(toArray()))).length,2);}finally{nestProxy.close();}
  const reactiveClient=createFlatClient(`127.0.0.1:${port}`,credentials.createInsecure());
  try{assert.deepEqual(await firstValueFrom(observeUnary(()=>reactiveClient.get(item,{metadata}))),item);assert.equal((await firstValueFrom(observeStream(()=>reactiveClient.watch(item)).pipe(toArray()))).length,2);}finally{reactiveClient.close();}
  const snapshot=immutableMessage(ItemSchema,item);assert.equal(snapshot.value.largeCount,item.largeCount);assert.throws(()=>{snapshot.value.labels.first='changed';},TypeError);assert.deepEqual(snapshot.toNative(),item);const native=snapshot.toNative();native.labels.first='changed';assert.equal(snapshot.toNative().labels.first,'a');
  assert.throws(()=>safeNumber(9007199254740993n),RangeError);assert.equal(safeNumber(42n),42);const time=new Date('1969-12-31T23:59:59.001Z');assert.deepEqual(timestampToDate(dateToTimestamp(time)),time);assert.throws(()=>timestampToDate({seconds:0n,nanos:1}),RangeError);assert.equal(durationToMilliseconds({seconds:-1n,nanos:-1e6}),-1001);
  const adapted=adaptMessage({encode:value=>toBinary(ItemSchema,value),decode:bytes=>fromBinary(ItemSchema,bytes)},{toNative:value=>create(ItemSchema,{name:value}),fromNative:value=>value.name});assert.equal(adapted.decode(adapted.encode('custom')),'custom');
  const tlsServer=new Server();tlsServer.addService(StoreService,{get(call,callback){assert.equal(call.metadata.get('authorization')[0],'Bearer test');callback(null,call.request);}});
  const pem=readFileSync('test-server.pem');const key=readFileSync('test-server.key');
  const tlsPort=await new Promise((resolve,reject)=>tlsServer.bindAsync('127.0.0.1:0',ServerCredentials.createSsl(null,[{private_key:key,cert_chain:pem}],false),(error,port)=>error?reject(error):resolve(port)));
  const auth=credentials.createFromMetadataGenerator((_,callback)=>{const metadata=new Metadata();metadata.set('authorization','Bearer test');callback(null,metadata);});
  const secure=createFlatClient(`localhost:${tlsPort}`,credentials.combineChannelCredentials(credentials.createSsl(pem),auth));
  try{assert.deepEqual(await secure.get(item),item);}finally{secure.close();await new Promise(resolve=>tlsServer.tryShutdown(resolve));}
  const frames=payload=>{const data=Buffer.alloc(5+payload.length);data.writeUInt32BE(payload.length,1);data.set(payload,5);const trailers=Buffer.from('grpc-status: 0\r\n');const tail=Buffer.alloc(5+trailers.length);tail[0]=128;tail.writeUInt32BE(trailers.length,1);tail.set(trailers,5);return Buffer.concat([data,tail]);};
  const http=createServer(async(request,response)=>{const chunks=[];for await(const chunk of request)chunks.push(chunk);const body=Buffer.concat(chunks);if(request.url.startsWith('/twirp/')){response.setHeader('content-type','application/protobuf');response.end(body);}else{response.setHeader('content-type','application/grpc-web+proto');const packet=frames(body.subarray(5));for(let i=0;i<packet.length;i+=3)response.write(packet.subarray(i,i+3));response.end();}});
  await new Promise(resolve=>http.listen(0,'127.0.0.1',resolve));
  try {const endpoint=`http://127.0.0.1:${http.address().port}`;const twirp=createStoreTwirpClient(endpoint);assert.deepEqual(await twirp.get(item),item);assert.throws(()=>twirp.watch(item),/does not support/);const web=createStoreGrpcWebClient(endpoint);assert.deepEqual(await web.get(item),item);const collected=[];for await(const value of web.watch(item))collected.push(value);assert.deepEqual(collected,[item]);assert.throws(()=>web.upload(item),/does not support/);}finally{await new Promise(resolve=>http.close(resolve));}
  console.log('all grpc checks passed');
} finally { client.close(); await new Promise(res=>server.tryShutdown(res)); }
