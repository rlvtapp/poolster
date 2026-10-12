from datetime import datetime, timezone
import common_pb2 as n
import common_values as v
import representation_pb2 as rn
import representation_values as rv

def rejects(action):
    try:
        action()
    except (ValueError, TypeError, OverflowError):
        return
    raise AssertionError('lossy or invalid value accepted')

native = rn.Value(signed=42, unsigned=2**64-2048)
native.elapsed.seconds = 1
native.elapsed.nanos = 100000000
public = rv.Value.from_native(native)
assert public.signed == 42.0 and public.unsigned == float(2**64-2048)
assert public.elapsed == 1.1 and public.to_native() == native
for number in [2**53+1, 2**63-1]:
    rejects(lambda: rv.Value.from_native(rn.Value(signed=number)))
for number in [42, 1.1, float('inf')]:
    rejects(lambda: rv.Value(signed=number).to_native())
public.unsigned = float(2**64)
rejects(public.to_native)
precise = rn.Value()
precise.elapsed.seconds = 315576000000
precise.elapsed.nanos = 1
rejects(lambda: rv.Value.from_native(precise))
for seconds in [float('inf'), float('nan'), 1.0000000001]:
    rejects(lambda: rv.Value(elapsed=seconds).to_native())
item = n.Item()
item.created_at.FromJsonString('2020-01-01T00:00:00.123456Z')
model = v.Item.from_native(item)
assert model.created_at == datetime(2020, 1, 1, microsecond=123456, tzinfo=timezone.utc)
assert model.to_native() == item
model.created_at = datetime(2020, 1, 1)
rejects(model.to_native)
item.created_at.nanos = 1
rejects(lambda: v.Item.from_native(item))
