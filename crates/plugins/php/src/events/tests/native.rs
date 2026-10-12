use super::*;
#[test]
#[ignore = "requires isolated PHP 8.2 php-rdkafka container, Composer and local Kafka"]
fn generated_php_compiles_and_runs_against_kafka() {
    let php_container =
        std::env::var("POOLSTER_KAFKA_PHP_CONTAINER").unwrap_or("poolster-events-php".into());
    let broker_container =
        std::env::var("POOLSTER_KAFKA_BROKER_CONTAINER").unwrap_or("poolster-alpha2-kafka".into());
    let root = std::path::PathBuf::from("/private/tmp/poolster-php-events-generated");
    let mut input = input();
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis();
    for op in &mut input.operations {
        op.topic = format!("poolster-php-{unique}");
    }
    let tree = render_event_package(&input, "acme/events").unwrap();
    for (path, contents) in tree.iter() {
        let p = root.join(path);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, contents).unwrap();
    }
    std::fs::write(root.join("probe.php"), include_str!("../probe.php")).unwrap();
    let status = std::process::Command::new("docker")
        .args([
            "exec",
            &broker_container,
            "rpk",
            "topic",
            "create",
            &input.operations[0].topic,
            "-p",
            "1",
        ])
        .status()
        .unwrap();
    assert!(status.success());
    for n in 0..4 {
        let topic = format!("{}-invalid-{n}", input.operations[0].topic);
        let status = std::process::Command::new("docker")
            .args([
                "exec",
                &broker_container,
                "rpk",
                "topic",
                "create",
                &topic,
                "-p",
                "1",
            ])
            .status()
            .unwrap();
        assert!(status.success());
    }
    let status=std::process::Command::new("docker").args(["exec","-w",root.to_str().unwrap(),&php_container,"sh","-c","composer install --no-interaction --no-progress && find src -name '*.php' -exec php -l {} \\; && php probe.php"]).status().unwrap();
    assert!(status.success());
}
