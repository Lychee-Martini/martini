use martini::converter::engine::EngineRegistry;
use martini::watcher::{DirectoryWatcher, OriginalAction, WatchRule};
use std::fs;
use std::sync::Arc;
use std::time::Duration;
use tempfile::tempdir;

#[test]
fn test_watcher_settle_and_convert() {
    let inbox = tempdir().unwrap();
    let outbox = tempdir().unwrap();
    let registry = Arc::new(EngineRegistry::default());

    let rule = WatchRule {
        input_dir: inbox.path().to_path_buf(),
        output_dir: outbox.path().to_path_buf(),
        recursive: false,
        settle_duration: Duration::from_millis(150),
        original_action: OriginalAction::Keep,
        overwrite: true,
        format_override: Some(martini::converter::Format::Yaml),
        initial_scan: false,
    };

    let watcher = DirectoryWatcher::new(rule, registry);
    let (tx, rx) = std::sync::mpsc::channel();

    let handle = std::thread::spawn(move || watcher.run_with_timeout(rx, Duration::from_secs(3)));

    // Let the watcher start listening
    std::thread::sleep(Duration::from_millis(100));
    fs::write(inbox.path().join("event.json"), r#"{"event": "ping"}"#).unwrap();

    // Wait for settle and processing
    std::thread::sleep(Duration::from_millis(600));
    let _ = tx.send(());
    let res = handle.join().unwrap();
    assert!(res.is_ok());

    let output_yaml = outbox.path().join("event.yaml");
    assert!(
        output_yaml.exists(),
        "Output yaml must be generated in outbox: {}",
        output_yaml.display()
    );
    let content = fs::read_to_string(&output_yaml).unwrap();
    assert!(content.contains("event: ping"));
}

#[test]
fn test_watcher_initial_scan() {
    let inbox = tempdir().unwrap();
    let outbox = tempdir().unwrap();
    let registry = Arc::new(EngineRegistry::default());

    // Pre-create file in inbox
    fs::write(inbox.path().join("existing.json"), r#"{"item": "pre"}"#).unwrap();

    let rule = WatchRule {
        input_dir: inbox.path().to_path_buf(),
        output_dir: outbox.path().to_path_buf(),
        recursive: false,
        settle_duration: Duration::from_millis(100),
        original_action: OriginalAction::Keep,
        overwrite: true,
        format_override: Some(martini::converter::Format::Yaml),
        initial_scan: true, // Should convert existing file on start
    };

    let watcher = DirectoryWatcher::new(rule, registry);
    let (tx, rx) = std::sync::mpsc::channel();

    let handle = std::thread::spawn(move || watcher.run_with_timeout(rx, Duration::from_secs(2)));

    std::thread::sleep(Duration::from_millis(400));
    let _ = tx.send(());
    let res = handle.join().unwrap();
    assert!(res.is_ok());

    let output_yaml = outbox.path().join("existing.yaml");
    assert!(
        output_yaml.exists(),
        "Existing file must be converted with initial_scan"
    );
}
