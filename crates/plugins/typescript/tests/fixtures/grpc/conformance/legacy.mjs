import assert from 'node:assert/strict';
import {isFieldSet,getExtension,setExtension,hasExtension,getOption} from '@bufbuild/protobuf';
import {EnvelopeCreate,EnvelopeSchema,EnvelopeBinary,EnvelopeJson,Ext_legacy_tag,Ext_legacy_values,Ext_legacy_marker} from './dist/legacy.js';
const unset=EnvelopeCreate({requiredValue:1});assert.equal(unset.defaultValue,'fallback');assert.equal(isFieldSet(unset,EnvelopeSchema.field.defaultValue),false);
const explicit=EnvelopeCreate({requiredValue:1,defaultValue:'fallback',details:{text:'group'}});assert.equal(isFieldSet(explicit,EnvelopeSchema.field.defaultValue),true);
assert.deepEqual(EnvelopeBinary.decode(EnvelopeBinary.encode(explicit)),explicit);
assert.throws(()=>EnvelopeBinary.encode(EnvelopeCreate()),/required/i);
assert.throws(()=>EnvelopeBinary.decode(new Uint8Array()),/required/i);
setExtension(explicit,Ext_legacy_tag,'registered');setExtension(explicit,Ext_legacy_values,[1n,9223372036854775807n]);
const decoded=EnvelopeBinary.decode(EnvelopeBinary.encode(explicit));assert.equal(getExtension(decoded,Ext_legacy_tag),'registered');assert.deepEqual(getExtension(decoded,Ext_legacy_values),[1n,9223372036854775807n]);assert.ok(hasExtension(decoded,Ext_legacy_tag));
assert.equal(getOption(EnvelopeSchema,Ext_legacy_marker),'descriptor retained');
assert.deepEqual(EnvelopeJson.parse(EnvelopeJson.stringify(decoded)),decoded);
// Independently encoded proto2 group start=3/end=3 with text field4.
const group=EnvelopeBinary.decode(Uint8Array.from([8,1,27,34,1,120,28]));assert.equal(group.details.text,'x');
console.log('groups extensions presence passed');
