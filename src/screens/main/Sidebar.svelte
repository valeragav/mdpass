<script lang="ts" module>
  import { SvelteSet } from "svelte/reactivity";

  // Свёрнутые разделы. Живут только в памяти: названия разделов не должны
  // оставаться на диске в открытом виде.
  const collapsed = new SvelteSet<string>();
</script>

<script lang="ts">
  import { onDestroy } from "svelte";
  import { api, errorText, type EntryView } from "../../lib/api";
  import { go } from "../../lib/nav.svelte";
  import { showToast } from "../../lib/toast.svelte";
  import Button from "../../lib/ui/Button.svelte";
  import Icon from "../../lib/ui/Icon.svelte";
  import { entryHint, loadEntries, vault } from "../../lib/vault.svelte";

  interface Props {
    /// Записи после поиска и фильтра по тегу.
    visible: EntryView[];
    query: string;
    tag: string;
  }

  let { visible, query = $bindable(), tag = $bindable() }: Props = $props();

  type GroupRow = { kind: "group"; key: string; depth: number; label: string; path: string[] };
  type EntryRow = { kind: "entry"; key: string; depth: number; entry: EntryView };
  type Row = GroupRow | EntryRow;

  const pathKey = (path: string[]) => path.join("\u0000");
  const startsWith = (path: string[], prefix: string[]) => prefix.every((part, i) => path[i] === part);

  // Записи идут в порядке файла. Заголовок раздела ставим там, где путь меняется.
  let rows = $derived.by(() => {
    const out: Row[] = [];
    let last: string[] = [];
    for (const entry of visible) {
      entry.path.forEach((part, i) => {
        // Если раздел сменился на этом уровне, все более глубокие тоже начинаются заново.
        if (last[i] !== part) {
          const path = entry.path.slice(0, i + 1);
          out.push({ kind: "group", key: `g:${pathKey(path)}`, depth: i, label: part, path });
          last = [...last.slice(0, i), part];
        }
      });
      last = entry.path.slice();
      out.push({ kind: "entry", key: `e:${entry.id}`, depth: entry.path.length, entry });
    }
    return out;
  });

  // Во время поиска разделы раскрыты: иначе найденное могло бы оказаться внутри свёрнутого.
  let filtering = $derived(query.trim().length > 0 || tag.length > 0);

  function hiddenByCollapse(row: Row): boolean {
    if (filtering) return false;
    const path = row.kind === "group" ? row.path.slice(0, -1) : row.entry.path;
    return path.some((_, i) => collapsed.has(pathKey(path.slice(0, i + 1))));
  }

  let shown = $derived(rows.filter((row) => !hiddenByCollapse(row)));

  function toggle(group: GroupRow) {
    const key = pathKey(group.path);
    if (collapsed.has(key)) collapsed.delete(key);
    else collapsed.add(key);
  }

  let tagChips = $derived.by(() => {
    const counts = new Map<string, number>();
    for (const entry of vault.entries) for (const t of entry.tags) counts.set(t, (counts.get(t) ?? 0) + 1);
    return [...counts].map(([name, count]) => ({ name, count }));
  });

  let empty = $derived(vault.entries.length === 0);

  // ---- Перетаскивание ----------------------------------------------------
  // Сделано на событиях указателя: HTML5 drag-and-drop в окне Tauri на Windows
  // перехватывает само окно.

  type Dragged = { kind: "entry"; entry: EntryView } | { kind: "section"; path: string[] };
  type Target =
    | { kind: "into"; path: string[]; rowKey: string }
    | { kind: "before-entry"; entry: EntryView; rowKey: string }
    | { kind: "after-entry"; entry: EntryView; rowKey: string }
    | { kind: "before-section"; path: string[]; rowKey: string }
    | { kind: "root"; rowKey: "root" };

  const DRAG_THRESHOLD = 6;

  let list: HTMLElement | undefined = $state();
  let pending: { dragged: Dragged; label: string; x: number; y: number } | null = null;
  let dragging = $state<{ dragged: Dragged; label: string } | null>(null);
  let pointer = $state({ x: 0, y: 0 });
  let target = $state<Target | null>(null);
  let suppressClick = false;
  let scrollTimer: ReturnType<typeof setInterval> | undefined;

  function startPress(event: PointerEvent, dragged: Dragged, label: string) {
    if (event.button !== 0) return;
    pending = { dragged, label, x: event.clientX, y: event.clientY };
    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", onUp, { once: true });
    window.addEventListener("pointercancel", cancelDrag, { once: true });
  }

  function onMove(event: PointerEvent) {
    pointer = { x: event.clientX, y: event.clientY };
    if (!dragging && pending) {
      if (Math.hypot(event.clientX - pending.x, event.clientY - pending.y) < DRAG_THRESHOLD) return;
      dragging = { dragged: pending.dragged, label: pending.label };
      scrollTimer = setInterval(autoScroll, 16);
    }
    if (dragging) {
      event.preventDefault();
      target = findTarget(event.clientX, event.clientY);
    }
  }

  /// Список прокручивается, когда указатель у его верхнего или нижнего края.
  function autoScroll() {
    if (!list) return;
    const rect = list.getBoundingClientRect();
    const edge = 40;
    if (pointer.y < rect.top + edge) list.scrollTop -= 8;
    else if (pointer.y > rect.bottom - edge) list.scrollTop += 8;
  }

  function findTarget(x: number, y: number): Target | null {
    if (!dragging) return null;
    const el = document.elementFromPoint(x, y)?.closest<HTMLElement>("[data-drop]");
    if (!el) return null;
    if (el.dataset.drop === "root") return { kind: "root", rowKey: "root" };
    const row = rows.find((r) => r.key === el.dataset.drop);
    if (!row) return null;
    const rect = el.getBoundingClientRect();
    const offset = (y - rect.top) / rect.height;
    const dragged = dragging.dragged;

    if (dragged.kind === "entry") {
      if (row.kind === "group") return { kind: "into", path: row.path, rowKey: row.key };
      if (row.entry.id === dragged.entry.id) return null;
      return offset < 0.5
        ? { kind: "before-entry", entry: row.entry, rowKey: row.key }
        : { kind: "after-entry", entry: row.entry, rowKey: row.key };
    }

    // Раздел нельзя положить внутрь самого себя.
    const path = row.kind === "group" ? row.path : row.entry.path;
    if (startsWith(path, dragged.path)) return null;
    if (row.kind === "group" && offset < 0.3) return { kind: "before-section", path: row.path, rowKey: row.key };
    return { kind: "into", path, rowKey: row.key };
  }

  function onUp() {
    const done = dragging;
    const where = target;
    cancelDrag();
    if (done) {
      // Клик, который браузер пришлёт после отпускания, не должен выбрать запись.
      suppressClick = true;
      setTimeout(() => (suppressClick = false), 0);
      if (where) drop(done.dragged, where);
    }
  }

  function cancelDrag() {
    window.removeEventListener("pointermove", onMove);
    window.removeEventListener("pointerup", onUp);
    window.removeEventListener("pointercancel", cancelDrag);
    clearInterval(scrollTimer);
    pending = null;
    dragging = null;
    target = null;
  }

  onDestroy(cancelDrag);

  /// Запись, перед которой встанет перенесённая, если бросить её после `entry`.
  function nextInSection(entry: EntryView, skip: number): number | null {
    const same = vault.entries.filter((e) => pathKey(e.path) === pathKey(entry.path) && e.id !== skip);
    const at = same.findIndex((e) => e.id === entry.id);
    return same[at + 1]?.id ?? null;
  }

  async function drop(dragged: Dragged, where: Target) {
    try {
      if (dragged.kind === "entry") {
        const id = dragged.entry.id;
        if (where.kind === "into") await api.moveEntry(id, where.path, null);
        else if (where.kind === "root") await api.moveEntry(id, [], null);
        else if (where.kind === "before-entry") await api.moveEntry(id, where.entry.path, where.entry.id);
        else if (where.kind === "after-entry") await api.moveEntry(id, where.entry.path, nextInSection(where.entry, id));
        else return;
        if (where.kind === "into") collapsed.delete(pathKey(where.path));
      } else {
        const path = dragged.path;
        if (where.kind === "into") {
          await api.moveSection(path, where.path, null);
          collapsed.delete(pathKey(where.path));
        } else if (where.kind === "root") await api.moveSection(path, [], null);
        else if (where.kind === "before-section") await api.moveSection(path, where.path.slice(0, -1), where.path);
        else return;
      }
      await loadEntries();
      showToast("Перемещено");
    } catch (err) {
      showToast(errorText(err));
    }
  }

  function dropClass(rowKey: string): string {
    if (!target || target.rowKey !== rowKey) return "";
    if (target.kind === "before-entry" || target.kind === "before-section") return "drop-before";
    if (target.kind === "after-entry") return "drop-after";
    return "drop-into";
  }

  function isDragged(row: Row): boolean {
    const d = dragging?.dragged;
    if (!d) return false;
    if (d.kind === "entry") return row.kind === "entry" && row.entry.id === d.entry.id;
    const path = row.kind === "group" ? row.path : row.entry.path;
    return startsWith(path, d.path);
  }
</script>

<aside class:dragging={dragging !== null}>
  <div class="top">
    <div class="search">
      <Icon name="search" color="var(--text-subtle)" />
      <input
        type="search"
        bind:value={query}
        placeholder="Поиск по названию и тегам"
        aria-label="Поиск"
        disabled={empty}
        spellcheck="false"
      />
    </div>
    {#if tagChips.length > 0}
      <div class="tags">
        {#each tagChips as chip (chip.name)}
          <button
            type="button"
            class="chip mono"
            class:on={tag === chip.name}
            aria-pressed={tag === chip.name}
            onclick={() => (tag = tag === chip.name ? "" : chip.name)}
          >
            <span>[{chip.name}]</span>
            <span class="count">{chip.count}</span>
          </button>
        {/each}
      </div>
    {/if}
  </div>

  <nav class="list scroll" aria-label="Записи" bind:this={list}>
    {#if empty}
      <p class="none">Записей пока нет</p>
    {:else if visible.length === 0}
      <p class="none">Ничего не найдено</p>
    {/if}
    {#each shown as row (row.key)}
      {#if row.kind === "group"}
        {@const open = filtering || !collapsed.has(pathKey(row.path))}
        <button
          type="button"
          class="group mono {dropClass(row.key)}"
          class:faded={isDragged(row)}
          class:on={vault.selectedSection !== null && pathKey(vault.selectedSection) === pathKey(row.path)}
          data-drop={row.key}
          aria-expanded={open}
          style:padding-left="{6 + row.depth * 16}px"
          onpointerdown={(e) => startPress(e, { kind: "section", path: row.path }, row.label)}
          onclick={() => {
            if (suppressClick) return;
            vault.selectedSection = row.path;
            toggle(row);
          }}
        >
          <span class="chevron" class:open aria-hidden="true"><Icon name="chevron" size={14} stroke={2} /></span>
          <span class="label">{row.label}</span>
        </button>
      {:else}
        <button
          type="button"
          class="entry {dropClass(row.key)}"
          class:on={row.entry.id === vault.selectedId}
          class:faded={isDragged(row)}
          data-drop={row.key}
          aria-current={row.entry.id === vault.selectedId ? "true" : undefined}
          style:padding-left="{24 + row.depth * 16}px"
          onpointerdown={(e) => startPress(e, { kind: "entry", entry: row.entry }, row.entry.title)}
          onclick={() => {
            if (suppressClick) return;
            vault.selectedId = row.entry.id;
            vault.selectedSection = null;
          }}
        >
          <span class="title">{row.entry.title}</span>
          <span class="hint mono">{entryHint(row.entry)}</span>
        </button>
      {/if}
    {/each}
    {#if dragging}
      <div class="root-drop mono {dropClass('root')}" data-drop="root">Перетащите сюда, чтобы перенести на верхний уровень</div>
    {/if}
  </nav>

  <div class="bottom">
    <Button variant="primary" size="sm" icon="plus" full onclick={() => go("new-entry")}>Новая запись</Button>
  </div>
</aside>

{#if dragging}
  <div class="ghost" style:left="{pointer.x + 14}px" style:top="{pointer.y + 10}px" aria-hidden="true">
    {dragging.dragged.kind === "section" ? "# " : ""}{dragging.label}
  </div>
{/if}

<style>
  aside {
    width: 320px;
    flex: none;
    display: flex;
    flex-direction: column;
    min-height: 0;
    background: var(--bg-chrome);
    border-right: 1px solid var(--line);
  }

  aside.dragging {
    cursor: grabbing;
    user-select: none;
  }

  .top {
    padding: 16px 16px 12px;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .search {
    position: relative;
    display: flex;
    align-items: center;
  }

  .search :global(svg) {
    position: absolute;
    left: 12px;
  }

  input {
    width: 100%;
    height: var(--control-h-sm);
    padding: 0 12px 0 36px;
    border-radius: var(--radius);
    border: 1px solid var(--line-strong);
    background: var(--bg-panel);
    color: var(--text);
  }

  input:disabled {
    opacity: 0.6;
  }

  .tags {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }

  .chip {
    height: 28px;
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 0 10px;
    border-radius: 6px;
    border: 1px solid var(--line-strong);
    font-size: 12px;
    color: var(--text-muted);
  }

  .chip:hover {
    background: var(--bg-hover-strong);
  }

  .chip.on {
    border-color: var(--accent);
    background: color-mix(in srgb, var(--accent) 12%, transparent);
    color: var(--accent);
  }

  .count {
    color: var(--text-subtle);
  }

  .list {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: 0 8px 8px;
  }

  .none {
    padding: 24px 12px;
    color: var(--text-subtle);
  }

  .group {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 6px;
    margin-top: 8px;
    min-height: 32px;
    padding-right: 10px;
    border-radius: 6px;
    font-size: 12px;
    color: var(--text-muted);
  }

  .group:hover {
    background: var(--bg-hover);
  }

  /* Выбранный раздел: в нём создастся новая запись. */
  .group.on {
    color: var(--accent);
  }

  .chevron {
    width: 14px;
    height: 14px;
    display: inline-flex;
    color: var(--text-faint);
    transition: transform 0.12s;
  }

  .chevron.open {
    transform: rotate(90deg);
  }

  .label {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .entry {
    width: 100%;
    height: 36px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    padding-right: 10px;
    border-radius: 6px;
    color: var(--text-2);
  }

  .entry:hover {
    background: var(--bg-hover);
  }

  .entry.on {
    background: color-mix(in srgb, var(--accent) 12%, transparent);
    color: var(--text-strong);
  }

  .faded {
    opacity: 0.4;
  }

  /* Куда встанет перетаскиваемое: линия над строкой, под ней или подсветка раздела. */
  .drop-before {
    box-shadow: inset 0 2px 0 var(--accent);
  }

  .drop-after {
    box-shadow: inset 0 -2px 0 var(--accent);
  }

  .drop-into {
    background: color-mix(in srgb, var(--accent) 18%, transparent);
    outline: 1px solid var(--accent);
    outline-offset: -1px;
  }

  .title {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .hint {
    flex: none;
    font-size: 11px;
    color: var(--text-subtle);
  }

  .root-drop {
    margin-top: 12px;
    padding: 14px 12px;
    border: 1px dashed var(--line-dashed);
    border-radius: 8px;
    font-size: 12px;
    color: var(--text-subtle);
    text-align: center;
  }

  .ghost {
    position: fixed;
    z-index: 20;
    max-width: 260px;
    padding: 6px 12px;
    border-radius: 6px;
    background: var(--bg-panel);
    border: 1px solid var(--accent);
    color: var(--text);
    font-size: 13px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    pointer-events: none;
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.35);
  }

  .bottom {
    padding: 12px 16px 16px;
    border-top: 1px solid var(--line);
  }
</style>
