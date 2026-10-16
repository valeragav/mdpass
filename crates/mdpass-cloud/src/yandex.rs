//! Яндекс Диск. Доступ только к папке приложения (`Приложения/mdpass` на Диске
//! пользователя): остальной Диск mdpass не видит.
//!
//! Вход свой: пользователь получает OAuth-токен на странице Яндекса сам (там же
//! можно создать своё приложение с правом `cloud_api:disk.app_folder`) и вводит
//! токен в настройках mdpass. Пароль от Яндекса mdpass не видит и в OAuth не ходит.
//!
//! Записи «только если файл не меняли» у Яндекс Диска нет: перед загрузкой версия
//! сверяется отдельным запросом, а запись другого устройства, которая всё же
//! проскочила между ними, заметит следующая синхронизация.

use mdpass_core::cloud::{CloudError, CloudStore};
use serde::Deserialize;
use ureq::Agent;

use crate::http::{agent, failed, md5_hex, offline};

const RESOURCES: &str = "https://cloud-api.yandex.net/v1/disk/resources";
/// Хранилище больше этого не скачиваем: это уже точно не оно.
const MAX_FILE: u64 = 64 * 1024 * 1024;

#[derive(Deserialize)]
struct Folder {
    #[serde(rename = "_embedded")]
    embedded: Option<Items>,
}

#[derive(Deserialize)]
struct Items {
    items: Vec<Item>,
}

#[derive(Deserialize)]
struct Item {
    name: String,
    #[serde(rename = "type")]
    kind: String,
}

/// Имена хранилищ (`*.age`) в папке приложения: что можно скачать на новое устройство.
pub fn list_vaults(access_token: &str) -> Result<Vec<String>, CloudError> {
    let mut response = agent()
        .get(RESOURCES)
        .query("path", "app:/")
        .query("limit", "200")
        .query("fields", "_embedded.items.name,_embedded.items.type")
        .header("Authorization", format!("OAuth {access_token}"))
        .call()
        .map_err(offline)?;
    let status = response.status().as_u16();
    let body = response.body_mut().read_to_string().map_err(offline)?;
    match status {
        200 => {
            let folder: Folder = serde_json::from_str(&body).map_err(|err| CloudError::Other(err.to_string()))?;
            let mut names: Vec<String> = folder
                .embedded
                .map(|e| e.items)
                .unwrap_or_default()
                .into_iter()
                .filter(|item| item.kind == "file" && item.name.ends_with(".age"))
                .map(|item| item.name)
                .collect();
            names.sort();
            Ok(names)
        }
        // Папки приложения ещё нет: в неё ничего не загружали.
        404 => Ok(Vec::new()),
        _ => Err(failed(status, &body)),
    }
}

/// Файл хранилища в папке приложения на Яндекс Диске.
pub struct YandexDisk {
    agent: Agent,
    token: String,
    /// Путь в папке приложения: `app:/vault.md.age`.
    path: String,
}

#[derive(Deserialize)]
struct Resource {
    md5: Option<String>,
}

#[derive(Deserialize)]
struct Link {
    href: String,
}

impl YandexDisk {
    /// Файл `file_name` в папке приложения.
    pub fn new(access_token: impl Into<String>, file_name: &str) -> Self {
        Self { agent: agent(), token: access_token.into(), path: format!("app:/{file_name}") }
    }

    fn auth(&self) -> String {
        format!("OAuth {}", self.token)
    }

    /// Ссылка для скачивания или загрузки: `kind` — `download` или `upload`.
    fn link(&self, kind: &str) -> Result<String, CloudError> {
        let mut request = self
            .agent
            .get(format!("{RESOURCES}/{kind}"))
            .query("path", &self.path)
            .header("Authorization", self.auth());
        if kind == "upload" {
            request = request.query("overwrite", "true");
        }
        let mut response = request.call().map_err(offline)?;
        let status = response.status().as_u16();
        let body = response.body_mut().read_to_string().map_err(offline)?;
        if status != 200 {
            return Err(failed(status, &body));
        }
        let link: Link = serde_json::from_str(&body).map_err(|err| CloudError::Other(err.to_string()))?;
        Ok(link.href)
    }
}

impl CloudStore for YandexDisk {
    fn version(&mut self) -> Result<Option<String>, CloudError> {
        let mut response = self
            .agent
            .get(RESOURCES)
            .query("path", &self.path)
            .query("fields", "md5,type")
            .header("Authorization", self.auth())
            .call()
            .map_err(offline)?;
        let status = response.status().as_u16();
        let body = response.body_mut().read_to_string().map_err(offline)?;
        match status {
            200 => {
                let resource: Resource =
                    serde_json::from_str(&body).map_err(|err| CloudError::Other(err.to_string()))?;
                // Без хеша это не файл, а, например, папка с тем же именем.
                resource.md5.map(Some).ok_or_else(|| CloudError::Other("в облаке на месте файла не файл".into()))
            }
            404 => Ok(None),
            _ => Err(failed(status, &body)),
        }
    }

    fn download(&mut self) -> Result<(Vec<u8>, String), CloudError> {
        let href = self.link("download")?;
        let mut response = self.agent.get(&href).call().map_err(offline)?;
        let status = response.status().as_u16();
        if status != 200 {
            let body = response.body_mut().read_to_string().unwrap_or_default();
            return Err(failed(status, &body));
        }
        let bytes = response.body_mut().with_config().limit(MAX_FILE).read_to_vec().map_err(offline)?;
        let version = md5_hex(&bytes);
        Ok((bytes, version))
    }

    fn upload(&mut self, bytes: &[u8], expected: Option<&str>) -> Result<String, CloudError> {
        // Записи «только если не меняли» нет: сверяем версию прямо перед загрузкой.
        if self.version()?.as_deref() != expected {
            return Err(CloudError::Changed);
        }
        let href = self.link("upload")?;
        let mut response = self.agent.put(&href).send(bytes).map_err(offline)?;
        let status = response.status().as_u16();
        if status != 201 && status != 202 {
            let body = response.body_mut().read_to_string().unwrap_or_default();
            return Err(failed(status, &body));
        }
        Ok(md5_hex(bytes))
    }
}
