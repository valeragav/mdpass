use std::fs;
use std::path::PathBuf;

use mdpass_core::config::AppConfig;
use tempfile::TempDir;

#[test]
fn missing_or_broken_config_gives_defaults() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("config.json");
    assert_eq!(AppConfig::load(&path), AppConfig::default());
    fs::write(&path, "{ не json").unwrap();
    assert_eq!(AppConfig::load(&path), AppConfig::default());
}

#[test]
fn saved_config_loads_back_and_creates_its_folder() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("nested").join("config.json");
    let config = AppConfig { last_vault: Some(PathBuf::from("/home/u/vault.md.age")) };
    config.save(&path).unwrap();
    assert_eq!(AppConfig::load(&path), config);
}

#[test]
fn unknown_keys_are_ignored() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("config.json");
    fs::write(&path, r#"{"last_vault": "/v.age", "from_a_newer_version": 1}"#).unwrap();
    assert_eq!(AppConfig::load(&path).last_vault, Some(PathBuf::from("/v.age")));
}
