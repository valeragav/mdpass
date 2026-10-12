//! Формат хранилища: обычный Markdown.
//!
//! ```text
//! # work                 ← раздел
//! ## server              ← вложенный раздел
//! ### vps1               ← запись: заголовок без вложенных заголовков
//! ssh: user@host         ← поле «ключ: значение»
//! password: ••••         ← секретное по имени ключа
//! pin_hint!: ••••        ← секретное по пометке «!»
//! note: первая строка
//!   вторая строка        ← продолжение значения, отступ в два пробела
//! tags: [work], [prod]   ← теги
//! ```
//!
//! Документ хранит исходный текст каждого куска файла. Записи, которые не менялись,
//! выводятся как были, поэтому файл без правок после разбора и сборки совпадает
//! с исходным побайтно, а ручные правки пользователя не теряются.

use std::fmt;

use zeroize::Zeroize;

/// Самый глубокий заголовок Markdown.
const MAX_LEVEL: usize = 6;
/// Новые записи пишутся заголовком не мельче `###`, чтобы в файле их было легко отличить
/// от разделов.
const MIN_ENTRY_LEVEL: usize = 3;
const TAGS_KEY: &str = "tags";
const SECRET_MARKER: char = '!';
/// Ключ секретный, если содержит одно из этих слов…
const SECRET_WORDS: [&str; 4] = ["password", "passwd", "token", "secret"];
/// …или совпадает с одним из этих имён.
const SECRET_NAMES: [&str; 8] = ["pass", "pwd", "pin", "otp", "totp", "api_key", "apikey", "private_key"];

/// Секретен ли ключ по имени. У таких полей признак секрета снять нельзя:
/// при чтении файла он всё равно восстановится по имени.
pub fn is_secret_name(key: &str) -> bool {
    let key = key.to_lowercase();
    SECRET_WORDS.iter().any(|word| key.contains(word)) || SECRET_NAMES.contains(&key.as_str())
}

/// Номер записи. Постоянен, пока документ открыт, в файл не пишется.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EntryId(u64);

impl EntryId {
    pub fn get(self) -> u64 {
        self.0
    }

    pub fn from_raw(raw: u64) -> Self {
        Self(raw)
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct Field {
    pub key: String,
    /// Строки многострочного значения разделены `\n`.
    pub value: String,
    pub secret: bool,
}

impl Field {
    /// Поле, секретное по имени ключа.
    pub fn new(key: impl Into<String>, value: impl Into<String>) -> Self {
        let key = key.into();
        let secret = is_secret_name(&key);
        Self { key, value: value.into(), secret }
    }
}

impl Drop for Field {
    fn drop(&mut self) {
        self.value.zeroize();
    }
}

/// Значение секретного поля в отладочный вывод не попадает.
impl fmt::Debug for Field {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value: &dyn fmt::Debug = if self.secret { &"<скрыто>" } else { &self.value };
        f.debug_struct("Field").field("key", &self.key).field("value", value).field("secret", &self.secret).finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub id: EntryId,
    /// Названия разделов от верхнего к нижнему.
    pub path: Vec<String>,
    pub title: String,
    pub fields: Vec<Field>,
    pub tags: Vec<String>,
    /// Строки записи, которые не похожи на поле «ключ: значение».
    pub extra: Vec<String>,
}

/// Данные записи для добавления или изменения.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntryDraft {
    pub path: Vec<String>,
    pub title: String,
    pub fields: Vec<Field>,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FormatError {
    EmptyTitle,
    LineBreakInName,
    EmptySection,
    TooDeep,
    InvalidKey(String),
    InvalidTag(String),
    NotFound,
}

impl fmt::Display for FormatError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyTitle => write!(f, "У записи нет названия"),
            Self::LineBreakInName => write!(f, "Название раздела или записи должно быть в одну строку"),
            Self::EmptySection => write!(f, "Пустое название раздела"),
            Self::TooDeep => write!(f, "Слишком много вложенных разделов: можно не больше {}", MAX_LEVEL - 1),
            Self::InvalidKey(key) => write!(
                f,
                "Недопустимый ключ поля «{key}»: нужен непустой ключ без пробелов, «:» и «!» на конце, кроме «{TAGS_KEY}»"
            ),
            Self::InvalidTag(tag) => write!(f, "Недопустимый тег «{tag}»: в теге не должно быть «,», «[» и «]»"),
            Self::NotFound => write!(f, "Запись не найдена"),
        }
    }
}

impl std::error::Error for FormatError {}

#[derive(Clone)]
pub struct Document {
    bom: bool,
    eol: &'static str,
    nodes: Vec<Node>,
    next_id: u64,
}

/// В исходном тексте есть секреты, поэтому в отладочный вывод он не попадает.
impl fmt::Debug for Document {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let entries = self.nodes.iter().filter(|node| matches!(node, Node::Entry(_))).count();
        f.debug_struct("Document").field("entries", &entries).finish_non_exhaustive()
    }
}

/// Стирает текст файла из памяти. Копии, которые остались после перевыделения
/// строк, так не стираются: это защита от случайной утечки, а не гарантия.
impl Drop for Document {
    fn drop(&mut self) {
        for node in &mut self.nodes {
            match node {
                Node::Text(text) => text.zeroize(),
                Node::Section(section) => section.raw.zeroize(),
                Node::Entry(entry) => {
                    entry.extra.zeroize();
                    if let Some(raw) = &mut entry.raw {
                        raw.zeroize();
                    }
                }
            }
        }
    }
}

#[derive(Clone)]
enum Node {
    /// Текст вне записей: начало файла или текст раздела.
    Text(String),
    Section(Section),
    Entry(EntryNode),
}

#[derive(Clone)]
struct Section {
    level: usize,
    title: String,
    /// Строка заголовка и пустые строки после неё.
    raw: String,
}

#[derive(Clone)]
struct EntryNode {
    id: EntryId,
    level: usize,
    title: String,
    fields: Vec<Field>,
    tags: Vec<String>,
    /// Строки записи, которые не похожи на поле. Сохраняются как есть.
    extra: Vec<String>,
    trailing_blank: usize,
    /// Исходный текст. `None`, если запись менялась и выводится заново.
    raw: Option<String>,
}

impl Default for Document {
    fn default() -> Self {
        Self::parse("")
    }
}

impl Document {
    pub fn parse(input: &str) -> Self {
        let (bom, text) = match input.strip_prefix('\u{feff}') {
            Some(rest) => (true, rest),
            None => (false, input),
        };
        let eol = if text.contains("\r\n") { "\r\n" } else { "\n" };
        let mut doc = Self { bom, eol, nodes: Vec::new(), next_id: 0 };

        let lines: Vec<&str> = text.split_inclusive('\n').collect();
        let headings: Vec<(usize, usize, String)> = lines
            .iter()
            .enumerate()
            .filter_map(|(i, line)| parse_heading(content(line)).map(|(level, title)| (i, level, title)))
            .collect();

        let first = headings.first().map_or(lines.len(), |h| h.0);
        if first > 0 {
            doc.nodes.push(Node::Text(lines[..first].concat()));
        }

        for (n, (start, level, title)) in headings.iter().enumerate() {
            let next = headings.get(n + 1);
            let end = next.map_or(lines.len(), |h| h.0);
            let body = &lines[start + 1..end];
            let is_entry = next.is_none_or(|h| h.1 <= *level);
            if is_entry {
                let id = doc.new_id();
                let mut entry = parse_entry(id, *level, title.clone(), body);
                entry.raw = Some(lines[*start..end].concat());
                doc.nodes.push(Node::Entry(entry));
            } else {
                let blanks = body.iter().take_while(|line| is_blank(line)).count();
                doc.nodes.push(Node::Section(Section {
                    level: *level,
                    title: title.clone(),
                    raw: lines[*start..start + 1 + blanks].concat(),
                }));
                if blanks < body.len() {
                    doc.nodes.push(Node::Text(body[blanks..].concat()));
                }
            }
        }
        doc
    }

    pub fn to_markdown(&self) -> String {
        let mut out = String::new();
        if self.bom {
            out.push('\u{feff}');
        }
        self.render_nodes(&self.nodes, &mut out);
        out
    }

    pub fn entries(&self) -> Vec<Entry> {
        let paths = self.node_paths();
        self.nodes
            .iter()
            .zip(paths)
            .filter_map(|(node, path)| match node {
                Node::Entry(e) => Some(Entry {
                    id: e.id,
                    path,
                    title: e.title.clone(),
                    fields: e.fields.clone(),
                    tags: e.tags.clone(),
                    extra: e.extra.clone(),
                }),
                _ => None,
            })
            .collect()
    }

    pub fn entry(&self, id: EntryId) -> Option<Entry> {
        self.entries().into_iter().find(|e| e.id == id)
    }

    pub fn add(&mut self, draft: EntryDraft) -> Result<EntryId, FormatError> {
        let draft = normalize(draft)?;
        let mut work = self.clone();
        let id = work.new_id();
        work.insert_entry(id, draft, Vec::new())?;
        *self = work;
        Ok(id)
    }

    pub fn update(&mut self, id: EntryId, draft: EntryDraft) -> Result<(), FormatError> {
        let draft = normalize(draft)?;
        let index = self.entry_index(id).ok_or(FormatError::NotFound)?;
        let mut work = self.clone();
        let current_path = work.node_paths().swap_remove(index);
        let Node::Entry(entry) = &mut work.nodes[index] else { unreachable!() };

        if current_path == draft.path {
            let unchanged = entry.title == draft.title && entry.fields == draft.fields && entry.tags == draft.tags;
            if !unchanged {
                entry.title = draft.title;
                entry.fields = draft.fields;
                entry.tags = draft.tags;
                entry.raw = None;
            }
        } else {
            let extra = std::mem::take(&mut entry.extra);
            work.nodes.remove(index);
            work.remove_empty_sections();
            work.insert_entry(id, draft, extra)?;
        }
        *self = work;
        Ok(())
    }

    /// Исходный текст записи из файла. `None`, если запись уже выводится заново.
    pub fn entry_source(&self, id: EntryId) -> Option<&str> {
        match &self.nodes[self.entry_index(id)?] {
            Node::Entry(entry) => entry.raw.as_deref(),
            _ => None,
        }
    }

    /// Выводит запись заново в обычном виде, даже если данные не менялись:
    /// например, чтобы добавить пробел в слитное «ключ:значение».
    pub fn rewrite(&mut self, id: EntryId) -> Result<(), FormatError> {
        let index = self.entry_index(id).ok_or(FormatError::NotFound)?;
        if let Node::Entry(entry) = &mut self.nodes[index] {
            entry.raw = None;
        }
        Ok(())
    }

    /// Убирает из записи строки без ключа: их перенесли в поля.
    pub fn clear_extra(&mut self, id: EntryId) -> Result<(), FormatError> {
        let index = self.entry_index(id).ok_or(FormatError::NotFound)?;
        if let Node::Entry(entry) = &mut self.nodes[index] {
            entry.extra.zeroize();
            entry.extra.clear();
            entry.raw = None;
        }
        Ok(())
    }

    pub fn remove(&mut self, id: EntryId) -> Result<(), FormatError> {
        let index = self.entry_index(id).ok_or(FormatError::NotFound)?;
        self.nodes.remove(index);
        self.remove_empty_sections();
        Ok(())
    }

    fn new_id(&mut self) -> EntryId {
        self.next_id += 1;
        EntryId(self.next_id)
    }

    fn entry_index(&self, id: EntryId) -> Option<usize> {
        self.nodes.iter().position(|node| matches!(node, Node::Entry(e) if e.id == id))
    }

    fn render_nodes(&self, nodes: &[Node], out: &mut String) {
        for node in nodes {
            match node {
                Node::Text(text) => out.push_str(text),
                Node::Section(section) => out.push_str(&section.raw),
                Node::Entry(entry) => match &entry.raw {
                    Some(raw) => out.push_str(raw),
                    None => render_entry(entry, self.eol, out),
                },
            }
        }
    }

    /// Путь каждого узла: для раздела — вместе с ним самим, для записи и текста —
    /// разделы, в которых они лежат.
    fn node_paths(&self) -> Vec<Vec<String>> {
        let mut stack: Vec<(usize, String)> = Vec::new();
        let titles = |stack: &[(usize, String)]| stack.iter().map(|(_, t)| t.clone()).collect::<Vec<_>>();
        self.nodes
            .iter()
            .map(|node| match node {
                Node::Text(_) => titles(&stack),
                Node::Section(s) => {
                    stack.retain(|(level, _)| *level < s.level);
                    stack.push((s.level, s.title.clone()));
                    titles(&stack)
                }
                Node::Entry(e) => {
                    stack.retain(|(level, _)| *level < e.level);
                    titles(&stack)
                }
            })
            .collect()
    }

    fn heading_level(&self, index: usize) -> Option<usize> {
        match &self.nodes[index] {
            Node::Section(s) => Some(s.level),
            Node::Entry(e) => Some(e.level),
            Node::Text(_) => None,
        }
    }

    /// Конец раздела: первый заголовок того же уровня или выше.
    fn subtree_end(&self, index: usize) -> usize {
        let level = self.heading_level(index).expect("раздел");
        (index + 1..self.nodes.len())
            .find(|&i| self.heading_level(i).is_some_and(|l| l <= level))
            .unwrap_or(self.nodes.len())
    }

    fn remove_empty_sections(&mut self) {
        loop {
            let empty = (0..self.nodes.len()).find(|&i| {
                matches!(self.nodes[i], Node::Section(_))
                    && !self.nodes[i + 1..self.subtree_end(i)].iter().any(|node| match node {
                        Node::Entry(_) => true,
                        Node::Text(text) => !text.trim().is_empty(),
                        Node::Section(_) => false,
                    })
            });
            match empty {
                Some(i) => {
                    let end = self.subtree_end(i);
                    self.nodes.drain(i..end);
                }
                None => break,
            }
        }
    }

    fn insert_entry(&mut self, id: EntryId, draft: EntryDraft, extra: Vec<String>) -> Result<(), FormatError> {
        let mut parent = None;
        for depth in 0..draft.path.len() {
            let prefix = &draft.path[..=depth];
            let paths = self.node_paths();
            let found =
                (0..self.nodes.len()).find(|&i| matches!(self.nodes[i], Node::Section(_)) && paths[i] == prefix);
            parent = Some(match found {
                Some(i) => i,
                None => self.create_section(parent, &draft.path[depth])?,
            });
        }

        let (pos, level) = self.entry_slot(parent);
        if level > MAX_LEVEL {
            return Err(FormatError::TooDeep);
        }
        let entry = EntryNode {
            id,
            level,
            title: draft.title,
            fields: draft.fields,
            tags: draft.tags,
            extra,
            trailing_blank: 0,
            raw: None,
        };
        let index = self.insert_spaced(pos, Node::Entry(entry));
        let has_next = index + 1 < self.nodes.len();
        if let Node::Entry(entry) = &mut self.nodes[index] {
            entry.trailing_blank = usize::from(has_next);
        }
        Ok(())
    }

    /// Добавляет раздел в конец родителя и возвращает его позицию.
    fn create_section(&mut self, parent: Option<usize>, title: &str) -> Result<usize, FormatError> {
        let level = parent.and_then(|p| self.heading_level(p)).map_or(1, |l| l + 1);
        if level > MAX_LEVEL {
            return Err(FormatError::TooDeep);
        }
        let pos = parent.map_or(self.nodes.len(), |p| self.subtree_end(p));
        let raw = format!("{} {title}{eol}{eol}", "#".repeat(level), eol = self.eol);
        Ok(self.insert_spaced(pos, Node::Section(Section { level, title: title.to_string(), raw })))
    }

    /// Куда встаёт новая запись раздела и каким заголовком: после последней записи
    /// раздела её же уровнем, а если записей нет — перед первым вложенным заголовком.
    fn entry_slot(&self, parent: Option<usize>) -> (usize, usize) {
        let (start, end, parent_level) = match parent {
            Some(p) => (p + 1, self.subtree_end(p), self.heading_level(p).unwrap_or(0)),
            None => (0, self.nodes.len(), 0),
        };
        let paths = self.node_paths();
        let parent_path = parent.map_or_else(Vec::new, |p| paths[p].clone());
        let last_own = (start..end).rev().find(|&i| matches!(self.nodes[i], Node::Entry(_)) && paths[i] == parent_path);
        match last_own {
            Some(i) => (i + 1, self.heading_level(i).unwrap_or(MIN_ENTRY_LEVEL)),
            None => {
                let pos = (start..end).find(|&i| self.heading_level(i).is_some()).unwrap_or(end);
                (pos, (parent_level + 1).max(MIN_ENTRY_LEVEL))
            }
        }
    }

    /// Вставляет узел так, чтобы перед ним был перевод строки и пустая строка.
    fn insert_spaced(&mut self, mut pos: usize, node: Node) -> usize {
        let mut before = String::new();
        self.render_nodes(&self.nodes[..pos], &mut before);
        if !before.is_empty() {
            if !before.ends_with('\n') {
                self.nodes.insert(pos, Node::Text(self.eol.to_string()));
                pos += 1;
                before.push_str(self.eol);
            }
            if !before.ends_with("\n\n") && !before.ends_with("\n\r\n") {
                self.nodes.insert(pos, Node::Text(self.eol.to_string()));
                pos += 1;
            }
        }
        self.nodes.insert(pos, node);
        pos
    }
}

/// Строка без перевода строки на конце.
fn content(line: &str) -> &str {
    let line = line.strip_suffix('\n').unwrap_or(line);
    line.strip_suffix('\r').unwrap_or(line)
}

fn is_blank(line: &str) -> bool {
    line.trim().is_empty()
}

fn parse_heading(line: &str) -> Option<(usize, String)> {
    let level = line.bytes().take_while(|&b| b == b'#').count();
    if level == 0 || level > MAX_LEVEL {
        return None;
    }
    let rest = &line[level..];
    if !rest.is_empty() && !rest.starts_with([' ', '\t']) {
        return None;
    }
    let mut title = rest.trim();
    // Закрывающие решётки «## раздел ##» в название не входят.
    let without_closing = title.trim_end_matches('#');
    if without_closing.is_empty() {
        title = "";
    } else if without_closing.len() < title.len() && without_closing.ends_with([' ', '\t']) {
        title = without_closing.trim_end();
    }
    Some((level, title.to_string()))
}

enum FieldLine {
    Field(Field),
    Tags(Vec<String>),
}

fn parse_field(line: &str) -> Option<FieldLine> {
    let colon = line.find(':')?;
    let key = &line[..colon];
    if key.is_empty() || key.contains(char::is_whitespace) {
        return None;
    }
    let value = line[colon + 1..].trim();
    if key.eq_ignore_ascii_case(TAGS_KEY) {
        return Some(FieldLine::Tags(parse_tags(value)));
    }
    let (key, marked) = match key.strip_suffix(SECRET_MARKER) {
        Some(key) => (key, true),
        None => (key, false),
    };
    if key.is_empty() {
        return None;
    }
    Some(FieldLine::Field(Field {
        key: key.to_string(),
        value: value.to_string(),
        secret: marked || is_secret_name(key),
    }))
}

fn parse_tags(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(|tag| tag.trim().trim_start_matches('[').trim_end_matches(']').trim())
        .filter(|tag| !tag.is_empty())
        .map(str::to_string)
        .collect()
}

fn parse_entry(id: EntryId, level: usize, title: String, body: &[&str]) -> EntryNode {
    let mut fields: Vec<Field> = Vec::new();
    let mut tags = Vec::new();
    let mut extra = Vec::new();
    let mut continues = false;
    for line in body {
        let line = content(line);
        if is_blank(line) {
            continues = false;
            continue;
        }
        if line.starts_with([' ', '\t']) {
            if continues && let Some(last) = fields.last_mut() {
                last.value.push('\n');
                last.value.push_str(line.trim());
            } else {
                extra.push(line.to_string());
            }
            continue;
        }
        continues = false;
        match parse_field(line) {
            Some(FieldLine::Field(field)) => {
                fields.push(field);
                continues = true;
            }
            Some(FieldLine::Tags(found)) => tags.extend(found),
            None => extra.push(line.to_string()),
        }
    }
    let trailing_blank = body.iter().rev().take_while(|line| is_blank(line)).count();
    EntryNode { id, level, title, fields, tags, extra, trailing_blank, raw: None }
}

fn render_entry(entry: &EntryNode, eol: &str, out: &mut String) {
    out.push_str(&"#".repeat(entry.level));
    out.push(' ');
    out.push_str(&entry.title);
    out.push_str(eol);

    // Теги пишутся перед заметкой, если она есть, иначе в конце полей.
    let tags_at = entry.fields.iter().position(|f| f.key.eq_ignore_ascii_case("note")).unwrap_or(entry.fields.len());
    for (i, field) in entry.fields.iter().enumerate() {
        if i == tags_at {
            render_tags(&entry.tags, eol, out);
        }
        render_field(field, eol, out);
    }
    if tags_at == entry.fields.len() {
        render_tags(&entry.tags, eol, out);
    }

    for line in &entry.extra {
        out.push_str(line);
        out.push_str(eol);
    }
    for _ in 0..entry.trailing_blank {
        out.push_str(eol);
    }
}

fn render_field(field: &Field, eol: &str, out: &mut String) {
    out.push_str(&field.key);
    if field.secret && !is_secret_name(&field.key) {
        out.push(SECRET_MARKER);
    }
    out.push(':');
    let mut lines = field.value.split('\n');
    if let Some(first) = lines.next().filter(|first| !first.is_empty()) {
        out.push(' ');
        out.push_str(first);
    }
    out.push_str(eol);
    for line in lines {
        out.push_str("  ");
        out.push_str(line);
        out.push_str(eol);
    }
}

fn render_tags(tags: &[String], eol: &str, out: &mut String) {
    if tags.is_empty() {
        return;
    }
    let list: Vec<String> = tags.iter().map(|tag| format!("[{tag}]")).collect();
    out.push_str(TAGS_KEY);
    out.push_str(": ");
    out.push_str(&list.join(", "));
    out.push_str(eol);
}

/// Проверяет черновик и приводит его к виду, в котором он запишется в файл.
fn normalize(draft: EntryDraft) -> Result<EntryDraft, FormatError> {
    let title = draft.title.trim().to_string();
    if title.is_empty() {
        return Err(FormatError::EmptyTitle);
    }
    if title.contains(['\n', '\r']) {
        return Err(FormatError::LineBreakInName);
    }

    let mut path = Vec::with_capacity(draft.path.len());
    for part in draft.path {
        let part = part.trim();
        if part.is_empty() {
            return Err(FormatError::EmptySection);
        }
        if part.contains(['\n', '\r']) {
            return Err(FormatError::LineBreakInName);
        }
        path.push(part.to_string());
    }
    if path.len() >= MAX_LEVEL {
        return Err(FormatError::TooDeep);
    }

    let mut fields = Vec::with_capacity(draft.fields.len());
    for field in draft.fields {
        let key = field.key.trim();
        let valid = !key.is_empty()
            && !key.contains(|c: char| c.is_whitespace() || c == ':')
            && !key.ends_with(SECRET_MARKER)
            && !key.starts_with('#')
            && !key.eq_ignore_ascii_case(TAGS_KEY);
        if !valid {
            return Err(FormatError::InvalidKey(key.to_string()));
        }
        // Пустые строки внутри значения не сохраняются: в файле они закончили бы поле.
        let value =
            field.value.split('\n').map(str::trim).filter(|line| !line.is_empty()).collect::<Vec<_>>().join("\n");
        fields.push(Field { key: key.to_string(), value, secret: field.secret || is_secret_name(key) });
    }

    let mut tags = Vec::with_capacity(draft.tags.len());
    for tag in draft.tags {
        let tag = tag.trim();
        if tag.is_empty() {
            continue;
        }
        if tag.contains([',', '[', ']', '\n', '\r']) {
            return Err(FormatError::InvalidTag(tag.to_string()));
        }
        tags.push(tag.to_string());
    }

    Ok(EntryDraft { path, title, fields, tags })
}
