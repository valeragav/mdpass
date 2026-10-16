//! Версия файла для синхронизации между устройствами. Вторая строка файла:
//!
//! ```text
//! <!-- mdpass:1 -->
//! <!-- mdpass-sync rev=4b9e0c1d2a3f5e6b at=1792138000 by=Ноутбук parents=3c1a…,91f0… -->
//! ```
//!
//! `rev` — случайный номер версии, новый при каждой записи. `parents` — номера
//! прошлых версий, последняя первой. По ним два устройства находят общую версию,
//! от которой разошлись их файлы, и сливают изменения ([`crate::merge`]).
//!
//! `at` — когда записана эта версия (секунды Unix), `by` — имя устройства, которое
//! её записало. Их показывает окно конфликта: «изменено на Ноутбуке в 12:41».
//! Имя пишется только в зашифрованное хранилище: в открытом файле имя компьютера
//! было бы видно всем. В `by` пробелы и прочие знаки кодируются как `%XX`.
//!
//! Старые версии mdpass эту строку сохраняют как текст, но не обновляют. Тогда
//! `rev` остаётся прежним, а содержимое меняется: это видно, если сравнить файл
//! с сохранённой версией с тем же `rev`.

use std::sync::RwLock;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::format::Document;

const PREFIX: &str = "<!-- mdpass-sync";
const END: &str = " -->";
/// Сколько прошлых версий помнит файл.
pub const MAX_PARENTS: usize = 64;
/// Самое длинное имя устройства, в символах.
const MAX_DEVICE: usize = 64;

/// Имя этого устройства для новых версий. Задаёт приложение при запуске.
static DEVICE: RwLock<Option<String>> = RwLock::new(None);

/// Задаёт имя этого устройства. `None` или пустое — без имени.
pub fn set_device_name(name: Option<String>) {
    let name = name.map(|n| n.trim().chars().take(MAX_DEVICE).collect::<String>()).filter(|n| !n.is_empty());
    *DEVICE.write().unwrap_or_else(|poisoned| poisoned.into_inner()) = name;
}

/// Имя этого устройства, если его задали.
pub fn device_name() -> Option<String> {
    DEVICE.read().unwrap_or_else(|poisoned| poisoned.into_inner()).clone()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stamp {
    pub rev: String,
    /// Прошлые версии, последняя первой.
    pub parents: Vec<String>,
    /// Когда записана эта версия, секунды Unix.
    pub at: Option<u64>,
    /// Устройство, которое её записало.
    pub device: Option<String>,
}

impl Stamp {
    /// Эта версия и все прошлые, которые помнит файл, начиная с неё самой.
    pub fn history(&self) -> impl Iterator<Item = &str> {
        std::iter::once(self.rev.as_str()).chain(self.parents.iter().map(String::as_str))
    }
}

/// Версия документа. `None`, если строки нет или она испорчена.
pub fn stamp(doc: &Document) -> Option<Stamp> {
    let line = doc.meta_line(PREFIX)?;
    let body = line.strip_prefix(PREFIX)?.strip_suffix(END)?;
    let mut rev = None;
    let mut parents = Vec::new();
    let mut at = None;
    let mut device = None;
    for part in body.split_whitespace() {
        match part.split_once('=') {
            Some(("rev", value)) if is_rev(value) => rev = Some(value.to_string()),
            Some(("at", value)) => at = value.parse().ok(),
            Some(("by", value)) => device = Some(decode(value)).filter(|d| !d.is_empty()),
            Some(("parents", value)) => {
                parents = value.split(',').filter(|v| is_rev(v)).map(str::to_string).take(MAX_PARENTS).collect()
            }
            _ => {}
        }
    }
    Some(Stamp { rev: rev?, parents, at, device })
}

/// Ставит документу новую версию без имени устройства. Прошлые — его прежняя
/// версия и её история, а после слияния ещё и история второй стороны из `merged`.
pub fn restamp(doc: &mut Document, merged: &[&Stamp]) -> Stamp {
    restamp_as(doc, merged, None)
}

/// Как [`restamp`], но с именем устройства, которое записывает версию.
pub fn restamp_as(doc: &mut Document, merged: &[&Stamp], device: Option<&str>) -> Stamp {
    let mut parents: Vec<String> = Vec::new();
    let own = stamp(doc);
    // Истории идут вперемешку, по одной версии с каждой стороны: так в первых
    // `MAX_PARENTS` остаются недавние версии обеих.
    let mut histories: Vec<Vec<&str>> =
        own.iter().chain(merged.iter().copied()).map(|s| s.history().collect()).collect();
    let longest = histories.iter().map(Vec::len).max().unwrap_or(0);
    for depth in 0..longest {
        for history in &mut histories {
            if let Some(rev) = history.get(depth)
                && !parents.iter().any(|p| p == rev)
            {
                parents.push(rev.to_string());
            }
        }
    }
    parents.truncate(MAX_PARENTS);
    let at = SystemTime::now().duration_since(UNIX_EPOCH).ok().map(|d| d.as_secs());
    let device = device.map(|d| d.chars().take(MAX_DEVICE).collect::<String>()).filter(|d| !d.trim().is_empty());
    let next = Stamp { rev: new_rev(), parents, at, device };
    doc.set_meta_line(PREFIX, &render(&next));
    next
}

fn render(stamp: &Stamp) -> String {
    let mut line = format!("{PREFIX} rev={}", stamp.rev);
    if let Some(at) = stamp.at {
        line.push_str(&format!(" at={at}"));
    }
    if let Some(device) = &stamp.device {
        line.push_str(&format!(" by={}", encode(device)));
    }
    if !stamp.parents.is_empty() {
        line.push_str(&format!(" parents={}", stamp.parents.join(",")));
    }
    line.push_str(END);
    line
}

/// Имя устройства для строки: всё, кроме букв, цифр, `.` и `_`, — как `%XX`.
/// Так в нём нет пробелов, `-` и `>`, которые сломали бы строку.
fn encode(name: &str) -> String {
    let mut out = String::new();
    for c in name.chars() {
        if c.is_alphanumeric() || matches!(c, '.' | '_') {
            out.push(c);
        } else {
            let mut buf = [0u8; 4];
            for b in c.encode_utf8(&mut buf).bytes() {
                out.push_str(&format!("%{b:02X}"));
            }
        }
    }
    out
}

fn decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let hex = |b: u8| (b as char).to_digit(16);
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && let (Some(h), Some(l)) = (hex(bytes[i + 1]), hex(bytes[i + 2]))
        {
            out.push((h * 16 + l) as u8);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).chars().take(MAX_DEVICE).collect()
}

fn is_rev(value: &str) -> bool {
    (1..=32).contains(&value.len()) && value.bytes().all(|b| b.is_ascii_hexdigit())
}

fn new_rev() -> String {
    let mut bytes = [0u8; 8];
    getrandom::fill(&mut bytes).expect("системный генератор случайных чисел");
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
