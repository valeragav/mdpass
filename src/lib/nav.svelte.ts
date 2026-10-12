// Какой экран сейчас открыт и с каким файлом хранилища работаем.
// Экраны соответствуют артбордам на холсте дизайна.
import type { VaultFile } from "./api";

export type Screen =
  | "loading"
  | "welcome"
  | "unlock"
  | "file-error"
  | "main"
  | "new-entry"
  | "edit-entry"
  | "import"
  | "settings"
  | "backups";

interface AppState {
  screen: Screen;
  /// Файл, который открываем или уже открыли.
  vault: VaultFile | null;
  /// Куда по умолчанию положить новое хранилище.
  defaultVault: VaultFile | null;
  /// Техническая причина, по которой файл не открылся.
  fileErrorDetail: string;
}

export const app = $state<AppState>({ screen: "loading", vault: null, defaultVault: null, fileErrorDetail: "" });

export function go(screen: Screen) {
  app.screen = screen;
}
