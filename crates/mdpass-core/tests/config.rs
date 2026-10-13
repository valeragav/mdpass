use std::fs;
use std::path::PathBuf;

use mdpass_core::config::{AppConfig, Theme};
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
    let config = AppConfig {
        last_vault: Some(PathBuf::from("/home/u/vault.md.age")),
        auto_lock_minutes: 15,
        theme: Theme::Light,
        ..AppConfig::default()
    };
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

#[test]
fn missing_keys_take_defaults_from_the_design() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("config.json");
    fs::write(&path, r#"{"auto_lock_minutes": 1}"#).unwrap();
    let config = AppConfig::load(&path);
    assert_eq!(config.auto_lock_minutes, 1);
    assert_eq!(config.clipboard_clear_seconds, 30);
    assert!(config.lock_on_minimize && config.backups_enabled);
    assert_eq!(config.backups_to_keep, 30);
}

#[test]
fn hand_edited_values_are_brought_into_range() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("config.json");
    fs::write(&path, r#"{"backups_to_keep": 0, "accent": "red", "auto_lock_minutes": 999999}"#).unwrap();
    let config = AppConfig::load(&path);
    assert_eq!(config.backups_to_keep, 1);
    assert_eq!(config.accent, AppConfig::default().accent);
    assert_eq!(config.auto_lock_minutes, 24 * 60);
}

#[test]
fn sidebar_width_stays_in_range() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("config.json");
    fs::write(&path, r#"{"sidebar_width": 5000}"#).unwrap();
    assert_eq!(AppConfig::load(&path).sidebar_width, 640);
    fs::write(&path, r#"{"sidebar_width": 10}"#).unwrap();
    assert_eq!(AppConfig::load(&path).sidebar_width, 220);
    // В настройках старой версии ширины нет: берётся обычная.
    fs::write(&path, r#"{"auto_lock_minutes": 5}"#).unwrap();
    assert_eq!(AppConfig::load(&path).sidebar_width, 320);
}
