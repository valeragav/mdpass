//! Формат хранилища: обычный Markdown.
//!
//! ```text
//! # work                 ← раздел, до шести уровней: # … ######
//! ## server              ← вложенный раздел
//! **vps1**               ← запись: строка с названием жирным
//! ssh: user@host         ← поле «ключ: значение»
//! password: ••••         ← секретное по имени ключа
//! pin_hint!: ••••        ← секретное по пометке «!»
//! note: первая строка
//!   вторая строка        ← продолжение значения, отступ в два пробела
//! tags: [work], [prod]   ← теги
//! ```
//!
//! Первая строка файла — метка формата `<!-- mdpass:1 -->`. В редакторах Markdown
//! она не видна. По ней будущие версии узнают, как читать файл.
//!
//! Заголовки — только разделы. Запись принадлежит последнему открытому разделу.
//! Записи до первого заголовка лежат на верхнем уровне.
//!
//! Документ хранит исходный текст каждого куска файла. Записи, которые не менялись,
//! выводятся как были, поэтому файл без правок после разбора и сборки совпадает
//! с исходным побайтно, а ручные правки пользователя не теряются.

use std::fmt;

use zeroize::Zeroize;

/// Версия формата, которую пишет и понимает эта версия приложения.
pub const FORMAT_VERSION: u32 = 1;
const HEADER_START: &str = "<!-- mdpass:";
const HEADER_END: &str = " -->";

/// Самый глубокий заголовок Markdown: столько уровней разделов бывает.
const MAX_LEVEL: usize = 6;
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
    /// Название записи начинается или кончается звёздочкой: в файле его не отличить от жирного.
    InvalidTitle,
    LineBreakInName,
    EmptySection,
    TooDeep,
    InvalidKey(String),
    InvalidTag(String),
    NotFound,
    /// Раздел нельзя перенести внутрь самого себя.
    IntoItself,
    /// В целевом разделе уже есть раздел с таким названием.
    SectionExists(String),
}

impl fmt::Display for FormatError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyTitle => write!(f, "У записи нет названия"),
            Self::InvalidTitle => write!(f, "Название записи не может начинаться или кончаться звёздочкой"),
            Self::LineBreakInName => write!(f, "Название раздела или записи должно быть в одну строку"),
            Self::EmptySection => write!(f, "Пустое название раздела"),
            Self::TooDeep => write!(f, "Слишком много вложенных разделов: можно не больше {MAX_LEVEL}"),
            Self::InvalidKey(key) => write!(
                f,
                "Недопустимый ключ поля «{key}»: нужен непустой ключ без пробелов, «:» и «!» на конце, кроме «{TAGS_KEY}»"
            ),
            Self::InvalidTag(tag) => write!(f, "Недопустимый тег «{tag}»: в теге не должно быть «,», «[» и «]»"),
            Self::NotFound => write!(f, "Запись не найдена"),
            Self::IntoItself => write!(f, "Раздел нельзя перенести внутрь самого себя"),
            Self::SectionExists(title) => write!(f, "Здесь уже есть раздел «{title}»"),
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

/// Чем начинается кусок файла.
enum LineKind {
    Heading(usize, String),
    Title(String),
    Other,
}

fn line_kind(line: &str) -> LineKind {
    let line = content(line);
    if let Some((level, title)) = heading_of(line) {
        LineKind::Heading(level, title)
    } else if let Some(title) = entry_title_of(line) {
        LineKind::Title(title.to_string())
    } else {
        LineKind::Other
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
        let kinds: Vec<LineKind> = lines.iter().map(|line| line_kind(line)).collect();
        let starts_piece = |i: usize| !matches!(kinds[i], LineKind::Other);
        let piece_end = |from: usize| (from..lines.len()).find(|&i| starts_piece(i)).unwrap_or(lines.len());

        let mut i = piece_end(0);
        if i > 0 {
            doc.nodes.push(Node::Text(lines[..i].concat()));
        }
        while i < lines.len() {
            let end = piece_end(i + 1);
            match &kinds[i] {
                LineKind::Heading(level, title) => {
                    let blanks = lines[i + 1..end].iter().take_while(|line| is_blank(line)).count();
                    doc.nodes.push(Node::Section(Section {
                        level: *level,
                        title: title.clone(),
                        raw: lines[i..i + 1 + blanks].concat(),
                    }));
                    if i + 1 + blanks < end {
                        doc.nodes.push(Node::Text(lines[i + 1 + blanks..end].concat()));
                    }
                }
                LineKind::Title(title) => {
                    let id = doc.new_id();
                    let mut entry = parse_entry(id, title.clone(), &lines[i + 1..end]);
                    entry.raw = Some(lines[i..end].concat());
                    doc.nodes.push(Node::Entry(entry));
                }
                LineKind::Other => unreachable!("кусок начинается с заголовка или названия записи"),
            }
            i = end;
        }
        doc
    }

    /// Версия формата из метки в первой строке. `None`, если метки нет:
    /// такой файл читается как версия 1.
    pub fn format_version(&self) -> Option<u32> {
        let Some(Node::Text(text)) = self.nodes.first() else { return None };
        let line = content(text.lines().next()?);
        line.strip_prefix(HEADER_START)?.strip_suffix(HEADER_END)?.trim().parse().ok()
    }

    /// Ставит метку формата первой строкой, если её ещё нет.
    pub fn ensure_header(&mut self) {
        if self.format_version().is_some() {
            return;
        }
        let gap = if self.nodes.is_empty() { "" } else { self.eol };
        let header = format!("{HEADER_START}{FORMAT_VERSION}{HEADER_END}{eol}{gap}", eol = self.eol);
        self.nodes.insert(0, Node::Text(header));
    }

    /// В документе нет ни записей, ни разделов, ни текста, кроме метки формата.
    pub fn is_blank(&self) -> bool {
        let markdown = self.to_markdown();
        let mut lines = markdown.trim_start_matches('\u{feff}').lines();
        let first = lines.clone().next().unwrap_or_default();
        if first.starts_with(HEADER_START) {
            lines.next();
        }
        lines.all(|line| line.trim().is_empty())
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
            work.remove_if_empty(&current_path);
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

    /// Переносит запись в раздел `path`: перед записью `before`, если она лежит в этом
    /// разделе, иначе в конец раздела. Недостающие разделы создаются, опустевший
    /// раздел, откуда ушла запись, убирается.
    pub fn move_entry(&mut self, id: EntryId, path: Vec<String>, before: Option<EntryId>) -> Result<(), FormatError> {
        let path = normalize_path(path)?;
        let index = self.entry_index(id).ok_or(FormatError::NotFound)?;
        if before == Some(id) {
            return Ok(());
        }
        let mut work = self.clone();
        let origin = work.node_paths().swap_remove(index);
        let entry = work.nodes.remove(index);
        work.remove_if_empty(&origin);
        let parent = work.ensure_section(&path)?;
        let paths = work.node_paths();
        let target = before.and_then(|b| work.entry_index(b)).filter(|&i| paths[i] == path);
        let pos = target.unwrap_or_else(|| work.entry_slot(parent));
        let index = work.insert_spaced(pos, entry);
        work.close_gap_after(index);
        *self = work;
        Ok(())
    }

    /// Переносит раздел со всем содержимым в раздел `parent` (пустой — верхний
    /// уровень): перед разделом-соседом `before` или в конец. Уровни заголовков
    /// разделов внутри сдвигаются, записи остаются как были.
    pub fn move_section(
        &mut self,
        path: &[String],
        parent: &[String],
        before: Option<&[String]>,
    ) -> Result<(), FormatError> {
        if parent.starts_with(path) && !path.is_empty() {
            return Err(FormatError::IntoItself);
        }
        let start = self.section_index(path).ok_or(FormatError::NotFound)?;
        let title = path.last().cloned().ok_or(FormatError::NotFound)?;
        let origin = &path[..path.len() - 1];
        if parent != origin {
            let mut target = parent.to_vec();
            target.push(title.clone());
            if self.section_index(&target).is_some() {
                return Err(FormatError::SectionExists(title));
            }
        }

        let mut work = self.clone();
        let end = work.subtree_end(start);
        let mut chunk: Vec<Node> = work.nodes.drain(start..end).collect();
        work.remove_if_empty(origin);
        let parent_index = work.ensure_section(parent)?;
        let parent_level = parent_index.and_then(|p| work.section_level(p)).unwrap_or(0);

        let Some(Node::Section(head)) = chunk.first() else { unreachable!("раздел") };
        let delta = (parent_level + 1) as isize - head.level as isize;
        for node in &mut chunk {
            if let Node::Section(section) = node {
                let level = section.level as isize + delta;
                if !(1..=MAX_LEVEL as isize).contains(&level) {
                    return Err(FormatError::TooDeep);
                }
                relevel(&mut section.raw, section.level, level as usize);
                section.level = level as usize;
            }
        }

        let sibling = before.filter(|b| !b.is_empty() && &b[..b.len() - 1] == parent);
        let pos = sibling
            .and_then(|b| work.section_index(b))
            .unwrap_or_else(|| parent_index.map_or(work.nodes.len(), |p| work.subtree_end(p)));
        let count = chunk.len();
        let mut nodes = chunk.into_iter();
        let first = work.insert_spaced(pos, nodes.next().expect("раздел"));
        for (offset, node) in nodes.enumerate() {
            work.nodes.insert(first + 1 + offset, node);
        }
        work.close_gap_after(first + count - 1);
        *self = work;
        Ok(())
    }

    pub fn remove(&mut self, id: EntryId) -> Result<(), FormatError> {
        let index = self.entry_index(id).ok_or(FormatError::NotFound)?;
        let path = self.node_paths().swap_remove(index);
        self.nodes.remove(index);
        self.remove_if_empty(&path);
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
                Node::Section(s) => {
                    stack.retain(|(level, _)| *level < s.level);
                    stack.push((s.level, s.title.clone()));
                    titles(&stack)
                }
                Node::Text(_) | Node::Entry(_) => titles(&stack),
            })
            .collect()
    }

    fn section_level(&self, index: usize) -> Option<usize> {
        match &self.nodes[index] {
            Node::Section(s) => Some(s.level),
            _ => None,
        }
    }

    /// Конец раздела: следующий заголовок того же уровня или выше.
    fn subtree_end(&self, index: usize) -> usize {
        let level = self.section_level(index).expect("раздел");
        (index + 1..self.nodes.len())
            .find(|&i| self.section_level(i).is_some_and(|l| l <= level))
            .unwrap_or(self.nodes.len())
    }

    fn is_empty_section(&self, index: usize) -> bool {
        !self.nodes[index + 1..self.subtree_end(index)].iter().any(|node| match node {
            Node::Entry(_) => true,
            Node::Text(text) => !text.trim().is_empty(),
            Node::Section(_) => false,
        })
    }

    /// Убирает раздел `path` и его родителей, если в них не осталось ни записей, ни текста.
    /// Другие пустые разделы не трогает: их мог завести сам пользователь.
    fn remove_if_empty(&mut self, path: &[String]) {
        for depth in (1..=path.len()).rev() {
            let Some(index) = self.section_index(&path[..depth]) else { continue };
            if !self.is_empty_section(index) {
                break;
            }
            let end = self.subtree_end(index);
            self.nodes.drain(index..end);
        }
    }

    /// Позиция раздела по пути. Недостающие разделы создаются. `None` — верхний уровень.
    fn ensure_section(&mut self, path: &[String]) -> Result<Option<usize>, FormatError> {
        let mut parent = None;
        for depth in 0..path.len() {
            let prefix = &path[..=depth];
            parent = Some(match self.section_index(prefix) {
                Some(i) => i,
                None => self.create_section(parent, &path[depth])?,
            });
        }
        Ok(parent)
    }

    fn section_index(&self, path: &[String]) -> Option<usize> {
        let paths = self.node_paths();
        (0..self.nodes.len()).find(|&i| matches!(self.nodes[i], Node::Section(_)) && paths[i] == path)
    }

    /// Ставит пустую строку после узла, если за ним идёт что-то ещё.
    fn close_gap_after(&mut self, index: usize) {
        let has_next = index + 1 < self.nodes.len();
        let eol = self.eol;
        let text = match &mut self.nodes[index] {
            Node::Text(text) => text,
            Node::Section(section) => &mut section.raw,
            Node::Entry(entry) => match &mut entry.raw {
                Some(raw) => raw,
                None => {
                    entry.trailing_blank = if has_next { entry.trailing_blank.max(1) } else { 0 };
                    return;
                }
            },
        };
        if !text.ends_with('\n') {
            text.push_str(eol);
        }
        if has_next && !text.ends_with("\n\n") && !text.ends_with("\n\r\n") {
            text.push_str(eol);
        }
        // В конце файла пустые строки, которые узел принёс со старого места, не нужны.
        let blank = format!("{eol}{eol}");
        while !has_next && text.ends_with(&blank) {
            text.truncate(text.len() - eol.len());
        }
    }

    fn insert_entry(&mut self, id: EntryId, draft: EntryDraft, extra: Vec<String>) -> Result<(), FormatError> {
        let parent = self.ensure_section(&draft.path)?;
        let pos = self.entry_slot(parent);
        let entry = EntryNode {
            id,
            title: draft.title,
            fields: draft.fields,
            tags: draft.tags,
            extra,
            trailing_blank: 0,
            raw: None,
        };
        let index = self.insert_spaced(pos, Node::Entry(entry));
        self.close_gap_after(index);
        Ok(())
    }

    /// Добавляет раздел в конец родителя и возвращает его позицию.
    fn create_section(&mut self, parent: Option<usize>, title: &str) -> Result<usize, FormatError> {
        let level = parent.and_then(|p| self.section_level(p)).map_or(1, |l| l + 1);
        if level > MAX_LEVEL {
            return Err(FormatError::TooDeep);
        }
        let pos = parent.map_or(self.nodes.len(), |p| self.subtree_end(p));
        let raw = format!("{} {title}{eol}{eol}", "#".repeat(level), eol = self.eol);
        Ok(self.insert_spaced(pos, Node::Section(Section { level, title: title.to_string(), raw })))
    }

    /// Куда встаёт новая запись раздела: после его последней записи, а если записей
    /// нет — перед первым вложенным разделом. Записи верхнего уровня стоят до первого
    /// заголовка: после него запись уже принадлежала бы разделу.
    fn entry_slot(&self, parent: Option<usize>) -> usize {
        let (start, end) = match parent {
            Some(p) => (p + 1, self.subtree_end(p)),
            None => (0, self.nodes.len()),
        };
        let paths = self.node_paths();
        let parent_path = parent.map_or_else(Vec::new, |p| paths[p].clone());
        let last_own = (start..end).rev().find(|&i| matches!(self.nodes[i], Node::Entry(_)) && paths[i] == parent_path);
        match last_own {
            Some(i) => i + 1,
            None => (start..end).find(|&i| self.section_level(i).is_some()).unwrap_or(end),
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
pub(crate) fn content(line: &str) -> &str {
    let line = line.strip_suffix('\n').unwrap_or(line);
    line.strip_suffix('\r').unwrap_or(line)
}

fn is_blank(line: &str) -> bool {
    line.trim().is_empty()
}

/// Уровень и название заголовка `## раздел`.
pub(crate) fn heading_of(line: &str) -> Option<(usize, String)> {
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

/// Название записи из строки `**название**`. Строка должна начинаться с начала:
/// продолжения значений идут с отступом и записью не считаются.
pub(crate) fn entry_title_of(line: &str) -> Option<&str> {
    let line = line.trim_end();
    let inner = line.strip_prefix("**")?.strip_suffix("**")?.trim();
    (!inner.is_empty() && !inner.starts_with('*') && !inner.ends_with('*')).then_some(inner)
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

fn parse_entry(id: EntryId, title: String, body: &[&str]) -> EntryNode {
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
    EntryNode { id, title, fields, tags, extra, trailing_blank, raw: None }
}

fn render_entry(entry: &EntryNode, eol: &str, out: &mut String) {
    out.push_str("**");
    out.push_str(&entry.title);
    out.push_str("**");
    out.push_str(eol);

    // Строка без ключа с отступом после поля прочиталась бы как продолжение его
    // значения. Поэтому такие строки идут до полей, а остальные — после.
    let indented = |line: &&String| line.starts_with([' ', '\t']);
    for line in entry.extra.iter().filter(indented) {
        out.push_str(line);
        out.push_str(eol);
    }

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

    for line in entry.extra.iter().filter(|line| !indented(line)) {
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

/// Можно ли записать поле с таким ключом так, чтобы оно прочиталось обратно полем.
pub fn is_valid_key(key: &str) -> bool {
    !key.is_empty()
        && !key.contains(|c: char| c.is_whitespace() || c == ':')
        && !key.ends_with(SECRET_MARKER)
        && !key.starts_with('#')
        && !key.starts_with('*')
        && !key.eq_ignore_ascii_case(TAGS_KEY)
}

/// Проверяет путь к разделу и убирает пробелы по краям названий.
fn normalize_path(parts: Vec<String>) -> Result<Vec<String>, FormatError> {
    let mut path = Vec::with_capacity(parts.len());
    for part in parts {
        let part = part.trim();
        if part.is_empty() {
            return Err(FormatError::EmptySection);
        }
        if part.contains(['\n', '\r']) {
            return Err(FormatError::LineBreakInName);
        }
        path.push(part.to_string());
    }
    if path.len() > MAX_LEVEL {
        return Err(FormatError::TooDeep);
    }
    Ok(path)
}

/// Меняет уровень заголовка. Исходный текст сохраняется, меняются только решётки.
fn relevel(raw: &mut String, old: usize, new: usize) {
    if old != new && raw.len() >= old {
        *raw = format!("{}{}", "#".repeat(new), &raw[old..]);
    }
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
    if title.starts_with('*') || title.ends_with('*') {
        return Err(FormatError::InvalidTitle);
    }

    let path = normalize_path(draft.path)?;

    let mut fields = Vec::with_capacity(draft.fields.len());
    for field in draft.fields {
        let key = field.key.trim();
        if !is_valid_key(key) {
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
