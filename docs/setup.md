# Установка и разработка

- [Установка](#установка)
- [Сборка из исходников](#сборка-из-исходников)
- [Разработка](#разработка)
- [Устройство проекта](#устройство-проекта)
- [Выпуск](#выпуск)

## Установка

Установщики для Windows, macOS и Linux выкладываются в
[выпуски](https://github.com/valeragav/mdpass/releases/latest):

| Система | Файл |
| --- | --- |
| Windows | `mdpass_<версия>_x64-setup.exe` |
| macOS (Apple Silicon и Intel) | `mdpass_<версия>_aarch64.dmg`, `mdpass_<версия>_x64.dmg` |
| Linux | `.AppImage`, `.deb`, `.rpm` |

Пока установщики не подписаны: Windows покажет «Неизвестный издатель» («Подробнее» →
«Выполнить в любом случае»), macOS попросит открыть приложение через контекстное меню.

Удаление приложения хранилища не трогает.

## Сборка из исходников

Нужны [Rust](https://rustup.rs) (stable), [Node.js](https://nodejs.org) 22 и
[зависимости Tauri](https://tauri.app/start/prerequisites/) для вашей системы
(на Windows — WebView2, он уже есть в Windows 10 и 11).

```sh
git clone https://github.com/valeragav/mdpass.git
cd mdpass
npm install
npm run tauri build                  # установщик для текущей системы
npm run tauri build -- --no-bundle   # только приложение, без установщика
```

Готовое приложение — `target/release/mdpass` (`mdpass.exe` на Windows), установщики —
в `target/release/bundle/`.

Демо-хранилище с выдуманными записями, чтобы попробовать приложение:

```sh
cargo run -p mdpass-core --example demo_vault -- ./demo.md.age "demo-pass-123"
```

## Разработка

```sh
npm run tauri dev          # приложение с горячей перезагрузкой
npm run dev                # только интерфейс в браузере: команды Rust заменяет заглушка
cargo test --workspace     # тесты ядра и приложения, включая стресс-тесты формата
npm run test:ui            # сценарии экранов в невидимом браузере (Playwright)
npm run build              # проверка типов и сборка интерфейса
npm run demo:gif           # перезаписать docs/demo.gif (нужен ffmpeg)
```

В режиме `npm run dev` верный мастер-пароль — `password1`, все данные в заглушке
(`src/dev/mock.ts`) выдуманные. В сборку приложения заглушка не попадает.

Проверки перед коммитом — те же, что в CI:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
npm run build
npm run test:ui
```

Перед выпуском — ручная проверка в настоящем окне по [чек-листу](manual-check.md).

## Устройство проекта

| Папка | Что внутри |
| --- | --- |
| `crates/mdpass-core` | формат файла, шифрование, резервные копии, импорт; без Tauri, с тестами |
| `src-tauri` | команды Tauri, сессия, автоблокировка, буфер обмена, журнал |
| `src` | интерфейс на Svelte 5 + TypeScript |
| `tests/ui` | тесты интерфейса (Playwright) |
| `docs` | документация, демо и сценарий его записи |

Главные решения:

- **Секреты расшифровываются только в Rust.** Интерфейс получает записи без значений
  секретов и запрашивает значение отдельной командой, когда его показывают или копируют.
- **Формат хранит исходный текст** каждого куска файла: неизменённые записи выводятся как
  были, поэтому ручные правки в файле не теряются.
- **Запись атомарная** (временный файл и переименование) и проверяет, что файл не меняли
  вне приложения.

Подробнее о защите — в [SECURITY.md](SECURITY.md).

## Выпуск

1. Обновить версию в `package.json`, `Cargo.toml` и `src-tauri/tauri.conf.json` и записать
   изменения в [CHANGELOG.md](../CHANGELOG.md).
2. Пройти [ручную проверку](manual-check.md).
3. Поставить тег и отправить его:

   ```sh
   git tag v0.1.0
   git push origin v0.1.0
   ```

GitHub Actions (`.github/workflows/release.yml`) соберёт установщики для Windows, macOS,
Linux и APK для Android и создаст черновик выпуска. Останется проверить файлы и
опубликовать его.

APK подписывается ключом из секретов репозитория: `ANDROID_KEYSTORE` (файл `.jks` в
base64), `ANDROID_KEYSTORE_PASSWORD` и `ANDROID_KEY_ALIAS`. Ключ один на всё время жизни
приложения: APK с другим ключом не встанет поверх установленного. Храните файл ключа и
пароль в надёжном месте, в репозиторий они не попадают.

## Android

Проект Android лежит в `src-tauri/gen/android`. Для сборки нужны Android SDK с NDK и Java 17
или новее (подойдёт встроенная в Android Studio), а также цели Rust:

```sh
rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android
```

Переменные для сборки (PowerShell):

```powershell
$sdk = "$env:LOCALAPPDATA\Android\Sdk"
$env:JAVA_HOME = 'C:\Program Files\Android\Android Studio\jbr'
$env:ANDROID_HOME = $sdk
$env:NDK_HOME = "$sdk\ndk\<версия>"
```

Запуск на эмуляторе или подключённом телефоне с обновлением интерфейса на ходу:

```sh
npm run tauri android dev
```

Отладочный APK для эмулятора x86_64 и его установка:

```powershell
npm run tauri android build -- --debug --apk --target x86_64
& "$sdk\platform-tools\adb.exe" install -r src-tauri\gen\android\app\build\outputs\apk\universal\debug\app-universal-debug.apk
```

На телефоне хранилище лежит в личной папке приложения. Файл, выбранный в системном
диалоге, приложение копирует туда же. Токены облаков — в файле в той же папке.

Иконка для Android — `src-tauri/icons/app-icon-android.svg`: ключ меньше, чтобы круглая
маска лаунчера его не срезала. Команда `tauri icon` перезаписывает и иконки для компьютеров,
поэтому иконки собирают во временную папку и переносят только папки `mipmap-*` из
`<папка>/android` в `src-tauri/gen/android/app/src/main/res`:

```sh
npm run tauri icon src-tauri/icons/app-icon-android.svg -- -o <папка>
```
