// Какой экран сейчас открыт. Экраны соответствуют артбордам на холсте дизайна.
export type Screen =
  | "welcome"
  | "unlock"
  | "file-error"
  | "main"
  | "new-entry"
  | "edit-entry"
  | "import"
  | "settings"
  | "backups";

export const nav = $state<{ screen: Screen }>({ screen: "unlock" });

export function go(screen: Screen) {
  nav.screen = screen;
}
