// Команды Rust-части. Типы повторяют `src-tauri/src/commands.rs`.
import { invoke } from "@tauri-apps/api/core";
import { open, save } from "@tauri-apps/plugin-dialog";

export interface VaultFile {
  path: string;
  display: string;
  name: string;
  exists: boolean;
}

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
  defaultVault: VaultFile | null;
  settings: Settings;
}

export type CommandError =
  | { kind: "wrongPassphrase"; retryInMs: number }
  | { kind: "tooSoon"; retryInMs: number }
  | { kind: "damaged"; detail: string }
  | { kind: "notFound" }
  | { kind: "alreadyExists" }
  | { kind: "emptyPassphrase" }
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
  fields: FieldInput[];
}

export type BackupKind = "add" | "update" | "delete" | "passphrase" | "restore" | "import";

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
    case "changedOnDisk":
      return "Файл изменён вне приложения. Изменения не записаны: заблокируйте хранилище и откройте его заново";
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

const VAULT_FILTERS = [{ name: "Хранилище mdpass", extensions: ["age"] }];

export const api = {
  status: () => invoke<Status>("status"),
  describePath: (path: string) => invoke<VaultFile>("describe_path", { path }),
  createVault: (path: string, passphrase: string) => invoke<void>("create_vault", { path, passphrase }),
  unlock: (path: string, passphrase: string) => invoke<void>("unlock", { path, passphrase }),
  lock: () => invoke<void>("lock"),
  touch: () => invoke<void>("touch"),
  changePassphrase: (current: string, next: string) =>
    invoke<void>("change_passphrase", { current, new: next }),
  updateSettings: (settings: Settings) => invoke<Settings>("update_settings", { settings }),
  /// `path` — для файла, который не открывается; иначе копии открытого хранилища.
  listBackups: (path?: string) => invoke<BackupView[]>("list_backups", { path }),
  previewBackup: (id: string) => invoke<BackupChange[]>("preview_backup", { id }),
  restoreBackup: (id: string) => invoke<void>("restore_backup", { id }),
  restoreDamagedVault: (path: string, id: string, passphrase: string) =>
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
  addEntry: (draft: DraftInput) => invoke<number>("add_entry", { draft }),
  updateEntry: (entryId: number, draft: DraftInput) => invoke<void>("update_entry", { entryId, draft }),
  deleteEntry: (entryId: number) => invoke<void>("delete_entry", { entryId }),
  generatePassword: (length?: number) => invoke<string>("generate_password", { length }),

  analyzeImport: (path: string, fixes: ImportFixes) => invoke<ImportPreview>("analyze_import", { path, fixes }),
  runImport: (path: string, fixes: ImportFixes) => invoke<number>("run_import", { path, fixes }),
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
