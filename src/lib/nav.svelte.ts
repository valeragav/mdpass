// Какой экран сейчас открыт и с каким файлом хранилища работаем.
// Экраны соответствуют артбордам на холсте дизайна.
import type { Settings, VaultFile } from "./api";

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
  settings: Settings;
  /// Куда вернуться с экрана резервных копий.
  backupsFrom: Screen;
  /// Откуда пришли на импорт: с первого запуска это «шаг 2 из 2».
  importFrom: "welcome" | "main";
}

export const app = $state<AppState>({
  screen: "loading",
  vault: null,
  defaultVault: null,
  fileErrorDetail: "",
  backupsFrom: "main",
  importFrom: "main",
  // До ответа Rust действуют значения по умолчанию из дизайна.
  settings: {
    autoLockMinutes: 5,
    clipboardClearSeconds: 30,
    lockOnMinimize: true,
    backupsEnabled: true,
    backupsToKeep: 30,
    theme: "dark",
    textSize: "m",
    accent: "#E5A93D",
  },
});

export function go(screen: Screen) {
  app.screen = screen;
}
