// Записи открытого хранилища. Секретные значения здесь появляются только после
// «показать» и стираются при блокировке.
import { api, type EntryView } from "./api";
import { go } from "./nav.svelte";
import { showToast } from "./toast.svelte";

interface VaultState {
  entries: EntryView[];
  selectedId: number | null;
  /// Раздел, на который нажали в списке. Новая запись создаётся в нём.
  /// `null` — берётся раздел выбранной записи.
  selectedSection: string[] | null;
  /// Показанные значения секретов: ключ `${entryId}:${индекс поля}`.
  revealed: Record<string, string>;
}

export const vault = $state<VaultState>({ entries: [], selectedId: null, selectedSection: null, revealed: {} });

/// Раздел для новой записи: выбранный в списке или раздел выбранной записи.
export function currentSection(): string[] {
  if (vault.selectedSection) return vault.selectedSection;
  const entry = vault.entries.find((e) => e.id === vault.selectedId) ?? vault.entries.at(-1);
  return entry?.path ?? [];
}

export const revealKey = (entryId: number, field: number) => `${entryId}:${field}`;

export async function loadEntries() {
  vault.entries = await api.entries();
  const section = vault.selectedSection;
  if (section && !vault.entries.some((e) => section.every((part, i) => e.path[i] === part))) {
    vault.selectedSection = null;
  }
  if (!vault.entries.some((e) => e.id === vault.selectedId)) {
    vault.selectedId = vault.entries[0]?.id ?? null;
  }
}

export async function toggleReveal(entryId: number, field: number) {
  const key = revealKey(entryId, field);
  if (key in vault.revealed) {
    delete vault.revealed[key];
  } else {
    vault.revealed[key] = await api.revealField(entryId, field);
  }
}

export async function deleteEntry(entryId: number, title: string) {
  await api.deleteEntry(entryId);
  for (const key of Object.keys(vault.revealed)) if (key.startsWith(`${entryId}:`)) delete vault.revealed[key];
  await loadEntries();
  showToast(`Запись «${title}» удалена`);
}

/// Стирает записи из памяти интерфейса и показывает экран разблокировки.
export function forgetEntries() {
  vault.entries = [];
  vault.revealed = {};
  vault.selectedId = null;
  vault.selectedSection = null;
  go("unlock");
}

export async function lockVault() {
  await api.lock();
  forgetEntries();
}

/// Поля-заметки показываются отдельно от остальных полей.
export const isNote = (key: string) => /^note\d*$/i.test(key);

/// Подсказка в списке записей: ключ первого обычного поля, например «ssh» или «url».
export function entryHint(entry: EntryView): string {
  return entry.fields.find((f) => !f.secret && !isNote(f.key))?.key ?? "";
}

/// Подходит ли запись под поиск и тег. Секретные значения в поиске не участвуют.
export function matches(entry: EntryView, query: string, tag: string): boolean {
  if (tag && !entry.tags.includes(tag)) return false;
  const q = query.trim().toLowerCase();
  if (!q) return true;
  const haystack = [
    entry.path.join(" "),
    entry.title,
    entry.tags.join(" "),
    ...entry.fields.filter((f) => !f.secret).map((f) => `${f.key} ${f.value ?? ""}`),
  ]
    .join(" ")
    .toLowerCase();
  return haystack.includes(q);
}
