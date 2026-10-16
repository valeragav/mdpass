# CLAUDE.md

mdpass — менеджер паролей на Tauri 2: все записи в одном Markdown-файле, зашифрованном age.
Устройство проекта, команды и выпуск описаны в [docs/setup.md](docs/setup.md), модель угроз —
в [docs/SECURITY.md](docs/SECURITY.md), формат файла — в [docs/guide.md](docs/guide.md).
Здесь только то, что нужно знать, чтобы правильно менять код.

## Проверки

Те же, что в CI. Порядок важен: Tauri встраивает `dist` в бинарник, поэтому без
`npm run build` не соберутся ни `cargo clippy`, ни `cargo test`.

```sh
npm run build                                          # svelte-check + vite build
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
npm run test:ui                                        # Playwright, сам поднимает npm run dev
```

Один тест: `cargo test -p mdpass-core --test format <имя>`, `npx playwright test -g "<название>"`.

## Где что

- `crates/mdpass-core` — формат, шифрование, копии, импорт. Без Tauri. Логику с данными
  писать здесь и покрывать тестами в `crates/mdpass-core/tests`.
- `src-tauri` — команды, сессия, автоблокировка, буфер обмена.
- `src` — Svelte 5 (руны) + TypeScript.
- `src/dev/mock.ts` — заглушка Rust-части для `npm run dev` и Playwright. Пароль `password1`.

## Новая или изменённая команда Tauri

Менять все места сразу, иначе браузерный режим и тесты интерфейса разойдутся с приложением:

1. `src-tauri/src/commands.rs` — сама команда.
2. `src-tauri/src/lib.rs` — `generate_handler![...]`.
3. `src/lib/api.ts` — обёртка и типы (они повторяют `commands.rs`).
4. `src/dev/mock.ts` — ветка в `switch (cmd)`.
5. Если нужен новый плагин или разрешение — `src-tauri/capabilities/default.json`,
   разрешения давать минимальные.

## Правила, которые нельзя нарушать

- **Секреты расшифровываются только в Rust.** Интерфейс получает записи без значений
  секретов и просит значение отдельной командой по «показать» или «скопировать».
  Не отдавать секреты в интерфейс в списках, поиске, логах и ошибках.
- **Формат хранит исходный текст.** Файл после разбора и сборки без изменений совпадает
  с исходным побайтно. Неизменённые записи выводятся как были. Любая правка формата
  проходит `tests/format.rs` и проптесты `tests/properties.rs`.
- **Запись в файл атомарная** (`fs_atomic.rs`) и проверяет, что файл не меняли снаружи.
- **Только выдуманные данные** в моках, тестах, примерах и демо: адреса из диапазонов для
  документации (`203.0.113.0/24` и т. п.), случайные пароли. Репозиторий публичный.
- Не коммитить хранилища: `*.md.age`, `*.damaged-*`, `.mdpass-*.tmp` уже в `.gitignore`.

## Стиль

- Комментарии в коде, документация, CHANGELOG и тексты интерфейса — на русском. Пишутся
  простыми короткими фразами, с точки зрения пользователя.
- Сообщения коммитов — на английском, одна строка без префиксов, по сути изменения:
  `Favorites scroll on their own: 5 rows visible, 10 when expanded`.
- Rust: `rustfmt.toml` (ширина 120), edition 2024. Clippy без предупреждений.

## Выпуск

Версия живёт в трёх местах: `package.json`, `Cargo.toml`, `src-tauri/tauri.conf.json`.
В `CHANGELOG.md` раздел `## X.Y.Z — ГГГГ-ММ-ДД`: сначала новое списком, потом
`### Исправления`. Тег `vX.Y.Z` запускает
`release.yml`. Перед выпуском — [docs/manual-check.md](docs/manual-check.md).
