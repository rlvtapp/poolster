const assert = require('node:assert/strict');
const { Kafka } = require('kafkajs');
(async () => {
  const sdk = await import('./dist/index.js');
  let eachMessage;
  const original = Kafka.prototype.consumer;
  Kafka.prototype.consumer = () => ({
    connect: async () => {}, subscribe: async () => {},
    run: async options => { eachMessage = options.eachMessage; },
    disconnect: async () => {},
  });
  const client = sdk.createKafkaClient();
  const errors = [], accepted = [];
  try {
    await sdk.receiveOrder(client, value => accepted.push(value), {
      onInvalidMessage: error => errors.push(error),
    });
    const valid = () => ({ value: Buffer.from('{"id":"42","quantity":1}'), key: Buffer.from('order-42'), headers: { trace: Buffer.from('trace') } });
    const malformed = [
      ['payload', { ...valid(), value: Buffer.concat([Buffer.from('{"id":"'), Buffer.from([0xff]), Buffer.from('","quantity":1}')]) }],
      ['key', { ...valid(), key: Buffer.from([0xc0, 0xaf]) }],
      ['header trace', { ...valid(), headers: { trace: Buffer.from([0xe2, 0x82]) } }],
    ];
    for (const [field, message] of malformed) {
      await eachMessage({ topic: 'test', partition: 0, message });
      assert.equal(errors.length, malformed.findIndex(item => item[0] === field) + 1);
      assert.match(errors.at(-1).message, new RegExp(`${field} contains invalid UTF-8`));
      assert.equal(accepted.length, 0);
    }
    await eachMessage({ topic: 'test', partition: 0, message: { ...valid(), value: Buffer.from('{"id":"café😀","quantity":1}'), key: Buffer.from('clé'), headers: { trace: Buffer.from('日本語') } } });
    assert.equal(accepted[0].payload.id, 'café😀');
    assert.equal(accepted[0].key, 'clé');
    assert.equal(accepted[0].headers.trace, '日本語');
    assert.equal(errors.length, 3);
    console.log('Strict payload/key/header UTF-8 rejection and Unicode preservation passed');
  } finally {
    Kafka.prototype.consumer = original;
    await client.disconnect();
  }
})().catch(error => { console.error(error); process.exitCode = 1; });
