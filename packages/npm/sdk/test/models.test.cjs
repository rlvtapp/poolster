'use strict';
const test=require('node:test');
const assert=require('node:assert/strict');
const fs=require('node:fs/promises');
const path=require('node:path');
const {generate,sdkPackage}=require('../index.cjs');
const bundle=require('../../plugins-all/index.cjs');
const {temporary}=require('../test-support/helpers.cjs');
const factories=[bundle.pluginTypeScript,bundle.pluginRust,bundle.pluginGo,bundle.pluginPython,bundle.pluginPhp,bundle.pluginJava,bundle.pluginCSharp,bundle.pluginRuby,bundle.pluginSwift,bundle.pluginElixir];
async function graphql(t){const dir=await temporary(t);const schema=path.join(dir,'schema.graphql'),operations=path.join(dir,'read.graphql');await fs.writeFile(schema,'input GreetingInput { name: String! nickname: String } type Query { hello(input: GreetingInput!): String! }');await fs.writeFile(operations,'query Read($input: GreetingInput!) { hello(input: $input) }');return {dir,input:{path:schema,plugin:bundle.inputGraphql(),operations:[operations]}};}
async function asyncapi(t){const dir=await temporary(t),source=path.join(dir,'messages.json');await fs.writeFile(source,JSON.stringify({asyncapi:'3.0.0',info:{title:'Models without broker',version:'1.0.0'},components:{messages:{Greeting:{name:'Greeting',payload:{type:'object',required:['message'],properties:{message:{type:'string',minLength:1}},additionalProperties:false}}}}}));return {dir,input:{path:source,plugin:bundle.inputAsyncApi()}};}
test('models mode is explicit and rejects SDK runtime options',()=>{
  assert.equal(sdkPackage({language:'typescript',mode:'models'}).mode,'models');
  assert.throws(()=>sdkPackage({language:'typescript',mode:'wrong'}),/mode/);
  for(const key of ['style','transport','raw','groups','contracts','scalars','subscriptions','endpoint','clientName','jobs'])assert.throws(()=>sdkPackage({language:'typescript',mode:'models',[key]:key==='raw'?false:'value'}),/models mode does not support/);
});
test('native GraphQL models-only generates all ten package languages and regenerates',async(t)=>{
  const {dir,input}=await graphql(t);const config={input,output:path.join(dir,'out'),plugins:factories.map(factory=>factory({mode:'models'}))};
  const first=await generate(config);assert.deepEqual(first.skipped,[]);assert.equal(first.api,null);
  for(const language of ['typescript','rust','go','python','php','java','csharp','ruby','swift','elixir'])assert.ok(first.files.some(file=>file.path.startsWith(language+'/') && /\.(ts|rs|go|py|php|java|cs|rb|swift|ex)$/.test(file.path) && file.contents.includes('GreetingInput')), `${language} did not emit the actual input model`);
  assert.ok(first.files.some(file=>file.path==='typescript/package.json'));
  assert.ok(first.files.some(file=>file.path==='typescript/shared-models/schema-manifest.json'));
  assert.ok(!first.files.some(file=>/graphql-runtime|kafka-runtime/.test(file.path)));
  assert.deepEqual((await generate(config)).changes,{added:[],modified:[],removed:[]});
});
test('AsyncAPI models-only emits all languages without a broker or routing contract',async(t)=>{
  const {dir,input}=await asyncapi(t);const result=await generate({input,output:path.join(dir,'out'),plugins:factories.map(factory=>factory({mode:'models'}))},{write:false});
  assert.deepEqual(result.skipped,[]);assert.ok(!result.files.some(file=>/kafka-runtime/.test(file.path)));
  for(const language of ['typescript','rust','go','python','php','java','csharp','ruby','swift','elixir'])assert.ok(result.files.some(file=>file.path.startsWith(language+'/')),language);
  assert.ok(result.files.find(file=>file.path.endsWith('schema-manifest.json')).contents.includes('minLength'));
});
test('mixed GraphQL SDK and model packages keep file owners and detect path collisions',async(t)=>{
  const {dir,input}=await graphql(t);const config={input,output:path.join(dir,'out'),plugins:[bundle.pluginTypeScript({path:'models',mode:'models'}),bundle.pluginTypeScript({path:'client'})]};
  const first=await generate(config);assert.ok(first.files.some(file=>file.path.startsWith('models/shared-models/')));assert.ok(first.files.some(file=>file.path.startsWith('client/graphql/operations/')));
  assert.ok(first.files.every(file=>typeof file.owner==='string'));
  assert.deepEqual((await generate(config)).changes,{added:[],modified:[],removed:[]});
  config.plugins=[bundle.pluginTypeScript({path:'same',mode:'models'}),bundle.pluginTypeScript({path:'same'})];await assert.rejects(()=>generate(config),/configured twice|duplicate generated file/);
});
test('models input validates malformed sources, operation documents and provider identity',async(t)=>{
  const {dir,input}=await graphql(t),config={input,output:path.join(dir,'out'),plugins:[bundle.pluginTypeScript({mode:'models'})]};
  input.plugin={...input.plugin,provider:'asyncapi.roas'};await assert.rejects(()=>generate(config),/provider|format/i);input.plugin=bundle.inputGraphql();
  await fs.writeFile(input.operations[0],'query Invalid { missing }');await assert.rejects(()=>generate(config),/missing|operation/i);
  await fs.writeFile(input.path,'not a schema');await assert.rejects(()=>generate(config),/schema|parse|syntax/i);
});
test('unsupported models pairs skip with diagnostics and protect existing output',async(t)=>{
  const {dir,input}=await graphql(t),out=path.join(dir,'out');await fs.mkdir(out);await fs.writeFile(path.join(out,'keep.txt'),'keep');
  const result=await generate({input,output:out,plugins:[{kind:'native-sdk',name:'models-postman',package:{language:'postman',mode:'models',path:'postman'}}]});
  assert.deepEqual(result.files,[]);assert.equal(result.skipped.length,1);assert.equal(await fs.readFile(path.join(out,'keep.txt'),'utf8'),'keep');
});
