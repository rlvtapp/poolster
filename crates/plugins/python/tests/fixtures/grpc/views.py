import asyncio
from datetime import datetime, timezone, timedelta
from google.protobuf.timestamp_pb2 import Timestamp
from google.protobuf.duration_pb2 import Duration
import common_pb2 as models
import views_poolster_rpc_test as views
native=models.Item(name='native',labels={'key':'value'},children=[models.Item.Detail(value='child')])
frozen=views.FrozenMessage(native); native.name='changed'
assert frozen.name=='native' and frozen.children[0].value=='child' and frozen.labels['key']=='value'
for change in [lambda:setattr(frozen,'name','mutation'),lambda:frozen.labels.__setitem__('key','mutation'),lambda:setattr(frozen.children[0],'value','mutation')]:
    try:change()
    except (AttributeError,TypeError):pass
    else:raise AssertionError('immutable view mutated')
copy=frozen.copy_native();copy.name='copy';assert frozen.name=='native'
typed=views.freeze(copy); assert isinstance(typed,views.Frozen_store_v1_Item)
assert isinstance(typed.children[0],views.Frozen_store_v1_Item_Detail)
try:typed.children[0].value='mutation'
except AttributeError:pass
else:raise AssertionError('typed immutable child mutated')
for number in [-(1<<63),0,(1<<63)-1]:assert views.integer_from_string(views.integer_to_string(number))==number
assert views.integer_from_string(str((1<<64)-1),unsigned=True)==(1<<64)-1
for invalid in [' 1','1.0','9223372036854775808','01']:
    try:views.integer_from_string(invalid)
    except ValueError:pass
    else:raise AssertionError('invalid integer accepted')
try:views.integer_to_float((1<<53)+1)
except ValueError:pass
else:raise AssertionError('lossy float accepted')
assert views.integer_from_float(12.0)==12
assert views.bytes_from_base64(views.bytes_to_base64(b'\0\xff'))==b'\0\xff'
try:views.bytes_from_base64('invalid!')
except ValueError:pass
else:raise AssertionError('invalid base64 accepted')
date=datetime(2024,1,2,3,4,5,678901,tzinfo=timezone.utc)
assert views.timestamp_to_datetime(views.timestamp_from_datetime(date))==date
for timestamp in [Timestamp(seconds=1,nanos=1),Timestamp(seconds=253402300800)]:
    try:views.timestamp_to_datetime(timestamp)
    except ValueError:pass
    else:raise AssertionError('lossy/out-of-range timestamp accepted')
try:views.timestamp_from_datetime(date.replace(tzinfo=None))
except ValueError:pass
else:raise AssertionError('naive datetime accepted')
duration=timedelta(seconds=-2,microseconds=10)
assert views.duration_to_timedelta(views.duration_from_timedelta(duration))==duration
try:views.duration_to_timedelta(Duration(nanos=1))
except ValueError:pass
else:raise AssertionError('lossy duration accepted')
adapter=views.MessageAdapter(models.Item,lambda name:models.Item(name=name),lambda message:message.name)
assert adapter.from_native(adapter.to_native('adapted'))=='adapted'
class Stream:
    def __init__(self):self.cancelled=False
    def cancel(self):self.cancelled=True
    async def initial_metadata(self):return (('metadata','retained'),)
    async def __aiter__(self):
        yield models.Item(name='one');yield models.Item(name='two')
async def check():
    native=Stream();reactive=views.ReactiveStream(native)
    assert await reactive.initial_metadata()==(('metadata','retained'),)
    iterator=reactive.__aiter__();assert (await anext(iterator)).name=='one'
    await iterator.aclose();assert native.cancelled
asyncio.run(check())
