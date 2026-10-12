use poolster_core::native::rpc::{RpcGenerationOptions, RpcWireSchema};
pub(super) fn render(
    runtime: &str,
    schema: &RpcWireSchema,
    options: &RpcGenerationOptions,
) -> anyhow::Result<(String, String)> {
    let mut source = include_str!("views.py.tmpl").replace("{{RUNTIME}}", runtime);
    let mut annotations = String::from(
        "import typing as _typing\nfrom google.protobuf.message import Message\n_T = _typing.TypeVar('_T', bound=Message)\n_V = _typing.TypeVar('_V')\n_R = _typing.TypeVar('_R')\n",
    );
    if options.immutable_views {
        annotations.push_str("class FrozenMessage(_typing.Generic[_T]):\n    def __init__(self, value: _T) -> None: ...\n    def __getattr__(self, name: str) -> _typing.Any: ...\n    def copy_native(self) -> _T: ...\n    def has_field(self, name: str) -> bool: ...\n    def which_oneof(self, name: str) -> str | None: ...\n");
    } else {
        source.push_str("\ndel FrozenMessage\n");
    }
    if options.reactive_streams {
        annotations.push_str("class ReactiveStream(_typing.Generic[_R]):\n    def __init__(self, call: _typing.AsyncIterator[_R]) -> None: ...\n    def __getattr__(self, name: str) -> _typing.Any: ...\n    def __aiter__(self) -> _typing.AsyncIterator[_R]: ...\n");
    } else {
        source.push_str("\ndel ReactiveStream\n");
    }
    if options.value_adapters {
        annotations.push_str("from datetime import datetime, timedelta\nfrom google.protobuf.timestamp_pb2 import Timestamp\nfrom google.protobuf.duration_pb2 import Duration\nclass MessageAdapter(_typing.Generic[_T, _V]):\n    def __init__(self, message_type: type[_T], to_native: _typing.Callable[[_V], _T], from_native: _typing.Callable[[_T], _V]) -> None: ...\n    def to_native(self, value: _V) -> _T: ...\n    def from_native(self, value: _T) -> _V: ...\ndef integer_from_string(value: str, *, unsigned: bool = ...) -> int: ...\ndef integer_to_string(value: int, *, unsigned: bool = ...) -> str: ...\ndef integer_to_float(value: int) -> float: ...\ndef integer_from_float(value: float, *, unsigned: bool = ...) -> int: ...\ndef bytes_to_base64(value: bytes) -> str: ...\ndef bytes_from_base64(value: str) -> bytes: ...\ndef timestamp_to_datetime(value: Timestamp) -> datetime: ...\ndef timestamp_from_datetime(value: datetime) -> Timestamp: ...\ndef duration_to_timedelta(value: Duration) -> timedelta: ...\ndef duration_from_timedelta(value: timedelta) -> Duration: ...\n");
    } else {
        source.push_str("\ndel MessageAdapter, integer_from_string, integer_to_string, integer_to_float, integer_from_float, bytes_to_base64, bytes_from_base64, timestamp_to_datetime, timestamp_from_datetime, duration_to_timedelta, duration_from_timedelta\n");
    }
    if options.immutable_views {
        let (code, types) = super::immutable::render(schema, options)?;
        source.push_str(&code);
        annotations.push_str(&types);
    }
    Ok((source, annotations))
}
