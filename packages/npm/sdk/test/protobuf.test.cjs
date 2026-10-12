'use strict';
const test=require('node:test');
const assert=require('node:assert/strict');
const path=require('node:path');
const {generate,defineConfig,sdkPackage}=require('../index.cjs');
const bundle=require('../../plugins-all/index.cjs');
const {temporary}=require('../test-support/helpers.cjs');
const root=path.resolve(__dirname,'../../../../');
const fixture=path.join(root,'crates/inputs/protobuf/tests/fixtures/rpc/service.proto');
async function setup(t){const dir=await temporary(t);return {input:{path:fixture,importRoots:[path.dirname(fixture)],plugin:bundle.inputProtobuf()},output:path.join(dir,'out')};}
test('Protobuf unsupported language is empty with an explicit skipped report',async(t)=>{
 const config=await setup(t);config.plugins=[bundle.pluginPhp()];const result=await generate(config,{write:false});assert.deepEqual(result.files,[]);assert.equal(result.skipped.length,1);assert.match(result.skipped[0].reason,/protobuf/);
});
test('Protobuf rejects HTTP styles, wrong toolchains and incompatible input options',async(t)=>{
 const config=await setup(t);config.plugins=[bundle.pluginPython({style:'flat'})];await assert.rejects(()=>generate(config),/Protobuf SDK packages/);
 config.plugins=[bundle.pluginGo()];await assert.rejects(()=>generate(config),/requires contractOptions.protobuf.module/);
 config.plugins=[bundle.pluginPython({contractOptions:{protobuf:{toolchain:{protoc:'protoc'}}}})];await assert.rejects(()=>generate(config),/Python Protobuf supports/);
 assert.throws(()=>defineConfig({...config,input:{...config.input,operations:['read.graphql']}}),/Protobuf input does not support operations/);
 config.input.plugin={...config.input.plugin,provider:'graphql.apollo'};config.plugins=[bundle.pluginPython()];await assert.rejects(()=>generate(config),/provider|format/);
});
test('Protobuf Python Node route generates canonical files and regenerates', {} ,async(t)=>{
 const config=await setup(t);config.plugins=[bundle.pluginPython({contractOptions:{protobuf:{packageName:'example-rpc'}}})];
 const first=await generate(config);assert.deepEqual(first.skipped,[]);assert.equal(first.api,null);
 assert.ok(first.files.some(f=>f.path.endsWith('_pb2.py')));assert.ok(first.files.some(f=>f.path.endsWith('_pb2_grpc.py')));assert.ok(first.files.every(f=>typeof f.owner==='string'));
 assert.deepEqual((await generate(config)).changes,{added:[],modified:[],removed:[]});
 const report=first.files.find(f=>f.path.endsWith('pyproject.toml'));assert.ok(report);assert.match(report.contents,/grpcio/);
});

test('Protobuf generator configuration validates executable paths and names',()=>{
 assert.throws(()=>sdkPackage({language:'python',contractOptions:{protobuf:{toolchain:{unknown:'binary'}}}}),/unknown Protobuf tool/);
 assert.throws(()=>sdkPackage({language:'go',contractOptions:{protobuf:{module:''}}}),/nonempty string/);
 assert.throws(()=>sdkPackage({language:'typescript',contractOptions:{protobuf:{raw:true}}}),/unknown Protobuf option/);
 assert.throws(()=>sdkPackage({language:'python',mode:'models',contractOptions:{protobuf:{packageName:'client'}}}),/models mode does not support contractOptions/);
});

test('Protobuf TypeScript Node route generates streaming stubs and regenerates', {} ,async(t)=>{
 const config=await setup(t);config.plugins=[bundle.pluginTypeScript()];
 const first=await generate(config);assert.deepEqual(first.skipped,[]);
 assert.ok(first.files.some(f=>f.path.endsWith('service.ts')));assert.ok(first.files.some(f=>f.path.endsWith('package.json')));
 assert.ok(first.files.map(f=>f.contents).join('\n').includes('ClientReadableStream'));
 assert.deepEqual((await generate(config)).changes,{added:[],modified:[],removed:[]});
});
test('Protobuf Go Node route preserves upstream generation and regenerates', {skip:!process.env.POOLSTER_PROTOC||!process.env.POOLSTER_PROTOC_GEN_GO||!process.env.POOLSTER_PROTOC_GEN_GO_GRPC},async(t)=>{
 const config=await setup(t);config.plugins=[bundle.pluginGo({contractOptions:{protobuf:{module:'example.com/js-rpc',goPackages:{'service.proto':'example.com/js-rpc/demo','common.proto':'example.com/js-rpc/demo'},toolchain:{protoc:process.env.POOLSTER_PROTOC,protocGenGo:process.env.POOLSTER_PROTOC_GEN_GO,protocGenGoGrpc:process.env.POOLSTER_PROTOC_GEN_GO_GRPC}}}})];
 const first=await generate(config);assert.deepEqual(first.skipped,[]);
 assert.ok(first.files.some(f=>f.path.endsWith('.pb.go')));assert.ok(first.files.some(f=>f.path.endsWith('_grpc.pb.go')));
 assert.deepEqual((await generate(config)).changes,{added:[],modified:[],removed:[]});
});
