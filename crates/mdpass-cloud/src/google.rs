//! Google Drive. Доступ только к служебной папке приложения (`appDataFolder`):
//! обычные файлы и папки пользователя mdpass не видит и не может увидеть —
//! `appDataFolder` недоступна даже через веб-интерфейс Google Drive.
//!
//! Вход свой: пользователь получает OAuth-токен сам (создаёт проект в Google Cloud
//! Console, включает Drive API, добавляет себя как тестового пользователя приложения
//! и выпускает токен с правом `drive.appdata`) и вводит токен в настройках mdpass.
//! Пароль от Google mdpass не видит и в OAuth не ходит.
//!
//! Drive API не даёт загрузить файл и задать ему родительскую папку одним запросом
//! с телом — новый файл создаётся как пустая запись (`POST /files`), и сразу после
//! этого в него записывается содержимое (`PATCH /upload/files/{id}?uploadType=media`).
//! Поэтому запись нового файла — два запроса; существующего — один.

use mdpass_core::cloud::{CloudError, CloudStore};
use serde::Deserialize;
use serde_json::json;
use ureq::Agent;

use crate::http::{agent, failed, offline};

const FILES: &str = "https://www.googleapis.com/drive/v3/files";
const UPLOAD: &str = "https://www.googleapis.com/upload/drive/v3/files";
/// Хранилище больше этого не скачиваем: это уже точно не оно.
const MAX_FILE: u64 = 64 * 1024 * 1024;

#[derive(Deserialize)]
struct FileList {
    files: Vec<FileMeta>,
}

#[derive(Deserialize, Clone)]
struct FileMeta {
    id: String,
    #[serde(rename = "md5Checksum")]
    md5_checksum: Option<String>,
}

/// Экранирует `'` и `\` в имени файла для встраивания в запрос поиска Drive.
fn escape(name: &str) -> String {
    name.replace('\\', "\\\\").replace('\'', "\\'")
}

/// Имена хранилищ (`*.age`) в служебной папке: что можно скачать на новое устройство.
pub fn list_vaults(access_token: &str) -> Result<Vec<String>, CloudError> {
    #[derive(Deserialize)]
    struct Named {
        name: String,
    }
    #[derive(Deserialize)]
    struct Listed {
        files: Vec<Named>,
    }
    let mut response = agent()
        .get(FILES)
        .query("spaces", "appDataFolder")
        .query("q", "trashed = false")
        .query("pageSize", "1000")
        .query("fields", "files(name)")
        .header("Authorization", format!("Bearer {access_token}"))
        .call()
        .map_err(offline)?;
    let status = response.status().as_u16();
    let body = response.body_mut().read_to_string().map_err(offline)?;
    if status != 200 {
        return Err(failed(status, &body));
    }
    let listed: Listed = serde_json::from_str(&body).map_err(|err| CloudError::Other(err.to_string()))?;
    let mut names: Vec<String> = listed.files.into_iter().map(|f| f.name).filter(|n| n.ends_with(".age")).collect();
    names.sort();
    Ok(names)
}

/// Файл хранилища в служебной папке приложения на Google Drive.
pub struct GoogleDrive {
    agent: Agent,
    token: String,
    name: String,
    /// Номер файла в Drive. Узнаём при первом обращении и запоминаем на время
    /// жизни значения: следующая синхронизация создаёт `GoogleDrive` заново.
    file_id: Option<String>,
}

impl GoogleDrive {
    /// Файл `file_name` в служебной папке приложения.
    pub fn new(access_token: impl Into<String>, file_name: &str) -> Self {
        Self { agent: agent(), token: access_token.into(), name: file_name.into(), file_id: None }
    }

    fn auth(&self) -> String {
        format!("Bearer {}", self.token)
    }

    /// Ищет файл по имени в служебной папке. `None` — файла нет.
    fn find(&self) -> Result<Option<FileMeta>, CloudError> {
        let query = format!("name = '{}' and trashed = false", escape(&self.name));
        let mut response = self
            .agent
            .get(FILES)
            .query("spaces", "appDataFolder")
            .query("q", &query)
            .query("fields", "files(id,md5Checksum)")
            .header("Authorization", self.auth())
            .call()
            .map_err(offline)?;
        let status = response.status().as_u16();
        let body = response.body_mut().read_to_string().map_err(offline)?;
        if status != 200 {
            return Err(failed(status, &body));
        }
        let list: FileList = serde_json::from_str(&body).map_err(|err| CloudError::Other(err.to_string()))?;
        Ok(list.files.into_iter().next())
    }

    /// Создаёт пустую запись файла в служебной папке и возвращает её номер.
    fn create(&self) -> Result<String, CloudError> {
        #[derive(Deserialize)]
        struct Created {
            id: String,
        }
        let body = json!({ "name": self.name, "parents": ["appDataFolder"] });
        let mut response = self
            .agent
            .post(FILES)
            .header("Authorization", self.auth())
            .header("Content-Type", "application/json")
            .send(serde_json::to_vec(&body).map_err(|err| CloudError::Other(err.to_string()))?)
            .map_err(offline)?;
        let status = response.status().as_u16();
        let text = response.body_mut().read_to_string().map_err(offline)?;
        if status != 200 {
            return Err(failed(status, &text));
        }
        let created: Created = serde_json::from_str(&text).map_err(|err| CloudError::Other(err.to_string()))?;
        Ok(created.id)
    }

    /// Записывает содержимое файла с известным номером.
    fn write(&self, id: &str, bytes: &[u8]) -> Result<String, CloudError> {
        let mut response = self
            .agent
            .patch(format!("{UPLOAD}/{id}"))
            .query("uploadType", "media")
            .query("fields", "md5Checksum")
            .header("Authorization", self.auth())
            .header("Content-Type", "application/octet-stream")
            .send(bytes)
            .map_err(offline)?;
        let status = response.status().as_u16();
        let body = response.body_mut().read_to_string().map_err(offline)?;
        if status != 200 {
            return Err(failed(status, &body));
        }
        let meta: FileMeta = serde_json::from_str(&body).map_err(|err| CloudError::Other(err.to_string()))?;
        meta.md5_checksum.ok_or_else(|| CloudError::Other("Google не вернул контрольную сумму файла".into()))
    }
}

impl CloudStore for GoogleDrive {
    fn version(&mut self) -> Result<Option<String>, CloudError> {
        let found = self.find()?;
        self.file_id = found.as_ref().map(|f| f.id.clone());
        Ok(found.and_then(|f| f.md5_checksum))
    }

    fn download(&mut self) -> Result<(Vec<u8>, String), CloudError> {
        let found = self.find()?.ok_or_else(|| CloudError::Other("файла нет".into()))?;
        self.file_id = Some(found.id.clone());
        let mut response = self
            .agent
            .get(format!("{FILES}/{}", found.id))
            .query("alt", "media")
            .header("Authorization", self.auth())
            .call()
            .map_err(offline)?;
        let status = response.status().as_u16();
        if status != 200 {
            let body = response.body_mut().read_to_string().unwrap_or_default();
            return Err(failed(status, &body));
        }
        let bytes = response.body_mut().with_config().limit(MAX_FILE).read_to_vec().map_err(offline)?;
        let version = found.md5_checksum.ok_or_else(|| CloudError::Other("файл без контрольной суммы".into()))?;
        Ok((bytes, version))
    }

    fn upload(&mut self, bytes: &[u8], expected: Option<&str>) -> Result<String, CloudError> {
        // Записи «только если не меняли» нет: сверяем версию прямо перед загрузкой.
        if self.version()?.as_deref() != expected {
            return Err(CloudError::Changed);
        }
        let id = match self.file_id.clone() {
            Some(id) => id,
            None => {
                let id = self.create()?;
                self.file_id = Some(id.clone());
                id
            }
        };
        self.write(&id, bytes)
    }
}
