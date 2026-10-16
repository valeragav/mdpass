//! Слияние двух версий хранилища, которые разошлись от общей базы: «моя» — та, что
//! в приложении, «их» — та, что пришла на диск с другого устройства.
//!
//! Результат строится из «их»: записи, которых я не менял, остаются в файле байт
//! в байт. Мои изменения переносятся поверх обычными правками документа. Если одно
//! и то же изменили с двух сторон по-разному, это конфликт: в результате остаётся
//! «их» версия записи, а выбор за пользователем ([`resolve`]).
//!
//! Записи сопоставляются без id в файле: по разделу и названию, потом только по
//! названию (запись перенесли), потом по полям в том же разделе (переименовали).
//! Промах даёт лишнюю запись или конфликт, но не потерю данных.

use crate::format::{Document, Entry, EntryDraft, EntryId, Field, FormatError};
use crate::importer::portable_draft;

/// Запись, которую изменили с двух сторон по-разному.
#[derive(Debug, Clone)]
pub struct Conflict {
    /// Моя версия записи. `None` — я её удалил.
    pub mine: Option<Entry>,
    /// Их версия записи в результате слияния. `None` — её удалили на другом устройстве.
    pub theirs: Option<Entry>,
    /// Что разошлось: ключи полей, а также `title`, `section`, `tags`, `extra`.
    /// Значения полей сюда не попадают.
    pub keys: Vec<String>,
}

#[derive(Debug)]
pub struct Merge {
    pub document: Document,
    pub conflicts: Vec<Conflict>,
    /// Номер каждой моей записи в слитом документе: в нём номера из их версии.
    /// Записи, которых в результате нет, сюда не попадают.
    pub ids: Vec<(EntryId, EntryId)>,
}

impl Merge {
    /// Номер моей записи в слитом документе.
    pub fn id(&self, mine: EntryId) -> Option<EntryId> {
        self.ids.iter().find(|(from, _)| *from == mine).map(|&(_, to)| to)
    }

    /// Решает конфликт номер `index` и обновляет номера записей.
    pub fn resolve(&mut self, index: usize, choice: Choice) -> Result<(), FormatError> {
        let conflict = self.conflicts[index].clone();
        let at = resolve(&mut self.document, &conflict, choice)?;
        if let Some(my) = &conflict.mine {
            self.ids.retain(|(from, _)| *from != my.id);
            if let Some(at) = at {
                self.ids.push((my.id, at));
            }
        }
        Ok(())
    }
}

/// Чем решить конфликт.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Choice {
    Mine,
    Theirs,
    /// Оставить обе: моя версия добавляется рядом с этим названием.
    Both {
        title: String,
    },
}

/// Сливает `mine` и `theirs`, которые разошлись от `base`.
pub fn merge(base: &Document, mine: &Document, theirs: &Document) -> Result<Merge, FormatError> {
    let base = base.entries();
    let mine = mine.entries();
    let their_entries = theirs.entries();
    let to_mine = pair(&base, &mine);
    let to_theirs = pair(&base, &their_entries);

    let mut document = theirs.clone();
    let mut conflicts = Vec::new();
    let mut ids = Vec::new();

    for (b, original) in base.iter().enumerate() {
        let my = to_mine[b].map(|i| &mine[i]);
        let their = to_theirs[b].map(|i| &their_entries[i]);
        match (my, their) {
            (None, None) => {}
            // Я удалил. Если там запись не трогали — удаляем и у них.
            (None, Some(their)) => {
                if same(their, original) {
                    document.remove(their.id)?;
                } else {
                    conflicts.push(Conflict {
                        mine: None,
                        theirs: Some(their.clone()),
                        keys: differing(original, their),
                    });
                }
            }
            // Удалили там. Если я запись не трогал — она так и остаётся удалённой.
            (Some(my), None) => {
                if !same(my, original) {
                    conflicts.push(Conflict { mine: Some(my.clone()), theirs: None, keys: differing(original, my) });
                }
            }
            (Some(my), Some(their)) => {
                ids.push((my.id, their.id));
                match merge_entry(original, my, their) {
                    Outcome::Keep => {}
                    Outcome::Update(merged) => apply(&mut document, their, &merged)?,
                    Outcome::Conflict(keys) => {
                        conflicts.push(Conflict { mine: Some(my.clone()), theirs: Some(their.clone()), keys })
                    }
                }
            }
        }
    }

    // Записи, которых не было в базе. Одинаковые новые записи с двух сторон
    // не дублируются, разные под одним названием — конфликт.
    let mut their_new: Vec<&Entry> =
        their_entries.iter().enumerate().filter(|(i, _)| !to_theirs.contains(&Some(*i))).map(|(_, e)| e).collect();
    for (i, my) in mine.iter().enumerate() {
        if to_mine.contains(&Some(i)) {
            continue;
        }
        match their_new.iter().position(|their| their.path == my.path && their.title == my.title) {
            Some(pos) => {
                let their = their_new.remove(pos);
                ids.push((my.id, their.id));
                if !same(my, their) {
                    conflicts.push(Conflict {
                        mine: Some(my.clone()),
                        theirs: Some(their.clone()),
                        keys: differing(their, my),
                    });
                }
            }
            None => ids.push((my.id, put(&mut document, None, my)?)),
        }
    }

    Ok(Merge { document, conflicts, ids })
}

/// Решает конфликт в документе, который вернул [`merge`]. Возвращает номер записи,
/// где теперь лежит моя версия: `None`, если её в документе нет.
pub fn resolve(document: &mut Document, conflict: &Conflict, choice: Choice) -> Result<Option<EntryId>, FormatError> {
    match (choice, &conflict.mine, &conflict.theirs) {
        (Choice::Theirs, Some(_), Some(their)) => Ok(Some(their.id)),
        (Choice::Theirs | Choice::Both { .. }, None, _) | (Choice::Theirs, _, None) => Ok(None),
        (Choice::Mine, None, Some(their)) => document.remove(their.id).map(|()| None),
        (Choice::Mine, Some(my), Some(their)) => apply(document, their, my).map(|()| Some(their.id)),
        (Choice::Mine | Choice::Both { .. }, Some(my), None) => put(document, None, my).map(Some),
        (Choice::Both { title }, Some(my), Some(_)) => put(document, None, &Entry { title, ..my.clone() }).map(Some),
        (Choice::Mine, None, None) => Ok(None),
    }
}

enum Outcome {
    /// В результате остаётся их версия.
    Keep,
    /// Их запись заменяется слитой.
    Update(Entry),
    Conflict(Vec<String>),
}

fn merge_entry(base: &Entry, mine: &Entry, theirs: &Entry) -> Outcome {
    if same(mine, base) || same(mine, theirs) {
        return Outcome::Keep;
    }
    if same(theirs, base) {
        return Outcome::Update(mine.clone());
    }

    let mut keys = Vec::new();
    let mut merged = theirs.clone();
    match pick(&base.path, &mine.path, &theirs.path) {
        Some(path) => merged.path = path,
        None => keys.push("section".to_string()),
    }
    match pick(&base.title, &mine.title, &theirs.title) {
        Some(title) => merged.title = title,
        None => keys.push("title".to_string()),
    }
    match pick(&base.extra, &mine.extra, &theirs.extra) {
        Some(extra) => merged.extra = extra,
        None => keys.push("extra".to_string()),
    }
    match merge_fields(base, mine, theirs) {
        Ok(fields) => merged.fields = fields,
        Err(clashing) => keys.extend(clashing),
    }
    merged.tags = merge_tags(&base.tags, &mine.tags, &theirs.tags);
    // Избранное и его место — не повод для конфликта: берём моё.
    merged.favorite = pick(&base.favorite, &mine.favorite, &theirs.favorite).unwrap_or(mine.favorite);

    if keys.is_empty() { Outcome::Update(merged) } else { Outcome::Conflict(keys) }
}

/// Значение после слияния: изменённое с одной стороны или одинаковое с обеих.
/// `None` — его изменили с двух сторон по-разному.
fn pick<T: PartialEq + Clone>(base: &T, mine: &T, theirs: &T) -> Option<T> {
    if mine == base || mine == theirs {
        Some(theirs.clone())
    } else if theirs == base {
        Some(mine.clone())
    } else {
        None
    }
}

/// Поля сливаются по ключам. Порядок — их, если там порядок не меняли, иначе мой.
/// В ошибке — ключи, которые изменили с двух сторон по-разному.
fn merge_fields(base: &Entry, mine: &Entry, theirs: &Entry) -> Result<Vec<Field>, Vec<String>> {
    if [base, mine, theirs].iter().any(|e| has_duplicate_keys(e)) {
        return pick(&base.fields, &mine.fields, &theirs.fields).ok_or_else(|| differing_fields(mine, theirs));
    }
    let keys = |e: &Entry| e.fields.iter().map(|f| f.key.clone()).collect::<Vec<_>>();
    let order_from = if keys(theirs) == keys(base) { mine } else { theirs };
    let mut order: Vec<String> = Vec::new();
    for key in [order_from, theirs, mine].into_iter().flat_map(keys) {
        if !order.contains(&key) {
            order.push(key);
        }
    }

    let field = |e: &Entry, key: &str| e.fields.iter().find(|f| f.key == key).cloned();
    let mut fields = Vec::new();
    let mut clashing = Vec::new();
    for key in order {
        match pick(&field(base, &key), &field(mine, &key), &field(theirs, &key)) {
            Some(Some(f)) => fields.push(f),
            Some(None) => {}
            None => clashing.push(key),
        }
    }
    if clashing.is_empty() { Ok(fields) } else { Err(clashing) }
}

/// Теги — множество: из их списка убираются те, что убрал я, и добавляются мои новые.
fn merge_tags(base: &[String], mine: &[String], theirs: &[String]) -> Vec<String> {
    if mine == base {
        return theirs.to_vec();
    }
    if theirs == base {
        return mine.to_vec();
    }
    let mut tags: Vec<String> = theirs.iter().filter(|t| !base.contains(t) || mine.contains(t)).cloned().collect();
    for tag in mine {
        if !base.contains(tag) && !tags.contains(tag) {
            tags.push(tag.clone());
        }
    }
    tags
}

/// Заменяет их запись в документе на `entry`.
fn apply(document: &mut Document, theirs: &Entry, entry: &Entry) -> Result<(), FormatError> {
    // Только перенос: запись переезжает как есть, с исходным текстом. Правка
    // нормализовала бы значения, как при сохранении из редактора.
    let moved = Entry { path: entry.path.clone(), ..theirs.clone() };
    if same(&moved, entry) && document.move_entry(theirs.id, entry.path.clone(), None).is_ok() {
        return Ok(());
    }
    put(document, Some(theirs.id), entry).map(drop)
}

/// Записывает `entry` на место записи `id` или новой записью. Если формат не даёт
/// записать её как есть, пробует проще: без разделов с пустым названием, ближе
/// к верхнему уровню (раздел оказался слишком глубоко), а в конце — как при импорте,
/// когда лишнее уходит в заметку. Слияние не должно падать из-за странного файла.
fn put(document: &mut Document, id: Option<EntryId>, entry: &Entry) -> Result<EntryId, FormatError> {
    let exact = draft(entry);
    let cleaned: Vec<String> =
        entry.path.iter().map(|part| part.trim().to_string()).filter(|part| !part.is_empty()).collect();
    let portable = portable_draft(entry);

    let mut tries = vec![(exact.clone(), entry.extra.clone())];
    for len in (0..=cleaned.len()).rev() {
        tries.push((EntryDraft { path: cleaned[..len].to_vec(), ..exact.clone() }, entry.extra.clone()));
    }
    for len in (0..=portable.path.len()).rev() {
        tries.push((EntryDraft { path: portable.path[..len].to_vec(), ..portable.clone() }, Vec::new()));
    }

    let mut error = FormatError::NotFound;
    for (draft, extra) in tries {
        let done = match id {
            Some(id) => document.update(id, draft).map(|()| id),
            None => document.add(draft),
        };
        match done {
            Ok(id) => {
                document.set_extra(id, extra)?;
                return Ok(id);
            }
            Err(err) => error = err,
        }
    }
    Err(error)
}

fn draft(entry: &Entry) -> EntryDraft {
    EntryDraft {
        path: entry.path.clone(),
        title: entry.title.clone(),
        fields: entry.fields.clone(),
        tags: entry.tags.clone(),
        favorite: entry.favorite,
    }
}

/// Совпадают ли записи по содержимому. Номер записи в документе не важен.
fn same(a: &Entry, b: &Entry) -> bool {
    a.path == b.path
        && a.title == b.title
        && a.fields == b.fields
        && a.tags == b.tags
        && a.favorite == b.favorite
        && a.extra == b.extra
}

fn has_duplicate_keys(entry: &Entry) -> bool {
    entry.fields.iter().enumerate().any(|(i, f)| entry.fields[..i].iter().any(|g| g.key == f.key))
}

/// Что отличается у двух версий записи: ключи полей и части записи.
fn differing(a: &Entry, b: &Entry) -> Vec<String> {
    let mut keys = Vec::new();
    if a.path != b.path {
        keys.push("section".to_string());
    }
    if a.title != b.title {
        keys.push("title".to_string());
    }
    keys.extend(differing_fields(a, b));
    if a.tags != b.tags {
        keys.push("tags".to_string());
    }
    if a.extra != b.extra {
        keys.push("extra".to_string());
    }
    keys
}

fn differing_fields(a: &Entry, b: &Entry) -> Vec<String> {
    let value = |e: &Entry, key: &str| e.fields.iter().filter(|f| f.key == key).cloned().collect::<Vec<_>>();
    let mut keys: Vec<String> = Vec::new();
    for field in a.fields.iter().chain(&b.fields) {
        if !keys.contains(&field.key) && value(a, &field.key) != value(b, &field.key) {
            keys.push(field.key.clone());
        }
    }
    keys
}

/// Для каждой записи базы — её пара в другой версии.
fn pair(base: &[Entry], other: &[Entry]) -> Vec<Option<usize>> {
    let mut found: Vec<Option<usize>> = vec![None; base.len()];
    let mut taken = vec![false; other.len()];
    let link = |found: &mut [Option<usize>], taken: &mut [bool], b: usize, o: usize| {
        found[b] = Some(o);
        taken[o] = true;
    };

    // Сначала записи, которые не менялись. Так одноимённые записи в одном разделе
    // не путаются между собой.
    for (b, entry) in base.iter().enumerate() {
        if let Some(o) = (0..other.len()).find(|&o| !taken[o] && same(&other[o], entry)) {
            link(&mut found, &mut taken, b, o);
        }
    }

    // Тот же раздел и название. Одноимённые записи идут парами по порядку.
    for (b, entry) in base.iter().enumerate() {
        if found[b].is_some() {
            continue;
        }
        if let Some(o) =
            (0..other.len()).find(|&o| !taken[o] && other[o].path == entry.path && other[o].title == entry.title)
        {
            link(&mut found, &mut taken, b, o);
        }
    }

    // То же название в другом разделе: запись перенесли. Только если оно единственное
    // среди оставшихся с обеих сторон.
    for b in 0..base.len() {
        if found[b].is_some() {
            continue;
        }
        let title = &base[b].title;
        let alone = (0..base.len()).filter(|&x| found[x].is_none() && base[x].title == *title).count() == 1;
        let candidates: Vec<usize> = (0..other.len()).filter(|&o| !taken[o] && other[o].title == *title).collect();
        if alone && candidates.len() == 1 {
            link(&mut found, &mut taken, b, candidates[0]);
        }
    }

    // Тот же раздел и похожие поля: запись переименовали. Только если лучший
    // кандидат один.
    for b in 0..base.len() {
        if found[b].is_some() {
            continue;
        }
        let scored: Vec<(usize, f64)> = (0..other.len())
            .filter(|&o| !taken[o] && other[o].path == base[b].path)
            .map(|o| (o, similarity(&base[b], &other[o])))
            .filter(|&(_, score)| score >= 0.5)
            .collect();
        let best = scored.iter().map(|&(_, s)| s).fold(0.0, f64::max);
        let top: Vec<usize> = scored.iter().filter(|&&(_, s)| s == best).map(|&(o, _)| o).collect();
        if top.len() == 1 {
            link(&mut found, &mut taken, b, top[0]);
        }
    }
    found
}

/// Доля совпадающих непустых полей. Пустые записи похожими не считаются.
fn similarity(a: &Entry, b: &Entry) -> f64 {
    let total = a.fields.len().max(b.fields.len());
    let shared = a.fields.iter().filter(|f| !f.value.is_empty() && b.fields.contains(f)).count();
    if shared == 0 { 0.0 } else { shared as f64 / total as f64 }
}
