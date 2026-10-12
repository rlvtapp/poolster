//! Symfony container integration for the runnable PHP Kafka event package.
use anyhow::{Result, ensure};
use poolster_core::{
    GeneratedFile, GeneratedTree,
    engine::{Handle, Meta, Plugin, PluginContext, Requirement},
    native::events::EventOperations,
};

pub struct AsyncApi {
    meta: Meta,
    provider: Option<Handle<EventOperations>>,
}
/// Generate a self-contained PHP Kafka SDK with Symfony DI registration.
pub fn asyncapi(provider: Option<Handle<EventOperations>>) -> AsyncApi {
    AsyncApi {
        meta: Meta::new(),
        provider,
    }
}
impl Plugin<crate::Symfony> for AsyncApi {
    fn kind(&self) -> &'static str {
        "symfony-asyncapi-kafka"
    }
    fn meta(&self) -> &Meta {
        &self.meta
    }
    fn supports_native_input(&self) -> bool {
        true
    }
    fn requires(&self) -> Vec<Requirement> {
        vec![Requirement::on(self.provider)]
    }
    fn generate(&self, cx: &mut PluginContext<'_, crate::Symfony>) -> Result<()> {
        ensure!(
            cx.settings.sdk_package.is_none(),
            "Symfony Kafka bundles include the PHP SDK; sdk_package applies to HTTP integration"
        );
        let input = cx.inputs.get::<EventOperations>()?;
        let name = cx
            .settings
            .package_name
            .as_deref()
            .unwrap_or("poolster/events-symfony");
        let reference = cx.inputs.reference::<EventOperations>()?.cloned();
        let mut tree = render_with_reference(input, name, reference)?;
        if let Some(version) = &cx.common.package_version {
            let mut manifest: serde_json::Value =
                serde_json::from_str(tree.get("composer.json").unwrap())?;
            manifest["version"] = version.clone().into();
            tree.replace(GeneratedFile::new(
                "composer.json",
                serde_json::to_string_pretty(&manifest)? + "\n",
            )?)?;
        }
        cx.files.append(tree)
    }
}
#[cfg(test)]
fn render(input: &EventOperations, package: &str) -> Result<GeneratedTree> {
    render_with_reference(input, package, None)
}
fn render_with_reference(
    input: &EventOperations,
    package: &str,
    reference: Option<poolster_core::blocks::ContractReference>,
) -> Result<GeneratedTree> {
    let mut tree =
        poolster_plugin_php::render_event_package_with_reference(input, package, reference)?;
    let namespace = poolster_plugin_php::graphql_package_namespace(package);
    let alias = crate::slug(package).replace('-', "_");
    let mut manifest: serde_json::Value = serde_json::from_str(tree.get("composer.json").unwrap())?;
    manifest["type"] = "symfony-bundle".into();
    for dependency in ["config", "dependency-injection", "http-kernel"] {
        manifest["require"][format!("symfony/{dependency}")] = "^6.4 || ^7.0".into();
    }
    // Preserve the PHP SDK namespace mapping; Symfony lives under the same PSR-4 root.
    tree.replace(GeneratedFile::new(
        "composer.json",
        serde_json::to_string_pretty(&manifest)? + "\n",
    )?)?;
    tree.insert(GeneratedFile::new("src/Symfony/PoolsterKafkaBundle.php",format!("<?php\ndeclare(strict_types=1);\nnamespace {namespace}\\Symfony;\nfinal class PoolsterKafkaBundle extends \\Symfony\\Component\\HttpKernel\\Bundle\\Bundle\n{{\n    public function getContainerExtension(): \\Symfony\\Component\\DependencyInjection\\Extension\\ExtensionInterface\n    {{\n        return new DependencyInjection\\PoolsterKafkaExtension();\n    }}\n}}\n"))?)?;
    let broker = format!(
        "'{}'",
        input
            .broker
            .brokers
            .join(",")
            .replace('\\', "\\\\")
            .replace('\'', "\\'")
    );
    tree.insert(GeneratedFile::new("src/Symfony/DependencyInjection/PoolsterKafkaExtension.php",format!("<?php\ndeclare(strict_types=1);\nnamespace {namespace}\\Symfony\\DependencyInjection;\nuse {namespace}\\Events\\KafkaRuntime;\nuse Symfony\\Component\\DependencyInjection\\ContainerBuilder;\nuse Symfony\\Component\\DependencyInjection\\Extension\\Extension;\nuse Symfony\\Component\\DependencyInjection\\Definition;\nfinal class PoolsterKafkaExtension extends Extension\n{{\n    public function getAlias(): string {{ return '{alias}_kafka'; }}\n    public function load(array $configs, ContainerBuilder $container): void\n    {{\n        $brokers = {broker};\n        foreach ($configs as $config) {{\n            foreach (array_keys($config) as $key) {{ if ($key !== 'brokers') {{ throw new \\InvalidArgumentException('unsupported Kafka configuration: '.$key); }} }}\n            if (isset($config['brokers'])) {{ $brokers = $config['brokers']; }}\n        }}\n        if (!is_string($brokers) || $brokers === '') {{ throw new \\InvalidArgumentException('Kafka brokers must be a nonempty string'); }}\n        $container->setDefinition(KafkaRuntime::class, (new Definition(KafkaRuntime::class, [$brokers]))->setPublic(true));\n    }}\n}}\n"))?)?;
    tree.insert(GeneratedFile::new("SYMFONY.md",format!("# Symfony Kafka integration\n\nRegister `{namespace}\\Symfony\\PoolsterKafkaBundle` in `config/bundles.php`. Configure `{alias}_kafka: {{ brokers: 'localhost:29092' }}`. Inject `{namespace}\\Events\\KafkaRuntime` into application services and use generated event operation classes. The bundle includes the PHP SDK and shared models.\n\nConsumer loops belong to application worker/console services. Use explicit acknowledgements and close each subscription in `finally`; close the producer runtime when the worker ends. DI registration does not start consumers, provision topics or commit offsets. Native runtime capabilities and limitations are documented in README.md.\n"))?)?;
    Ok(tree)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "requires isolated native PHP Kafka container and Composer"]
    fn generated_symfony_container_resolves_runtime() {
        let php_container =
            std::env::var("POOLSTER_KAFKA_PHP_CONTAINER").unwrap_or("poolster-events-php".into());
        let input = EventOperations {
            source: serde_json::json!({}),
            broker: poolster_core::native::events::KafkaBroker {
                brokers: vec!["127.0.0.1:29092".into()],
                client_id: None,
                bindings: serde_json::Value::Null,
            },
            operations: vec![],
        };
        let tree = render(&input, "acme/events").unwrap();
        let root = std::path::Path::new("/private/tmp/poolster-symfony-events-generated");
        for (path, source) in tree.iter() {
            let file = root.join(path);
            std::fs::create_dir_all(file.parent().unwrap()).unwrap();
            std::fs::write(file, source).unwrap();
        }
        std::fs::write(root.join("probe.php"),r#"<?php
require __DIR__.'/vendor/autoload.php';
$container=new Symfony\Component\DependencyInjection\ContainerBuilder();
$extension=new Acme\Events\Symfony\DependencyInjection\PoolsterKafkaExtension();
$extension->load([['brokers'=>'127.0.0.1:29092']],$container);
$container->compile();
$runtime=$container->get(Acme\Events\Events\KafkaRuntime::class);
if ($runtime->brokers !== '127.0.0.1:29092') {throw new RuntimeException('DI broker configuration lost');}
$runtime->close();
echo "Symfony Kafka DI runtime passed\n";
"#).unwrap();
        let status=std::process::Command::new("docker").args(["exec","-w",root.to_str().unwrap(),&php_container,"sh","-c","composer install --no-interaction --no-progress && find src -name '*.php' -exec php -l {} \\; && php probe.php"]).status().unwrap();
        assert!(status.success());
    }

    #[test]
    fn broker_literals_are_not_php_interpolation() {
        let input = EventOperations {
            source: serde_json::json!({}),
            broker: poolster_core::native::events::KafkaBroker {
                brokers: vec!["host-$environment:'\\123".into()],
                client_id: None,
                bindings: serde_json::Value::Null,
            },
            operations: vec![],
        };
        let tree = render(&input, "acme/events").unwrap();
        assert!(
            tree.get("src/Symfony/DependencyInjection/PoolsterKafkaExtension.php")
                .unwrap()
                .contains("$brokers = 'host-$environment:")
        );
        assert!(
            tree.get("src/Events/KafkaDefaults.php")
                .unwrap()
                .contains("new KafkaRuntime('host-$environment:")
        );
    }
    #[test]
    fn bundle_preserves_sdk_namespace_and_runtime() {
        let input = EventOperations {
            source: serde_json::json!({}),
            broker: poolster_core::native::events::KafkaBroker {
                brokers: vec!["127.0.0.1:29092".into()],
                client_id: None,
                bindings: serde_json::Value::Null,
            },
            operations: vec![],
        };
        let tree = render(&input, "acme/events").unwrap();
        assert!(tree.get("src/Events/KafkaRuntime.php").is_some());
        let manifest: serde_json::Value =
            serde_json::from_str(tree.get("composer.json").unwrap()).unwrap();
        assert_eq!(manifest["autoload"]["psr-4"]["Acme\\Events\\"], "src/");
        assert!(
            tree.get("src/Symfony/DependencyInjection/PoolsterKafkaExtension.php")
                .unwrap()
                .contains("KafkaRuntime::class")
        );
        assert!(tree.get(".poolster/model-contract.json").is_some());
    }
}
