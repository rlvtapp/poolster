# Python RPC verification

Generation is Rust-only: no Python executable, protoc, or grpcio-tools is needed.
Run ordinary generation, malformed-input, descriptor and provider-substitution
checks with:

```sh
cargo test --offline --locked -p poolster-plugin-python --lib grpc::
cargo test --offline --locked -p poolster-plugin-python --test grpc_native
```

To enable actual package wheel/install and local gRPC server tests, use Python
3.10+ in a temporary environment with runtime/build dependencies only:

```sh
python3 -m venv /tmp/poolster-python-rpc
/tmp/poolster-python-rpc/bin/pip install grpcio==1.78.0 protobuf==6.33.5 setuptools==80.9.0 wheel==0.46.3
POOLSTER_PYTHON_GRPC=/tmp/poolster-python-rpc/bin/python cargo test --offline --locked -p poolster-plugin-python --test grpc_native -- --include-ignored --nocapture
```

The native test installs its built wheel into an isolated target, verifies typing
files survive packaging, imports/compiles the emitted code, and runs a local
server on an ephemeral loopback port. It checks all four streaming shapes,
metadata, status errors, deadlines, cancellation, native proto2/proto3 presence,
defaults, extensions, oneofs, maps, enum values, pickle and official Timestamp
interoperability. Handwritten implementation files survive regeneration.

The upstream test uses the repository's pinned Protobuf corpus. To avoid
colliding with the installed regular `google.protobuf` package, it relocates
`unittest_import_public.proto` byte-for-byte and changes only the public-import
path in `unittest_import.proto`. See PROVENANCE.json for exact adaptation and the
original source hashes/license. Public aliases and explicit-zero presence are
verified in the generated modules; sources are retained in generated `proto/`.
