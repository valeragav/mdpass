//! Создаёт демо-хранилище с выдуманными записями, чтобы попробовать приложение.
//!
//! ```text
//! cargo run -p mdpass-core --example demo_vault -- <путь к файлу> <мастер-пароль>
//! ```
//!
//! Адреса взяты из диапазонов для документации (RFC 5737), пароли случайные.

use std::path::PathBuf;
use std::process::ExitCode;

use mdpass_core::format::{EntryDraft, Field};
use mdpass_core::password;
use mdpass_core::vault::{Passphrase, Vault, VaultOptions};

fn entry(path: &[&str], title: &str, fields: &[(&str, &str)], tags: &[&str]) -> EntryDraft {
    EntryDraft {
        path: path.iter().map(|s| s.to_string()).collect(),
        title: title.into(),
        fields: fields
            .iter()
            .map(|(key, value)| {
                let value = if *value == "*" { password::generate(16).as_str().to_owned() } else { value.to_string() };
                Field::new(*key, value)
            })
            .collect(),
        tags: tags.iter().map(|s| s.to_string()).collect(),
        favorite: false,
    }
}

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let (Some(path), Some(passphrase)) = (args.next(), args.next()) else {
        eprintln!("использование: demo_vault <путь к файлу> <мастер-пароль>");
        return ExitCode::FAILURE;
    };
    let path = PathBuf::from(path);
    let mut vault = match Vault::create(&path, Passphrase::from(passphrase), VaultOptions::default()) {
        Ok(vault) => vault,
        Err(err) => {
            eprintln!("не удалось создать {}: {err}", path.display());
            return ExitCode::FAILURE;
        }
    };

    let entries = [
        entry(&["work", "server"], "web-01 (прод)", &[("ssh", "deploy@203.0.113.10"), ("password", "*")], &["prod"]),
        entry(&["work", "server"], "web-02 (тест)", &[("ssh", "deploy@198.51.100.24"), ("password", "*")], &["dev"]),
        entry(
            &["work", "server"],
            "БД каталога",
            &[
                ("port", "5432"),
                ("user", "catalog_app"),
                ("password", "*"),
                ("note", "таблицу catalog_tmp можно удалять"),
            ],
            &[],
        ),
        entry(
            &["work", "site"],
            "example.test",
            &[("url", "https://example.test/"), ("login", "admin"), ("password", "*")],
            &["work", "prod"],
        ),
        entry(&["my"], "почта", &[("login", "me@example.test"), ("password", "*")], &[]),
    ];
    for draft in entries {
        vault.document_mut().add(draft).expect("демо-запись корректна");
    }
    if let Err(err) = vault.save() {
        eprintln!("не удалось записать {}: {err}", path.display());
        return ExitCode::FAILURE;
    }
    println!("создано: {}", path.display());
    ExitCode::SUCCESS
}
