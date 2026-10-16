use mdpass_core::format::Document;
use mdpass_core::merge::{Choice, Merge, merge, resolve};

const BASE: &str = "\
<!-- mdpass:1 -->

# work

**web-01**
ssh: deploy@203.0.113.10
password: correct-horse
tags: [work]

**db**
user: app_user
password: battery-staple

# home

**router**
url: http://192.0.2.1
password: admin-1
";

fn run(base: &str, mine: &str, theirs: &str) -> Merge {
    merge(&Document::parse(base), &Document::parse(mine), &Document::parse(theirs)).unwrap()
}

fn clean(base: &str, mine: &str, theirs: &str) -> String {
    let result = run(base, mine, theirs);
    assert!(result.conflicts.is_empty(), "конфликты: {:?}", result.conflicts);
    result.document.to_markdown()
}

#[test]
fn nothing_changed_keeps_their_file_byte_for_byte() {
    // Ручное форматирование в их файле не теряется.
    let theirs = BASE.replace("password: admin-1", "password:   admin-1  ");
    assert_eq!(clean(BASE, BASE, &theirs), theirs);
}

#[test]
fn only_mine_changed_gives_mine() {
    let mine = BASE.replace("correct-horse", "new-horse");
    assert_eq!(clean(BASE, &mine, BASE), mine);
}

#[test]
fn changes_in_different_entries_both_land() {
    let mine = BASE.replace("correct-horse", "new-horse");
    let theirs = BASE.replace("admin-1", "admin-2");
    assert_eq!(clean(BASE, &mine, &theirs), BASE.replace("correct-horse", "new-horse").replace("admin-1", "admin-2"));
}

#[test]
fn different_fields_of_one_entry_merge() {
    let mine = BASE.replace("password: correct-horse", "password: new-horse");
    let theirs = BASE.replace("ssh: deploy@203.0.113.10", "ssh: root@203.0.113.11");
    let out = clean(BASE, &mine, &theirs);
    assert!(out.contains("ssh: root@203.0.113.11\npassword: new-horse\n"), "{out}");
}

#[test]
fn same_field_changed_both_ways_is_a_conflict() {
    let mine = BASE.replace("correct-horse", "mine-horse");
    let theirs = BASE.replace("correct-horse", "their-horse");
    let result = run(BASE, &mine, &theirs);
    assert_eq!(result.conflicts.len(), 1);
    assert_eq!(result.conflicts[0].keys, ["password"]);
    // Пока конфликт не решён, в файле их версия.
    assert_eq!(result.document.to_markdown(), theirs);
}

#[test]
fn same_change_on_both_sides_is_not_a_conflict() {
    let changed = BASE.replace("correct-horse", "same-horse");
    assert_eq!(clean(BASE, &changed, &changed), changed);
}

#[test]
fn resolve_mine_theirs_and_both() {
    let mine = BASE.replace("correct-horse", "mine-horse");
    let theirs = BASE.replace("correct-horse", "their-horse");

    let mut result = run(BASE, &mine, &theirs);
    let conflict = result.conflicts[0].clone();
    resolve(&mut result.document, &conflict, Choice::Mine).unwrap();
    assert_eq!(result.document.to_markdown(), mine);

    let mut result = run(BASE, &mine, &theirs);
    let conflict = result.conflicts[0].clone();
    resolve(&mut result.document, &conflict, Choice::Theirs).unwrap();
    assert_eq!(result.document.to_markdown(), theirs);

    let mut result = run(BASE, &mine, &theirs);
    let conflict = result.conflicts[0].clone();
    resolve(&mut result.document, &conflict, Choice::Both { title: "web-01 (копия)".into() }).unwrap();
    let titles: Vec<String> = result.document.entries().into_iter().map(|e| e.title).collect();
    assert_eq!(titles, ["web-01", "db", "web-01 (копия)", "router"]);
    let out = result.document.to_markdown();
    assert!(out.contains("their-horse") && out.contains("mine-horse"), "{out}");
}

#[test]
fn delete_on_one_side_without_edits_deletes() {
    let mine = BASE.replace("**db**\nuser: app_user\npassword: battery-staple\n\n", "");
    let theirs = BASE.replace("admin-1", "admin-2");
    let out = clean(BASE, &mine, &theirs);
    assert!(!out.contains("**db**"), "{out}");
    assert!(out.contains("admin-2"), "{out}");

    // И наоборот: удалили там, я не трогал.
    let out = clean(BASE, &theirs, &mine);
    assert!(!out.contains("**db**"), "{out}");
}

#[test]
fn delete_against_edit_is_a_conflict() {
    let deleted = BASE.replace("**db**\nuser: app_user\npassword: battery-staple\n\n", "");
    let edited = BASE.replace("battery-staple", "new-staple");

    let result = run(BASE, &deleted, &edited);
    assert_eq!(result.conflicts.len(), 1);
    assert!(result.conflicts[0].mine.is_none());
    assert!(result.document.to_markdown().contains("new-staple"));

    let result = run(BASE, &edited, &deleted);
    assert_eq!(result.conflicts.len(), 1);
    assert!(result.conflicts[0].theirs.is_none());
    let mut document = result.document;
    resolve(&mut document, &result.conflicts[0], Choice::Mine).unwrap();
    assert!(document.to_markdown().contains("new-staple"));
}

#[test]
fn new_entries_from_both_sides_land() {
    let mine = BASE.replace("# home\n", "**cache**\nport: 6379\n\n# home\n");
    let theirs = format!("{BASE}\n**nas**\nurl: http://192.0.2.5\n");
    let out = clean(BASE, &mine, &theirs);
    assert!(out.contains("**cache**\nport: 6379") && out.contains("**nas**"), "{out}");
}

#[test]
fn same_new_entry_on_both_sides_is_not_duplicated() {
    let added = format!("{BASE}\n**nas**\nurl: http://192.0.2.5\n");
    let out = clean(BASE, &added, &added);
    assert_eq!(out.matches("**nas**").count(), 1);
}

#[test]
fn different_new_entries_with_one_title_conflict() {
    let mine = format!("{BASE}\n**nas**\nurl: http://192.0.2.5\n");
    let theirs = format!("{BASE}\n**nas**\nurl: http://192.0.2.6\n");
    let result = run(BASE, &mine, &theirs);
    assert_eq!(result.conflicts.len(), 1);
    assert_eq!(result.conflicts[0].keys, ["url"]);
}

#[test]
fn moved_entry_keeps_their_edit() {
    // Я перенёс запись в другой раздел, там поменяли пароль.
    let mine = BASE
        .replace("**db**\nuser: app_user\npassword: battery-staple\n\n", "")
        .replace("password: admin-1\n", "password: admin-1\n\n**db**\nuser: app_user\npassword: battery-staple\n");
    let theirs = BASE.replace("battery-staple", "new-staple");
    let out = clean(BASE, &mine, &theirs);
    let doc = Document::parse(&out);
    let db: Vec<_> = doc.entries().into_iter().filter(|e| e.title == "db").collect();
    assert_eq!(db.len(), 1, "{out}");
    assert_eq!(db[0].path, ["home"]);
    assert_eq!(db[0].fields[1].value, "new-staple");
}

#[test]
fn renamed_entry_keeps_their_edit() {
    let mine = BASE.replace("**router**", "**router (дом)**");
    let theirs = BASE.replace("admin-1", "admin-2");
    let out = clean(BASE, &mine, &theirs);
    assert!(out.contains("**router (дом)**\nurl: http://192.0.2.1\npassword: admin-2"), "{out}");
    assert!(!out.contains("**router**"), "{out}");
}

#[test]
fn tags_merge_as_a_set() {
    let mine = BASE.replace("tags: [work]", "tags: [work], [prod]");
    let theirs = BASE.replace("tags: [work]", "tags: [ssh]").replace("admin-1", "admin-2");
    let out = clean(BASE, &mine, &theirs.replace("tags: [ssh]", "tags: [work], [ssh]"));
    assert!(out.contains("tags: [work], [ssh], [prod]"), "{out}");
    // Убранный мной тег убирается и у них.
    let mine = BASE.replace("tags: [work]", "tags: [prod]");
    let theirs = BASE.replace("tags: [work]", "tags: [work], [ssh]");
    let out = clean(BASE, &mine, &theirs);
    assert!(out.contains("tags: [ssh], [prod]"), "{out}");
}

#[test]
fn conflicts_never_carry_secret_values_in_keys() {
    let mine = BASE.replace("correct-horse", "mine-horse");
    let theirs = BASE.replace("correct-horse", "their-horse");
    let result = run(BASE, &mine, &theirs);
    let keys = result.conflicts[0].keys.join(" ");
    assert!(!keys.contains("horse"));
}

#[test]
fn conflict_still_merges_the_other_fields() {
    // Пароль поменяли по-разному, а ssh — только здесь, url — только там.
    let base = "**web-01**\nssh: deploy@203.0.113.10\npassword: old\nurl: https://a.example.test\n";
    let mine = "**web-01**\nssh: root@203.0.113.10\npassword: mine\nurl: https://a.example.test\n";
    let theirs = "**web-01**\nssh: deploy@203.0.113.10\npassword: theirs\nurl: https://b.example.test\n";
    let result = run(base, mine, theirs);
    assert_eq!(result.conflicts.len(), 1);
    assert_eq!(result.conflicts[0].keys, ["password"]);
    assert_eq!(
        result.document.to_markdown(),
        "**web-01**\nssh: root@203.0.113.10\npassword: theirs\nurl: https://b.example.test\n"
    );

    // «Моя версия» берёт только спорное поле.
    let mut document = result.document;
    resolve(&mut document, &result.conflicts[0], Choice::Mine).unwrap();
    assert_eq!(
        document.to_markdown(),
        "**web-01**\nssh: root@203.0.113.10\npassword: mine\nurl: https://b.example.test\n"
    );
}

#[test]
fn collapse_folds_the_copy_back() {
    use mdpass_core::merge::collapse;
    let base = "**web-01**\nlogin: a\npassword: old\n";
    let mine = "**web-01**\nlogin: b\npassword: mine\n";
    let theirs = "**web-01**\nlogin: a\npassword: theirs\n";

    for keep_mine in [true, false] {
        let mut result = run(base, mine, theirs);
        let conflict = result.conflicts[0].clone();
        resolve(&mut result.document, &conflict, Choice::Both { title: "web-01 (копия)".into() }).unwrap();
        let entries = result.document.entries();
        let original = entries.iter().find(|e| e.title == "web-01").unwrap();
        let copy = entries.iter().find(|e| e.title == "web-01 (копия)").unwrap();
        collapse(&mut result.document, original, copy, &conflict.keys, "web-01", keep_mine).unwrap();

        let password = if keep_mine { "mine" } else { "theirs" };
        // После удалённой копии может остаться пустая строка в конце: так удаляется любая запись.
        assert_eq!(result.document.to_markdown().trim_end(), format!("**web-01**\nlogin: b\npassword: {password}"));
    }
}

#[test]
fn collapse_keeps_my_title_not_the_copy_mark() {
    use mdpass_core::merge::{TITLE, collapse};
    let base = "**web**\nk: v\n";
    let mine = "**web (мой)**\nk: v\n";
    let theirs = "**web (их)**\nk: v\n";
    let mut result = run(base, mine, theirs);
    let conflict = result.conflicts[0].clone();
    assert_eq!(conflict.keys, [TITLE]);
    resolve(&mut result.document, &conflict, Choice::Both { title: "web (мой) (копия)".into() }).unwrap();
    let entries = result.document.entries();
    let original = entries.iter().find(|e| e.title == "web (их)").unwrap();
    let copy = entries.iter().find(|e| e.title == "web (мой) (копия)").unwrap();
    collapse(&mut result.document, original, copy, &conflict.keys, "web (мой)", true).unwrap();
    let titles: Vec<String> = result.document.entries().into_iter().map(|e| e.title).collect();
    assert_eq!(titles, ["web (мой)"]);
}
