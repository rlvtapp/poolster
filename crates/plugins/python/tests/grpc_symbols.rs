use poolster_core::{
    GeneratedFile,
    engine::{Handle, Meta, Package, Packages, Plugin, PluginContext, Requirement},
    input::{InputProvider, InputRegistry},
    native::rpc::RpcContract,
};
use poolster_plugin_python::{ProtobufSymbols, Python, grpc};
use std::{path::Path, sync::Arc};
struct Consumer {
    meta: Meta,
    input: Handle<ProtobufSymbols>,
}
impl Plugin<Python> for Consumer {
    fn kind(&self) -> &'static str {
        "python-rpc-symbol-consumer"
    }
    fn meta(&self) -> &Meta {
        &self.meta
    }
    fn supports_native_input(&self) -> bool {
        true
    }
    fn requires(&self) -> Vec<Requirement> {
        vec![Requirement::on(Some(self.input))]
    }
    fn generate(&self, cx: &mut PluginContext<Python>) -> anyhow::Result<()> {
        let symbols = cx.inputs.get::<ProtobufSymbols>()?;
        assert!(symbols.source.is_some());
        let detail = &symbols.messages["store.v1.Item.Detail"];
        assert_eq!(detail.module, "common_pb2");
        assert_eq!(detail.class, "Item.Detail");
        assert_eq!(detail.source_path.as_deref(), Some("src/common_pb2.py"));
        assert!(
            symbols.messages["google.protobuf.Timestamp"]
                .source_path
                .is_none()
        );
        cx.files.emit(GeneratedFile::new(
            "companion.py",
            format!(
                "from {} import Item\nDetail = {}\n",
                detail.module, detail.class
            ),
        )?)
    }
}
#[test]
fn companion_uses_typed_native_symbols_and_contract_provenance() {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/grpc/service.proto");
    let mut registry = InputRegistry::new();
    registry
        .register(poolster_input_protobuf::ProtobufInput)
        .unwrap();
    let input = InputProvider::<RpcContract>::new(Arc::new(registry), "protobuf", source);
    let generator = grpc("symbols").input(input.handle());
    let consumer = Consumer {
        meta: Meta::new(),
        input: generator.handle(),
    };
    let tree = Packages::new()
        .package(
            Package::<Python>::new("sdk")
                .with(consumer)
                .with(generator)
                .with(input),
        )
        .generate_native()
        .unwrap();
    assert_eq!(
        tree.get("sdk/companion.py").unwrap(),
        "from common_pb2 import Item\nDetail = Item.Detail\n"
    );
}
