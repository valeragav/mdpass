// Действия с полями записи: показать, скопировать, управление стрелками. Общие для
// карточки записи и списка записей раздела.
import { api, errorText, type EntryView } from "./api";
import { isNavKey, keyNav, scrollToCursor } from "./keys.svelte";
import { app } from "./nav.svelte";
import { showToast } from "./toast.svelte";
import { revealKey, toggleReveal, vault } from "./vault.svelte";

export const MASK = "••••••••••••";

/// Поле одной из записей. Индекс — номер поля в записи: по нему Rust находит значение секрета.
export interface FieldRef {
  entry: EntryView;
  index: number;
}

/// Значение для показа: секрет — только после «показать», иначе `null`.
export function shownValue(entry: EntryView, index: number): string | null {
  const field = entry.fields[index];
  if (!field.secret) return field.value;
  return vault.revealed[revealKey(entry.id, index)] ?? null;
}

export async function revealField(entry: EntryView, index: number) {
  try {
    await toggleReveal(entry.id, index);
  } catch (err) {
    showToast(errorText(err));
  }
}

export async function copyField(entry: EntryView, index: number) {
  const field = entry.fields[index];
  try {
    await api.copyField(entry.id, index);
    const clear = app.settings.clipboardClearSeconds;
    const after = clear > 0 ? ` · буфер очистится через ${clear} с` : "";
    // Поле ssh Rust копирует готовой командой «ssh user@host».
    const what = field.secret
      ? "Секрет скопирован"
      : field.key.toLowerCase() === "ssh"
        ? "Скопирована команда ssh"
        : `Скопировано: ${field.key}`;
    showToast(what + after);
  } catch (err) {
    showToast(errorText(err));
  }
}

/// Строка под курсором: поле записи или строка-ссылка (подраздел, название записи),
/// которую Enter или → открывает.
export type CursorItem = FieldRef | { open: () => void };

const isField = (item: CursorItem | undefined): item is FieldRef => item !== undefined && "entry" in item;

/// Номер поля под курсором среди `count` полей на экране или -1, если курсор в списке слева.
export function fieldCursor(count: number): number {
  return keyNav.zone === "fields" && count > 0 ? Math.min(keyNav.field, count - 1) : -1;
}

export function pickField(pos: number) {
  keyNav.zone = "fields";
  keyNav.field = pos;
}

/// ↑ ↓ — по строкам, ← или Esc — назад в список, Ctrl+C или Enter — скопировать поле,
/// пробел — показать или скрыть секрет, Enter или → на ссылке — открыть её.
/// Строки помечены `data-cursor` с номером.
/// Открыть ссылку и поставить курсор на первую строку того, что открылось.
function open(item: { open: () => void }) {
  item.open();
  keyNav.field = 0;
}

export function onFieldKey(event: KeyboardEvent, order: CursorItem[], root: HTMLElement | undefined) {
  if (keyNav.zone !== "fields" || !isNavKey(event)) return;
  const cursor = fieldCursor(order.length);
  const current = cursor >= 0 ? order[cursor] : undefined;
  const move = (to: number) => {
    keyNav.field = Math.max(0, Math.min(to, order.length - 1));
    scrollToCursor(() => root?.querySelector(`[data-cursor="${keyNav.field}"]`));
  };

  if (event.ctrlKey || event.metaKey) {
    const key = event.key.toLowerCase();
    // Выделенный мышью текст Ctrl+C копирует как обычно.
    if ((key === "c" || key === "с") && isField(current) && !window.getSelection()?.toString()) {
      event.preventDefault();
      copyField(current.entry, current.index);
    }
    return;
  }
  if (event.shiftKey) return;
  switch (event.key) {
    case "ArrowDown":
      move(cursor + 1);
      break;
    case "ArrowUp":
      move(cursor - 1);
      break;
    case "Home":
      move(0);
      break;
    case "End":
      move(order.length - 1);
      break;
    case "ArrowLeft":
    case "Escape":
      keyNav.zone = "list";
      break;
    case "Enter":
      if (!current) return;
      if (isField(current)) copyField(current.entry, current.index);
      else open(current);
      break;
    case "ArrowRight":
      if (!current || isField(current)) return;
      open(current);
      break;
    case " ":
      if (!isField(current) || !current.entry.fields[current.index].secret) return;
      revealField(current.entry, current.index);
      break;
    default:
      return;
  }
  event.preventDefault();
}
