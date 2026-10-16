//! Версия файла для синхронизации между устройствами. Вторая строка файла:
//!
//! ```text
//! <!-- mdpass:1 -->
//! <!-- mdpass-sync rev=4b9e0c1d2a3f5e6b parents=3c1a…,91f0… -->
//! ```
//!
//! `rev` — случайный номер версии, новый при каждой записи. `parents` — номера
//! прошлых версий, последняя первой. По ним два устройства находят общую версию,
//! от которой разошлись их файлы, и сливают изменения ([`crate::merge`]).
//!
//! Старые версии mdpass эту строку сохраняют как текст, но не обновляют. Тогда
//! `rev` остаётся прежним, а содержимое меняется: это видно, если сравнить файл
//! с сохранённой версией с тем же `rev`.

use crate::format::Document;

const PREFIX: &str = "<!-- mdpass-sync";
const END: &str = " -->";
/// Сколько прошлых версий помнит файл.
pub const MAX_PARENTS: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stamp {
    pub rev: String,
    /// Прошлые версии, последняя первой.
    pub parents: Vec<String>,
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
    for part in body.split_whitespace() {
        match part.split_once('=') {
            Some(("rev", value)) if is_rev(value) => rev = Some(value.to_string()),
            Some(("parents", value)) => {
                parents = value.split(',').filter(|v| is_rev(v)).map(str::to_string).take(MAX_PARENTS).collect()
            }
            _ => {}
        }
    }
    Some(Stamp { rev: rev?, parents })
}

/// Ставит документу новую версию. Прошлые — его прежняя версия и её история,
/// а после слияния ещё и история второй стороны из `merged`.
pub fn restamp(doc: &mut Document, merged: &[&Stamp]) -> Stamp {
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
    let next = Stamp { rev: new_rev(), parents };
    doc.set_meta_line(PREFIX, &render(&next));
    next
}

fn render(stamp: &Stamp) -> String {
    if stamp.parents.is_empty() {
        format!("{PREFIX} rev={}{END}", stamp.rev)
    } else {
        format!("{PREFIX} rev={} parents={}{END}", stamp.rev, stamp.parents.join(","))
    }
}

fn is_rev(value: &str) -> bool {
    (1..=32).contains(&value.len()) && value.bytes().all(|b| b.is_ascii_hexdigit())
}

fn new_rev() -> String {
    let mut bytes = [0u8; 8];
    getrandom::fill(&mut bytes).expect("системный генератор случайных чисел");
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
