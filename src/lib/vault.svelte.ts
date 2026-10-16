// Записи открытого хранилища. Секретные значения здесь появляются только после
// «показать» и стираются при блокировке, при перечитывании записей и при смене
// выбранной записи или раздела.
import { untrack } from "svelte";
import { SvelteSet } from "svelte/reactivity";
import { api, errorText, type EntryView } from "./api";
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

/// Записи, которые при последнем слиянии изменили на двух устройствах. Пока список
/// не пуст, окно просит проверить их.
export const merge = $state({ bothKept: [] as string[] });

/// Свёрнутые разделы. Живут только в памяти: названия разделов не должны оставаться
/// на диске в открытом виде. Забываются при блокировке.
export const collapsed = new SvelteSet<string>();

/// Номер «поколения» показанных секретов. Растёт при каждом сбросе: ответ на
/// «показать», пришедший после сброса, относится к старым данным и отбрасывается.
let revealEpoch = 0;

function hideSecrets() {
  revealEpoch++;
  vault.revealed = {};
}

// Показанный секрет не переходит на другую запись или раздел.
$effect.root(() => {
  $effect(() => {
    void vault.selectedId;
    void vault.selectedSection;
    untrack(hideSecrets);
  });
});

/// Раздел для новой записи: выбранный в списке или раздел выбранной записи.
export function currentSection(): string[] {
  if (vault.selectedSection) return vault.selectedSection;
  const entry = vault.entries.find((e) => e.id === vault.selectedId) ?? vault.entries.at(-1);
  return entry?.path ?? [];
}

export const revealKey = (entryId: number, field: number) => `${entryId}:${field}`;

const samePlace = (a: EntryView, b: EntryView) => a.title === b.title && a.path.join("\u0000") === b.path.join("\u0000");

/// Перечитывает записи. Номера записей Rust раздаёт заново, когда перечитывает файл
/// (после «Перечитать» или восстановления копии), поэтому показанные секреты, привязанные
/// к номерам, сбрасываются, а выбранная запись находится по разделу и названию.
export async function loadEntries() {
  const selected = vault.entries.find((e) => e.id === vault.selectedId);
  const entries = await api.entries();
  hideSecrets();
  vault.entries = entries;
  const notice = await api.takeMergeNotice();
  if (notice?.bothKept.length) merge.bothKept = notice.bothKept;
  const section = vault.selectedSection;
  if (section && !entries.some((e) => section.every((part, i) => e.path[i] === part))) {
    vault.selectedSection = null;
  }
  const sameId = entries.find((e) => e.id === vault.selectedId);
  if (!sameId || (selected && !samePlace(sameId, selected))) {
    vault.selectedId = (selected && entries.find((e) => samePlace(e, selected)))?.id ?? entries[0]?.id ?? null;
  }
}

export async function toggleReveal(entryId: number, field: number) {
  const key = revealKey(entryId, field);
  if (key in vault.revealed) {
    delete vault.revealed[key];
    return;
  }
  const epoch = revealEpoch;
  const value = await api.revealField(entryId, field);
  if (epoch === revealEpoch) vault.revealed[key] = value;
}

export async function deleteEntry(entryId: number, title: string) {
  await api.deleteEntry(entryId);
  for (const key of Object.keys(vault.revealed)) if (key.startsWith(`${entryId}:`)) delete vault.revealed[key];
  await loadEntries();
  showToast(`Запись «${title}» удалена`);
}

/// Избранное по порядку: сначала записи с номером, по возрастанию, потом без номера,
/// в порядке файла.
export function favoritesInOrder(entries: EntryView[]): EntryView[] {
  const rank = (e: EntryView) => e.favoriteOrder ?? Infinity;
  return entries.filter((e) => e.favorite).sort((a, b) => rank(a) - rank(b));
}

export async function toggleFavorite(entry: EntryView) {
  await api.setFavorite(entry.id, !entry.favorite);
  await loadEntries();
  showToast(entry.favorite ? `«${entry.title}» убрана из избранного` : `«${entry.title}» добавлена в избранное`);
}

/// Стирает записи из памяти интерфейса и показывает экран разблокировки.
export function forgetEntries() {
  vault.entries = [];
  merge.bothKept = [];
  hideSecrets();
  collapsed.clear();
  vault.selectedId = null;
  vault.selectedSection = null;
  go("unlock");
}

/// Блокирует хранилище. Записи из окна стираются, даже если Rust ответил ошибкой.
export async function lockVault() {
  try {
    await api.lock();
  } catch (err) {
    showToast(errorText(err));
  } finally {
    forgetEntries();
  }
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
