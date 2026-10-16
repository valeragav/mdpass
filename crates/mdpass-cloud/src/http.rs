//! Общее для HTTP-облаков: клиент с таймаутами и разбор ошибок.

use std::time::Duration;

use mdpass_core::cloud::CloudError;
use ureq::Agent;

/// Клиент для запросов к облаку. Ошибочный ответ сервера — не ошибка запроса:
/// его разбирает облако, по коду и телу ответа.
pub(crate) fn agent() -> Agent {
    Agent::config_builder()
        .http_status_as_error(false)
        .timeout_connect(Some(Duration::from_secs(15)))
        .timeout_global(Some(Duration::from_secs(120)))
        .user_agent(concat!("mdpass/", env!("CARGO_PKG_VERSION")))
        .build()
        .into()
}

/// Ошибка соединения: нет сети, облако не отвечает, таймаут.
pub(crate) fn offline(err: ureq::Error) -> CloudError {
    CloudError::Offline(err.to_string())
}

/// Ответ сервера с ошибкой. Тело в текст ошибки не попадает целиком: в нём бывают
/// служебные данные, а пользователю хватит кода и описания.
pub(crate) fn failed(status: u16, body: &str) -> CloudError {
    if status == 401 {
        return CloudError::Unauthorized;
    }
    let description = serde_json::from_str::<serde_json::Value>(body).ok().and_then(|json| {
        ["description", "message", "error_description", "error"]
            .iter()
            .find_map(|key| json.get(key).and_then(|v| v.as_str()).map(str::to_string))
            // Google оборачивает ошибку в объект: `{"error": {"message": "…"}}`.
            .or_else(|| json.get("error").and_then(|e| e.get("message")).and_then(|v| v.as_str()).map(str::to_string))
    });
    match (status, description) {
        (500.., Some(text)) | (429, Some(text)) => CloudError::Offline(format!("{status}: {text}")),
        (500.., None) | (429, None) => CloudError::Offline(format!("ответ {status}")),
        (_, Some(text)) => CloudError::Other(format!("{status}: {text}")),
        (_, None) => CloudError::Other(format!("ответ {status}")),
    }
}

/// Хеш MD5 строкой из шестнадцатеричных цифр, как его отдаёт облако.
pub(crate) fn md5_hex(bytes: &[u8]) -> String {
    use md5::{Digest, Md5};
    Md5::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}
