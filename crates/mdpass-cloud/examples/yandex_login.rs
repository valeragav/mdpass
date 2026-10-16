//! Проверка доступа к Яндекс Диску с готовым токеном:
//! `cargo run -p mdpass-cloud --example yandex_login -- <токен>`
//!
//! Токен получают на странице Яндекса заранее. Пример только проверяет доступ
//! к папке приложения, сам токен не выводит и нигде не сохраняет.

use mdpass_cloud::yandex::YandexDisk;
use mdpass_core::cloud::CloudStore;

fn main() {
    let Some(token) = std::env::args().nth(1) else {
        eprintln!("Использование: yandex_login <токен>");
        std::process::exit(1);
    };

    let mut disk = YandexDisk::new(token, "mdpass-check.md.age");
    match disk.version() {
        Ok(None) => println!("Доступ к папке приложения есть, файла проверки там нет — так и должно быть"),
        Ok(Some(version)) => println!("Доступ к папке приложения есть, файл проверки уже лежит: {version}"),
        Err(err) => eprintln!("Доступа к Диску нет: {err}"),
    }
}
