// Команды Rust-части. Типы повторяют `src-tauri/src/commands.rs`.
import { invoke } from "@tauri-apps/api/core";
import { open, save } from "@tauri-apps/plugin-dialog";

export interface VaultFile {
  path: string;
  display: string;
  name: string;
  exists: boolean;
  /// Зашифрован ли файл. `null`, если файла нет.
  encrypted: boolean | null;
}

export type Protection = "strong" | "weak" | "none";

export interface Settings {
  /// 0 — не блокировать.
  autoLockMinutes: number;
  /// 0 — не очищать.
  clipboardClearSeconds: number;
  lockOnMinimize: boolean;
  backupsEnabled: boolean;
  backupsToKeep: number;
  theme: "dark" | "light" | "system";
  textSize: "s" | "m" | "l";
  accent: string;
}

export interface Status {
  unlocked: boolean;
  lastVault: VaultFile | null;
  /// Недавние хранилища, последнее — первым.
  recentVaults: VaultFile[];
  defaultVault: VaultFile | null;
  /// Защита открытого хранилища. `null`, пока оно заблокировано.
  protection: Protection | null;
  settings: Settings;
}

export type CommandError =
  | { kind: "wrongPassphrase"; retryInMs: number }
  | { kind: "tooSoon"; retryInMs: number }
  | { kind: "damaged"; detail: string }
  | { kind: "notFound" }
  | { kind: "alreadyExists" }
  | { kind: "emptyPassphrase" }
  | { kind: "passphraseRequired" }
  | { kind: "changedOnDisk" }
  | { kind: "locked" }
  | { kind: "entryNotFound" }
  | { kind: "clipboard"; message: string }
  | { kind: "invalid"; message: string }
  | { kind: "otherPassphrase" }
  | { kind: "io"; message: string };

export interface FieldView {
  key: string;
  /// У секретного поля значения нет: его нужно запросить отдельно.
  value: string | null;
  secret: boolean;
}

export interface EntryView {
  id: number;
  path: string[];
  title: string;
  tags: string[];
  /// Строка `favorite: …`: запись в списке «Избранное».
  favorite: boolean;
  /// Номер в избранном из строки `favorite: 2`. `null` — `favorite: true`, без номера.
  favoriteOrder: number | null;
  fields: FieldView[];
}

export interface FieldInput {
  key: string;
  value: string | null;
  secret: boolean;
  /// Оставить значение поля с этим номером из текущей записи.
  keepFrom: number | null;
}

export interface DraftInput {
  path: string[];
  title: string;
  tags: string[];
  favorite: boolean;
  fields: FieldInput[];
}

export type BackupKind = "add" | "update" | "delete" | "passphrase" | "restore" | "import" | "move";

export interface BackupView {
  id: string;
  createdMs: number;
  kind: BackupKind;
}

export type BackupChange =
  | { kind: "added"; title: string }
  | { kind: "removed"; title: string }
  | { kind: "changed"; title: string; keys: string[] };

export interface ImportFixes {
  legacyTags: boolean;
  duplicateNotes: boolean;
  emptySections: boolean;
  missingSpace: boolean;
}

export interface ImportIssue {
  kind: "legacyTags" | "duplicateNotes" | "emptySections" | "missingSpace";
  titles: string[];
}

export interface ImportPreview {
  entries: number;
  sections: number;
  fields: number;
  tags: number;
  issues: ImportIssue[];
  places: { path: string[]; title: string }[];
}

/// Вызывается, когда команда наткнулась на файл, изменённый вне приложения.
let onChangedOnDisk: (() => void) | null = null;
export function whenChangedOnDisk(handler: () => void) {
  onChangedOnDisk = handler;
}

async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(command, args);
  } catch (err) {
    if (isCommandError(err) && err.kind === "changedOnDisk") onChangedOnDisk?.();
    throw err;
  }
}

export function isCommandError(err: unknown): err is CommandError {
  return typeof err === "object" && err !== null && "kind" in err;
}

/// Текст ошибки для пользователя.
export function errorText(err: unknown): string {
  if (!isCommandError(err)) return String(err);
  switch (err.kind) {
    case "wrongPassphrase":
      return "Неверный мастер-пароль";
    case "tooSoon":
      return "Подождите перед следующей попыткой";
    case "damaged":
      return "Файл повреждён или изменён вне приложения";
    case "notFound":
      return "Файл не найден";
    case "alreadyExists":
      return "Такой файл уже есть. Выберите другое имя или откройте его";
    case "emptyPassphrase":
      return "Мастер-пароль не может быть пустым";
    case "passphraseRequired":
      return "Файл зашифрован: введите мастер-пароль";
    case "changedOnDisk":
      return "Файл изменён вне приложения. Изменение не записано";
    case "locked":
      return "Хранилище заблокировано";
    case "entryNotFound":
      return "Запись не найдена";
    case "clipboard":
      return `Не удалось скопировать: ${err.message}`;
    case "invalid":
      return err.message;
    case "otherPassphrase":
      return "Копия зашифрована прежним мастер-паролем: она сделана до его смены";
    case "io":
      return `Ошибка доступа к файлу: ${err.message}`;
  }
}

const VAULT_FILTERS = [{ name: "Хранилище mdpass", extensions: ["age", "md"] }];

export const api = {
  status: () => invoke<Status>("status"),
  describePath: (path: string) => invoke<VaultFile>("describe_path", { path }),
  createVault: (path: string, passphrase: string | null, protection: Protection) =>
    invoke<Protection>("create_vault", { path, passphrase, protection }),
  /// `passphrase: null` — открыть хранилище без шифрования.
  unlock: (path: string, passphrase: string | null) => invoke<Protection>("unlock", { path, passphrase }),
  lock: () => invoke<void>("lock"),
  touch: () => invoke<void>("touch"),
  reloadVault: () => invoke<void>("reload_vault"),
  /// Новое имя файла в той же папке, расширение подставится по защите.
  renameVault: (name: string) => invoke<VaultFile>("rename_vault", { name }),
  /// Файл заодно переименовывается под защиту: `vault.md.age` или `vault.md`.
  changeProtection: (current: string | null, next: string | null, protection: Protection) =>
    call<{ protection: Protection; vault: VaultFile }>("change_protection", { current, new: next, protection }),
  updateSettings: (settings: Settings) => invoke<Settings>("update_settings", { settings }),
  /// `path` — для файла, который не открывается; иначе копии открытого хранилища.
  listBackups: (path?: string) => invoke<BackupView[]>("list_backups", { path }),
  /// Папка копий открытого хранилища в папке приложения.
  backupsLocation: () => invoke<string>("backups_location"),
  previewBackup: (id: string) => invoke<BackupChange[]>("preview_backup", { id }),
  restoreBackup: (id: string) => call<void>("restore_backup", { id }),
  restoreDamagedVault: (path: string, id: string, passphrase: string | null) =>
    invoke<void>("restore_damaged_vault", { path, id, passphrase }),
  async exportBackup(id: string, path?: string): Promise<boolean> {
    const to = await save({ defaultPath: id, filters: VAULT_FILTERS, title: "Сохранить копию как файл" });
    if (!to) return false;
    await invoke<void>("export_backup", { path, id, to });
    return true;
  },
  entries: () => invoke<EntryView[]>("entries"),
  revealField: (entryId: number, field: number) => invoke<string>("reveal_field", { entryId, field }),
  copyField: (entryId: number, field: number) => invoke<void>("copy_field", { entryId, field }),
  addEntry: (draft: DraftInput) => call<number>("add_entry", { draft }),
  updateEntry: (entryId: number, draft: DraftInput) => call<void>("update_entry", { entryId, draft }),
  deleteEntry: (entryId: number) => call<void>("delete_entry", { entryId }),
  setFavorite: (entryId: number, favorite: boolean) => call<void>("set_favorite", { entryId, favorite }),
  /// Записи получают номера в избранном 1, 2, 3… в этом порядке.
  orderFavorites: (entryIds: number[]) => call<void>("order_favorites", { entryIds }),
  generatePassword: (length?: number) => invoke<string>("generate_password", { length }),
  moveEntry: (entryId: number, path: string[], beforeId: number | null) =>
    call<void>("move_entry", { entryId, path, beforeId }),
  moveSection: (path: string[], parent: string[], before: string[] | null) =>
    call<void>("move_section", { path, parent, before }),

  analyzeImport: (path: string, fixes: ImportFixes) => invoke<ImportPreview>("analyze_import", { path, fixes }),
  runImport: (path: string, fixes: ImportFixes) => call<number>("run_import", { path, fixes }),
  deleteImportSource: (path: string) => invoke<void>("delete_import_source", { path }),

  /// Диалог выбора открытого .md-файла для импорта.
  async pickMarkdown(): Promise<VaultFile | null> {
    const path = await open({
      multiple: false,
      directory: false,
      filters: [{ name: "Markdown", extensions: ["md", "markdown", "txt"] }],
      title: "Файл с паролями для импорта",
    });
    return typeof path === "string" ? api.describePath(path) : null;
  },

  /// Диалог выбора места для нового хранилища.
  async pickNewVault(defaultPath?: string): Promise<VaultFile | null> {
    const path = await save({ defaultPath, filters: VAULT_FILTERS, title: "Где создать хранилище" });
    return path ? api.describePath(path) : null;
  },

  /// Диалог выбора существующего хранилища.
  async pickExistingVault(): Promise<VaultFile | null> {
    const path = await open({ multiple: false, directory: false, filters: VAULT_FILTERS, title: "Открыть хранилище" });
    return typeof path === "string" ? api.describePath(path) : null;
  },
};
