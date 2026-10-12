'use strict';
const path=require('node:path');
const root=path.resolve(__dirname,'../..');
const {generate}=require(path.join(root,'packages/npm/sdk/index.cjs'));
const plugins=require(path.join(root,'packages/npm/plugins-all/index.cjs'));
const fixture=path.join(root,'crates/inputs/protobuf/tests/fixtures/conformance');
generate({
  input:{path:path.join(fixture,'portable.proto'),importRoots:[fixture],plugin:plugins.inputProtobuf()},
  output:process.argv[2],
  plugins:[
    plugins.pluginTypeScript({path:'typescript'}),
    plugins.pluginPython({path:'python',contractOptions:{protobuf:{packageName:'poolster-conformance'}}}),
    plugins.pluginGo({path:'go',contractOptions:{protobuf:{module:'example.com/poolster/conformance'}}}),
  ],
}).then(result=>{
  if(result.skipped.length)throw new Error(JSON.stringify(result.skipped));
  console.log(JSON.stringify({files:result.files.length}));
}).catch(error=>{console.error(error);process.exitCode=1;});
