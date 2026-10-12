import {create,equals} from '@bufbuild/protobuf';
import {Server,ServerCredentials,credentials} from '@grpc/grpc-js';
import {writeFileSync} from 'node:fs';
import {inspect} from 'node:util';
import {ConformanceService,ConformanceClient} from './dist/portable.grpc.js';
import {ScalarValuesSchema} from './dist/portable.js';

const value=()=>create(ScalarValuesSchema,{
  int64Value:-9223372036854775808n,uint64Value:18446744073709551615n,
  sint64Value:-9007199254740993n,fixed64Value:18446744073709551615n,
  sfixed64Value:-9223372036854775808n,int32Value:-2147483648,uint32Value:4294967295,
  sint32Value:-2147483648,fixed32Value:4294967295,sfixed32Value:-2147483648,
  floatValue:1.5,doubleValue:-2.25,boolValue:true,stringValue:'wire 😀',
  bytesValue:Uint8Array.from([0,255,128]),optionalValue:'',packedValues:[0,1,-1,2147483647],
  unpackedValues:[-2147483648,2],counters:{max:18446744073709551615n},
  recursive:{stringValue:'child'},choice:{case:'text',value:'chosen'},state:12345,renamed:'alias',
});
const same=(actual,expected)=>{if(Array.isArray(expected) ? !Array.isArray(actual)||actual.length!==expected.length||actual.some((item,index)=>!equals(ScalarValuesSchema,item,expected[index])) : !equals(ScalarValuesSchema,actual,expected))throw new Error(`independent peer changed message\nactual: ${inspect(actual,{depth:8})}\nexpected: ${inspect(expected,{depth:8})}`);};
function collect(call) {return new Promise((resolve,reject)=>{
  const values=[];call.on('data',x=>values.push(x));call.on('error',reject);call.on('end',()=>resolve(values));
});}
async function client(address) {
  const peer=new ConformanceClient(address,credentials.createInsecure());
  const request=value();
  try {
    const response=await new Promise((resolve,reject)=>peer.echo(request,{deadline:Date.now()+10000},(error,response)=>error?reject(error):resolve(response)));
    same(response,request);
    same(await collect(peer.watch(request,{deadline:Date.now()+10000})),[request,request]);
    const upload=await new Promise((resolve,reject)=>{
      const call=peer.upload({deadline:Date.now()+10000},(error,response)=>error?reject(error):resolve(response));
      call.write(request);call.write(request);call.end();
    });same(upload,request);
    const chat=peer.chat({deadline:Date.now()+10000});const received=collect(chat);
    chat.write(request);chat.write(request);chat.end();same(await received,[request,request]);
  } finally {peer.close();}
}
async function server(ready) {
  const peer=new Server();peer.addService(ConformanceService,{
    echo:(call,callback)=>callback(null,call.request),
    watch:call=>{call.write(call.request);call.write(call.request);call.end();},
    upload:(call,callback)=>{let result=value();call.on('data',x=>result=x);call.on('end',()=>callback(null,result));},
    chat:call=>{call.on('data',x=>call.write(x));call.on('end',()=>call.end());},
    wellKnown:(call,callback)=>callback(null,call.request),
  });
  const port=await new Promise((resolve,reject)=>peer.bindAsync('127.0.0.1:0',ServerCredentials.createInsecure(),(error,port)=>error?reject(error):resolve(port)));
  writeFileSync(ready,`127.0.0.1:${port}`);
  process.on('SIGTERM',()=>{peer.forceShutdown();process.exit(0);});
}
if(process.argv[2]==='server')await server(process.argv[3]);else await client(process.argv[3]);
