// Все горячие клавиши приложения для вкладки настроек. Те же, что в docs/guide.md.
// `keys` — равноценные варианты, в варианте клавиши через «+»: ["Ctrl+C", "Enter"].

export interface Shortcut {
  keys: string[];
  what: string;
}

export interface ShortcutGroup {
  title: string;
  items: Shortcut[];
}

export const SHORTCUTS: ShortcutGroup[] = [
  {
    title: "Главное окно",
    items: [
      { keys: ["Ctrl+F"], what: "Поиск" },
      { keys: ["Ctrl+N"], what: "Новая запись" },
      { keys: ["Ctrl+L"], what: "Заблокировать хранилище" },
    ],
  },
  {
    title: "Список записей слева",
    items: [
      { keys: ["↑", "↓"], what: "Выбрать запись или раздел. Избранное идёт первым" },
      { keys: ["Home", "End"], what: "Первая или последняя строка" },
      { keys: ["→", "Enter"], what: "Перейти к полям записи или в список раздела справа" },
      { keys: ["←"], what: "Свернуть раздел или перейти к разделу выше; из избранного — к его заголовку" },
      { keys: ["→", "←", "Enter"], what: "На заголовке «Избранное» — раскрыть или свернуть длинный список" },
      { keys: ["Alt+↑", "Alt+↓"], what: "Перенести избранную запись выше или ниже" },
      { keys: ["↑"], what: "С первой строки — в поиск; ↓ из поиска — обратно" },
    ],
  },
  {
    title: "Запись и список раздела справа",
    items: [
      { keys: ["↑", "↓"], what: "Выбрать поле, подраздел или запись" },
      { keys: ["Home", "End"], what: "Первая или последняя строка" },
      { keys: ["Ctrl+C", "Enter"], what: "Скопировать поле" },
      { keys: ["Пробел"], what: "Показать или скрыть секрет" },
      { keys: ["Enter", "→"], what: "Открыть подраздел или запись" },
      { keys: ["←", "Esc"], what: "Вернуться в список слева" },
    ],
  },
  {
    title: "Редактор записи",
    items: [
      { keys: ["Ctrl+S"], what: "Сохранить" },
      { keys: ["Alt+↑", "Alt+↓"], what: "Перенести поле выше или ниже" },
      { keys: ["Esc"], what: "Назад, с вопросом, если есть правки" },
    ],
  },
];

/// На macOS вместо Ctrl — Cmd.
export const IS_MAC = typeof navigator !== "undefined" && /Mac|iPhone|iPad/.test(navigator.platform);

export function keyLabel(key: string): string {
  return IS_MAC ? key.replace(/^Ctrl\+/, "⌘").replace(/^Alt\+/, "⌥") : key;
}
