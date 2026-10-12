import assert from 'node:assert/strict';
import {ScalarsCreate,ScalarsBinary,ScalarsValueCodec,WellKnownCreate,WellKnownValueCodec} from './dist/wire.js';
import {TimestampSchema} from './dist/google/protobuf/timestamp.js';
import {DurationSchema} from './dist/google/protobuf/duration.js';
import {create,equals} from '@bufbuild/protobuf';
import {ScalarsSchema} from './dist/wire.js';
const native=ScalarsCreate({int64Value:9007199254740993n,uint64Value:18446744073709551615n,bytesValue:Uint8Array.of(0,255),wideKeys:{'9007199254740993':'wide'},kind:987,optionalValue:0,node:{name:'child'}});
const view=ScalarsValueCodec.fromNative(native);assert.equal(view.int64Value,'9007199254740993');assert.equal(view.bytesValue,'AP8=');assert.equal(view.wideKeys.get('9007199254740993'),'wide');assert.deepEqual(view.kind,{number:987,displayName:'UNRECOGNIZED'});assert.equal(view.optionalValue,0);assert.equal(view.node.name,'child');assert.equal(view.node.child,null);assert.ok(equals(ScalarsSchema,ScalarsValueCodec.toNative(view),native));assert.deepEqual(ScalarsValueCodec.decode(ScalarsValueCodec.encode(view)),view);assert.deepEqual(ScalarsValueCodec.parse(ScalarsValueCodec.stringify(view)),view);
const future=ScalarsBinary.decode(Uint8Array.of(160,6,42));assert.deepEqual(ScalarsValueCodec.encode(ScalarsValueCodec.fromNative(future)),ScalarsBinary.encode(future));
assert.throws(()=>ScalarsValueCodec.toNative({...view,int64Value:'9223372036854775808'}),RangeError);assert.throws(()=>ScalarsValueCodec.toNative({...view,kind:{name:'BOGUS'}}),RangeError);assert.throws(()=>ScalarsValueCodec.toNative({...view,bytesValue:'%%%'}));

const wkt=WellKnownCreate({timestamp:create(TimestampSchema,{seconds:0n,nanos:123456789}),duration:create(DurationSchema,{seconds:-1n,nanos:-1000000})});const wkView=WellKnownValueCodec.fromNative(wkt);assert.equal(wkView.timestamp,'1970-01-01T00:00:00.123456789Z');assert.equal(wkView.duration,-1.001);assert.deepEqual(WellKnownValueCodec.toNative(wkView),wkt);

// Presence follows current values/tags after editing a decoded model.
const absent=ScalarsValueCodec.fromNative(ScalarsCreate());
absent.optionalValue=0;
const optionalEdited=ScalarsValueCodec.toNative(absent);
assert.equal(optionalEdited.optionalValue,0);
const unionEdited=ScalarsValueCodec.fromNative(native);
unionEdited.alternative={case:'integer',value:123};
assert.deepEqual(ScalarsValueCodec.toNative(unionEdited).alternative,{case:'integer',value:123});
const retained=ScalarsValueCodec.clone(ScalarsValueCodec.fromNative(future));
assert.deepEqual(ScalarsValueCodec.encode(retained),ScalarsBinary.encode(future));
