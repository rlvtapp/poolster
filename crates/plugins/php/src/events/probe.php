<?php
declare(strict_types=1);
require __DIR__ . '/vendor/autoload.php';
use Acme\Events\Events\{KafkaDefaults, KafkaRuntime, KafkaCodec, SendUserEvent, ReceiveUserEvent};
use Acme\Events\Models\{UserPayload, UserKey, UserHeaders};
function check(bool $condition, string $message): void { if (!$condition) { throw new RuntimeException($message); } }
function rejects(callable $test): void { try { $test(); } catch (Throwable) { return; } throw new RuntimeException('expected rejection'); }
$definition=SendUserEvent::definition();
foreach (["{\"id\":\"one\",\"id\":\"two\"}", "{", "{\"id\":\"\xff\"}", "{\"id\":123}", "{\"id\":\"\"}", 'null', '{"id":"one","extra":1}'] as $wire) {
    rejects(fn()=>KafkaCodec::decode($definition,$wire,[],null));
}
rejects(fn()=>KafkaCodec::decode($definition,'{"id":"one"}',[],"\xff"));
rejects(fn()=>KafkaCodec::decode($definition,'{"id":"one"}',["trace"=>"\xff"],null));
rejects(fn()=>KafkaCodec::decode($definition,null,[],null));
$stats=[];
$runtime=new KafkaRuntime('127.0.0.1:29092',static function(array $statistics) use (&$stats):void {$stats[]=$statistics['name']??'';});
$metadata=SendUserEvent::send($runtime,UserPayload::fromArray(['id'=>'one']),UserHeaders::fromArray(['trace'=>'trace-1']),UserKey::from('key'));
check($metadata->offset>=0,'delivery metadata');
$group='poolster-php-'.bin2hex(random_bytes(4));
$sub=ReceiveUserEvent::subscribe($runtime,$group);
$record=$sub->poll(15000);check($record!==null,'record delivered');check($record->payload->id==='one','typed payload');check($record->key->value==='key','typed key');check($record->headers->trace==='trace-1','typed header');
rejects(fn()=>$sub->poll(1));
$sub->close();
$sub=ReceiveUserEvent::subscribe($runtime,$group);$record=$sub->poll(15000);check($record!==null && $record->payload->id==='one','unacked replay');$record->ack();rejects(fn()=>$record->ack());
$sub->cancel();check($sub->poll(1000)===null,'cancelled');$sub->close();
$sub=ReceiveUserEvent::subscribe($runtime,$group);check($sub->poll(1500)===null,'committed record not redelivered');$sub->close();
SendUserEvent::send($runtime,UserPayload::fromArray(['id'=>'two']));
$sub=ReceiveUserEvent::subscribe($runtime,$group);rejects(fn()=>$sub->run(static function($r):void { throw new RuntimeException('handler failed'); }));$sub->close();
$sub=ReceiveUserEvent::subscribe($runtime,$group);$record=$sub->poll(15000);check($record!==null && $record->payload->id==='two','handler failure replay');$record->ack();$sub->close();
$fixed=ReceiveUserEvent::definition();$fixed['groupId']=$group.'-fixed';rejects(fn()=>$runtime->subscribe($fixed,'different'));
$matched=$runtime->subscribe($fixed,$fixed['groupId']);$matchedRecord=$matched->poll(15000);check($matchedRecord!==null,'matching fixed group');$matchedRecord->ack();$matched->close();
$start=hrtime(true);$sub=ReceiveUserEvent::subscribe($runtime,$group);check($sub->poll(1000,static fn()=>true)===null,'predicate cancel');check(hrtime(true)-$start<500000000,'bounded cancellation');$sub->close();
// Malformed wire values must fail after travelling through Kafka, never committing offsets.
$rawConf=new RdKafka\Conf();$rawConf->set('bootstrap.servers','127.0.0.1:29092');
$raw=new RdKafka\Producer($rawConf);
foreach ([["{\"id\":\"\xff\"}",null,[]],['{"id":"valid"}',"\xff",[]],['{"id":"valid"}',null,['trace'=>"\xff"]],['{"id":"one","id":"two"}',null,[]]] as $n=>[$wire,$key,$headers]) {
    $invalid=$definition;$invalid['topic']=$definition['topic'].'-invalid-'.$n;
    $raw->newTopic($invalid['topic'])->producev(RD_KAFKA_PARTITION_UA,0,$wire,$key,$headers);
    check($raw->flush(10000)===RD_KAFKA_RESP_ERR_NO_ERROR,'raw invalid wire delivery');
    $invalidGroup=$group.'-invalid-'.$n;
    $sub=$runtime->subscribe($invalid,$invalidGroup);
    rejects(fn()=>$sub->poll(15000));rejects(fn()=>$sub->poll(1));$sub->close();
    $sub=$runtime->subscribe($invalid,$invalidGroup);rejects(fn()=>$sub->poll(15000));$sub->close();
}
// Native librdkafka statistics expose actual client names, including configured IDs.
$native=(new ReflectionProperty($runtime,'producers'))->getValue($runtime);
foreach ($native as $producer) {$producer->poll(250);}
check((bool)array_filter($stats,static fn($name)=>str_starts_with($name,'poolster-php-send#producer-')),'native sender client ID');
check((bool)array_filter($stats,static fn($name)=>str_starts_with($name,'poolster-php-receive#consumer-')),'native receiver client ID');
$runtime->close();rejects(fn()=>SendUserEvent::send($runtime,UserPayload::fromArray(['id'=>'closed'])));
echo "PHP Kafka compile, schema, UTF8, delivery, ack, replay, handler failure and cancellation passed\n";
