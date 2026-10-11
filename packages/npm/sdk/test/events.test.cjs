'use strict';
const test=require('node:test');
const assert=require('node:assert/strict');
const fs=require('node:fs/promises');
const path=require('node:path');
const {generate,defineConfig}=require('../index.cjs');
const bundle=require('../../plugins-all/index.cjs');
const {temporary}=require('../test-support/helpers.cjs');
const fixture=path.resolve(__dirname,'../../../../crates/inputs/asyncapi/tests/fixtures/kafka-json.yaml');
async function setup(t){const dir=await temporary(t);return {input:{path:fixture,plugin:bundle.inputAsyncApi()},output:path.join(dir,'out')};}
test('AsyncAPI Kafka SDK packages reuse shared models in TS Go and Rust and regenerate',async(t)=>{
 const config=await setup(t);config.plugins=[bundle.pluginTypeScript(),bundle.pluginGo(),bundle.pluginRust()];
 const first=await generate(config);assert.deepEqual(first.skipped,[]);assert.equal(first.api,null);
 for(const lang of ['typescript','go','rust'])assert.ok(first.files.some(f=>f.path.startsWith(lang+'/')&&f.contents.includes('OrderCreatedPayload')),lang);
 assert.ok(first.files.some(f=>f.path==='typescript/events.ts'));
 assert.equal(first.files.map(f=>f.contents).join('\n').match(/export type OrderCreatedPayload =/g).length,1);
 assert.ok(first.files.every(f=>typeof f.owner==='string'));
 assert.deepEqual((await generate(config)).changes,{added:[],modified:[],removed:[]});
});
test('AsyncAPI event and models-only packages compose with distinct owners',async(t)=>{
 const config=await setup(t);config.plugins=[bundle.pluginTypeScript({path:'client'}),bundle.pluginPython({path:'model-types',mode:'models'})];
 config.input.broker={kind:'kafka',brokers:['127.0.0.1:29092'],client_id:'custom-js-client'};
 const result=await generate(config,{write:false});assert.deepEqual(result.skipped,[]);
 assert.ok(result.files.find(f=>f.path==='client/events.ts').contents.includes('custom-js-client'));
 assert.ok(result.files.some(f=>f.path.startsWith('model-types/')));
});
test('AsyncAPI unsupported SDK languages skip and incompatible options fail',async(t)=>{
 const config=await setup(t);config.plugins=[bundle.pluginPython()];
 const result=await generate(config,{write:false});assert.deepEqual(result.files,[]);assert.equal(result.skipped.length,1);
 config.plugins=[bundle.pluginTypeScript({style:'flat'})];await assert.rejects(()=>generate(config),/Kafka event SDK packages/);
 config.plugins=[bundle.pluginTypeScript()];config.input.broker={kind:'rabbitmq',brokers:['localhost:9092']};await assert.rejects(()=>generate(config),/broker.kind/);
 config.input.broker={kind:'kafka',brokers:['bad']};await assert.rejects(()=>generate(config),/broker|host:port/);
 delete config.input.broker;config.input.plugin={...config.input.plugin,provider:'graphql.apollo'};await assert.rejects(()=>generate(config),/provider|format/);
});
test('JS event route compiles and roundtrips the local Kafka broker', {skip:!process.env.POOLSTER_KAFKA_NODE_MODULES||!process.env.POOLSTER_KAFKA_BROKER},async(t)=>{
 const {execFile}=require('node:child_process');const {promisify}=require('node:util');const run=promisify(execFile);
 const dir=await temporary(t),source=path.join(dir,'events.yaml'),topic=`poolster-shared-js-${process.pid}`;
 await fs.writeFile(source,(await fs.readFile(fixture,'utf8')).replaceAll('poolster-orders',topic).replaceAll('poolster-alpha2-orders',`${topic}-group`));
 const output=path.join(dir,'out');await generate({input:{path:source,plugin:bundle.inputAsyncApi()},output,plugins:[bundle.pluginTypeScript()]});
 const client=path.join(output,'typescript');await fs.symlink(process.env.POOLSTER_KAFKA_NODE_MODULES,path.join(client,'node_modules'));
 await run('node',['node_modules/typescript/bin/tsc','-p','tsconfig.json'],{cwd:client});
 const probe=await fs.readFile(path.resolve(__dirname,'../../../../crates/plugins/typescript/tests/fixtures/kafka_probe.cjs'),'utf8');
 await fs.writeFile(path.join(client,'probe.cjs'),probe.replaceAll('poolster-orders',topic));
 const {stdout}=await run('node',['probe.cjs'],{cwd:client,timeout:60000});assert.match(stdout,/real broker roundtrip/);
});

test('broker options require AsyncAPI and cannot silently reach GraphQL',()=>{
 for(const format of ['graphql','openapi','protobuf']) {
  assert.throws(()=>defineConfig({input:{path:'schema',plugin:{kind:'native-input',format,provider:'test.provider'},broker:{kind:'kafka',brokers:['localhost:9092']}},output:'out',plugins:[bundle.pluginTypeScript()]}),/broker requires an AsyncAPI input/);
 }
});
