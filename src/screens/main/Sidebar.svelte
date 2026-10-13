<script lang="ts" module>
  import { SvelteSet } from "svelte/reactivity";

  // Свёрнутые разделы. Живут только в памяти: названия разделов не должны
  // оставаться на диске в открытом виде.
  const collapsed = new SvelteSet<string>();
  // Раскрыт ли длинный список избранного.
  let favoritesOpen = $state(false);
</script>

<script lang="ts">
  import { onDestroy } from "svelte";
  import { api, errorText, type EntryView } from "../../lib/api";
  import { app, go } from "../../lib/nav.svelte";
  import { showToast } from "../../lib/toast.svelte";
  import Button from "../../lib/ui/Button.svelte";
  import Icon from "../../lib/ui/Icon.svelte";
  import { isNavKey, keyNav, scrollToCursor } from "../../lib/keys.svelte";
  import { entryHint, favoritesInOrder, loadEntries, vault } from "../../lib/vault.svelte";

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
  // Строки избранного: заголовок (если список длинный) и записи.
  type FavHeadRow = { kind: "fav-head"; key: string };
  type FavRow = { kind: "fav"; key: string; entry: EntryView };
  // Кнопка «Свернуть все» над разделами.
  type FoldersRow = { kind: "folders"; key: string };
  type NavRow = Row | FavHeadRow | FavRow | FoldersRow;

  const pathKey = (path: string[]) => path.join("\u0000");
  const startsWith = (path: string[], prefix: string[]) => prefix.every((part, i) => path[i] === part);

  // Записи идут в порядке файла. Заголовок раздела ставим там, где путь меняется.
  let rows = $derived.by(() => {
    const out: Row[] = [];
    let last: string[] = [];
    for (const entry of visible) {
      entry.path.forEach((part, i) => {
        // Если раздел сменился на этом уровне, все более глубокие тоже начинаются заново.
        // Один и тот же раздел может встретиться в файле дважды, поэтому в ключе есть
        // номер первой записи после заголовка.
        if (last[i] !== part) {
          const path = entry.path.slice(0, i + 1);
          out.push({ kind: "group", key: `g:${entry.id}:${pathKey(path)}`, depth: i, label: part, path });
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

  // Избранное — над разделами, в порядке файла. Во время поиска не показывается:
  // найденное и так видно в списке.
  let favorites = $derived(filtering ? [] : favoritesInOrder(vault.entries));

  // Избранное прокручивается в своей области: в ней видно 5 строк, а если раскрыть
  // стрелкой у заголовка — 10. Высоты областей — в стилях `.fav-scroll`.
  const FAVORITES_SHOWN = 5;
  let favoritesLong = $derived(favorites.length > FAVORITES_SHOWN);
  let favBox: HTMLElement | undefined = $state();

  // Все разделы списка разом: свернуть или, если уже все свёрнуты, развернуть.
  let groupKeys = $derived([...new Set(rows.filter((r) => r.kind === "group").map((r) => pathKey(r.path)))]);
  let allCollapsed = $derived(groupKeys.length > 0 && groupKeys.every((key) => collapsed.has(key)));

  function toggleAll() {
    if (allCollapsed) collapsed.clear();
    else for (const key of groupKeys) collapsed.add(key);
  }

  // Все строки, по которым ходят стрелки: сначала избранное, потом «Свернуть все»,
  // разделы и записи. Во время поиска кнопка недоступна, и стрелки её пропускают.
  let navRows = $derived<NavRow[]>([
    ...(favoritesLong ? [{ kind: "fav-head", key: "fav-head" } as const] : []),
    ...favorites.map((entry) => ({ kind: "fav", key: `f:${entry.id}`, entry }) as const),
    ...(groupKeys.length > 0 && !filtering ? [{ kind: "folders", key: "folders" } as const] : []),
    ...shown,
  ]);

  // ---- Стрелки -----------------------------------------------------------
  // Курсор списка — это выбранная строка: запись или раздел. Избранная запись есть
  // и в избранном, и в разделе, поэтому отдельно помним, где из двух стоит курсор.

  let cursorIn = $state<"list" | "fav" | "fav-head" | "folders">("list");
  // Курсор на кнопке, а не на записи или разделе.
  let onButton = $derived(cursorIn === "fav-head" || cursorIn === "folders");

  // Курсор в избранном, только пока выбранная запись там видна. Иначе он в разделе.
  let favCursor = $derived(
    cursorIn === "fav" && vault.selectedSection === null && favorites.some((e) => e.id === vault.selectedId),
  );

  function isCurrent(row: NavRow): boolean {
    switch (row.kind) {
      case "fav-head":
      case "folders":
        return cursorIn === row.kind;
      case "fav":
        return favCursor && row.entry.id === vault.selectedId;
      case "group":
        return !onButton && vault.selectedSection !== null && pathKey(vault.selectedSection) === pathKey(row.path);
      case "entry":
        return !onButton && !favCursor && vault.selectedSection === null && row.entry.id === vault.selectedId;
    }
  }

  function select(row: NavRow | undefined) {
    if (!row) return;
    if (row.kind === "fav-head" || row.kind === "folders") cursorIn = row.kind;
    else if (row.kind === "group") {
      cursorIn = "list";
      vault.selectedSection = row.path;
    } else {
      cursorIn = row.kind === "fav" ? "fav" : "list";
      vault.selectedId = row.entry.id;
      vault.selectedSection = null;
    }
    // Иначе Enter и пробел нажали бы кнопку, на которой раньше щёлкнули мышью.
    if (document.activeElement instanceof HTMLElement && list?.contains(document.activeElement)) {
      document.activeElement.blur();
    }
    scrollToCursor(() => [...(list?.querySelectorAll<HTMLElement>("[data-nav]") ?? [])].find((el) => el.dataset.nav === row.key));
  }

  let search: HTMLInputElement | undefined = $state();

  /// ↓ в поиске возвращает в список: к выбранной строке или к первой.
  function onSearchKey(event: KeyboardEvent) {
    if (event.key !== "ArrowDown" || navRows.length === 0) return;
    event.preventDefault();
    search?.blur();
    keyNav.zone = "list";
    select(navRows.find(isCurrent) ?? navRows[0]);
  }

  function openFields() {
    keyNav.zone = "fields";
    keyNav.field = 0;
  }

  // ↑ ↓ — по строкам, → или Enter — перейти вправо: к полям записи или в список раздела,
  // ← — свернуть раздел или подняться к разделу. Избранное идёт первым: у его заголовка
  // → раскрывает весь список, ← сворачивает, Enter переключает. На «Свернуть все»
  // → разворачивает все разделы, ← сворачивает, Enter переключает.
  function onKey(event: KeyboardEvent) {
    if (moveFavoriteByKey(event)) return;
    if (keyNav.zone !== "list" || !isNavKey(event) || event.ctrlKey || event.metaKey || event.shiftKey) return;
    if (navRows.length === 0) return;
    const at = navRows.findIndex(isCurrent);
    const row = navRows[at];
    const open = row?.kind === "group" && (filtering || !collapsed.has(pathKey(row.path)));
    const parent = (): NavRow | undefined => {
      if (row.kind === "fav") return navRows.find((r) => r.kind === "fav-head");
      if (row.kind === "fav-head" || row.kind === "folders") return undefined;
      const path = row.kind === "group" ? row.path.slice(0, -1) : row.entry.path;
      for (let i = at - 1; i >= 0; i--) {
        const r = navRows[i];
        if (r.kind === "group" && pathKey(r.path) === pathKey(path)) return r;
      }
      return undefined;
    };

    switch (event.key) {
      case "ArrowDown":
        select(navRows[at < 0 ? 0 : Math.min(at + 1, navRows.length - 1)]);
        break;
      case "ArrowUp":
        // С первой строки — в поиск.
        if (at <= 0) search?.focus();
        else select(navRows[at - 1]);
        break;
      case "Home":
        select(navRows[0]);
        break;
      case "End":
        select(navRows[navRows.length - 1]);
        break;
      case "ArrowRight":
      case "Enter":
        if (!row) select(navRows[0]);
        else if (row.kind === "fav-head") {
          if (event.key === "Enter" || !favoritesOpen) favoritesOpen = !favoritesOpen;
        } else if (row.kind === "folders") {
          if (event.key === "Enter") toggleAll();
          else collapsed.clear();
        } else {
          // У раздела — в его список справа; свёрнутый заодно раскрывается.
          if (row.kind === "group" && !open) toggle(row);
          openFields();
        }
        break;
      case "ArrowLeft":
        if (!row) return;
        if (row.kind === "fav-head") favoritesOpen = false;
        else if (row.kind === "folders") for (const key of groupKeys) collapsed.add(key);
        else if (row.kind === "group" && open && !filtering) toggle(row);
        else select(parent());
        break;

      default:
        return;
    }
    event.preventDefault();
  }

  let tagChips = $derived.by(() => {
    const counts = new Map<string, number>();
    for (const entry of vault.entries) for (const t of entry.tags) counts.set(t, (counts.get(t) ?? 0) + 1);
    return [...counts].map(([name, count]) => ({ name, count }));
  });

  let empty = $derived(vault.entries.length === 0);

  // Теги занимают не больше двух строк, иначе вытеснили бы список записей.
  // Остальные раскрываются кнопкой.
  let tagsBox: HTMLElement | undefined = $state();
  let tagsOpen = $state(false);
  let tagsOverflow = $state(false);

  $effect(() => {
    void tagChips.length;
    if (!tagsBox || tagsOpen) return;
    tagsOverflow = tagsBox.scrollHeight > tagsBox.clientHeight + 1;
  });

  // Alt+↑ / Alt+↓ на избранной записи переносит её выше или ниже в избранном.
  function moveFavoriteByKey(event: KeyboardEvent): boolean {
    if (keyNav.zone !== "list" || !event.altKey || event.ctrlKey || event.metaKey || event.shiftKey) return false;
    if (event.key !== "ArrowUp" && event.key !== "ArrowDown") return false;
    if ((event.target as HTMLElement | null)?.closest("input, textarea, select, [contenteditable]")) return false;
    if (!favCursor) return false;
    event.preventDefault();
    const ids = favorites.map((e) => e.id);
    const from = ids.indexOf(vault.selectedId!);
    const to = from + (event.key === "ArrowUp" ? -1 : 1);
    if (to < 0 || to >= ids.length) return true;
    ids.splice(to, 0, ...ids.splice(from, 1));
    void saveFavoriteOrder(ids, false).then(() =>
      // Запись могла уйти за край области избранного.
      scrollToCursor(() => favBox?.querySelector(`[data-nav="f:${vault.selectedId}"]`)),
    );
    return true;
  }

  async function saveFavoriteOrder(ids: number[], toast = true) {
    try {
      await api.orderFavorites(ids);
      await loadEntries();
      if (toast) showToast("Порядок избранного изменён");
    } catch (err) {
      showToast(errorText(err));
    }
  }

  // ---- Ширина панели ------------------------------------------------------
  // Правый край панели тянется мышью; ←/→ на нём меняют ширину с клавиатуры,
  // двойной щелчок возвращает обычную. Ширина сохраняется в настройках приложения.

  const WIDTH = { min: 220, normal: 320, max: 640, step: 16 };
  // Справа от панели должно оставаться место для записи.
  const RIGHT_MIN = 400;

  let width = $state(app.settings.sidebarWidth);
  let resizing = $state(false);
  // Настройки приходят от Rust позже, чем рисуется панель.
  $effect(() => {
    const saved = app.settings.sidebarWidth;
    if (!resizing) width = saved;
  });

  const clampWidth = (w: number) =>
    Math.round(Math.max(WIDTH.min, Math.min(w, WIDTH.max, window.innerWidth - RIGHT_MIN)));

  async function saveWidth() {
    if (width === app.settings.sidebarWidth) return;
    const before = app.settings;
    app.settings = { ...before, sidebarWidth: width };
    try {
      app.settings = await api.updateSettings(app.settings);
    } catch (err) {
      app.settings = before;
      showToast(errorText(err));
    }
  }

  function startResize(event: PointerEvent) {
    if (event.button !== 0) return;
    event.preventDefault();
    resizing = true;
    const startX = event.clientX;
    const startWidth = width;
    const move = (e: PointerEvent) => (width = clampWidth(startWidth + e.clientX - startX));
    const stop = () => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", stop);
      window.removeEventListener("pointercancel", stop);
      resizing = false;
      void saveWidth();
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", stop);
    window.addEventListener("pointercancel", stop);
  }

  function onResizeKey(event: KeyboardEvent) {
    const delta = { ArrowLeft: -WIDTH.step, ArrowRight: WIDTH.step }[event.key];
    if (delta === undefined && event.key !== "Home" && event.key !== "End") return;
    // Иначе стрелки заодно ушли бы в список записей.
    event.preventDefault();
    event.stopPropagation();
    if (event.key === "Home") width = clampWidth(WIDTH.min);
    else if (event.key === "End") width = clampWidth(WIDTH.max);
    else width = clampWidth(width + delta!);
    void saveWidth();
  }

  function resetWidth() {
    width = clampWidth(WIDTH.normal);
    void saveWidth();
  }

  // ---- Перетаскивание ----------------------------------------------------
  // Сделано на событиях указателя: HTML5 drag-and-drop в окне Tauri на Windows
  // перехватывает само окно.

  type Dragged =
    | { kind: "entry"; entry: EntryView }
    | { kind: "section"; path: string[] }
    | { kind: "favorite"; entry: EntryView };
  type Target =
    | { kind: "into"; path: string[]; rowKey: string }
    | { kind: "before-favorite"; entry: EntryView; rowKey: string }
    | { kind: "after-favorite"; entry: EntryView; rowKey: string }
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
  /// Избранное переставляют внутри его области, поэтому прокручивается она.
  function autoScroll() {
    const box = dragging?.dragged.kind === "favorite" ? favBox : list;
    if (!box) return;
    const rect = box.getBoundingClientRect();
    const edge = box === favBox ? 20 : 40;
    if (pointer.y < rect.top + edge) box.scrollTop -= 8;
    else if (pointer.y > rect.bottom - edge) box.scrollTop += 8;
  }

  function findTarget(x: number, y: number): Target | null {
    if (!dragging) return null;
    const el = document.elementFromPoint(x, y)?.closest<HTMLElement>("[data-drop]");
    if (!el) return null;
    const rect = el.getBoundingClientRect();
    const offset = (y - rect.top) / rect.height;
    const dragged = dragging.dragged;

    // Избранное переставляется только внутри своего списка.
    const favorite = favorites.find((e) => `f:${e.id}` === el.dataset.drop);
    if (dragged.kind === "favorite" || favorite) {
      if (dragged.kind !== "favorite" || !favorite || favorite.id === dragged.entry.id) return null;
      const rowKey = `f:${favorite.id}`;
      return offset < 0.5
        ? { kind: "before-favorite", entry: favorite, rowKey }
        : { kind: "after-favorite", entry: favorite, rowKey };
    }

    if (el.dataset.drop === "root") return { kind: "root", rowKey: "root" };
    const row = rows.find((r) => r.key === el.dataset.drop);
    if (!row) return null;

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
    if (dragged.kind === "favorite") {
      if (where.kind !== "before-favorite" && where.kind !== "after-favorite") return;
      const ids = favorites.map((e) => e.id).filter((id) => id !== dragged.entry.id);
      const at = ids.indexOf(where.entry.id) + (where.kind === "after-favorite" ? 1 : 0);
      ids.splice(at, 0, dragged.entry.id);
      await saveFavoriteOrder(ids);
      return;
    }
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
    if (target.kind === "before-entry" || target.kind === "before-section" || target.kind === "before-favorite") {
      return "drop-before";
    }
    if (target.kind === "after-entry" || target.kind === "after-favorite") return "drop-after";
    return "drop-into";
  }

  function isDragged(row: Row): boolean {
    const d = dragging?.dragged;
    if (!d || d.kind === "favorite") return false;
    if (d.kind === "entry") return row.kind === "entry" && row.entry.id === d.entry.id;
    const path = row.kind === "group" ? row.path : row.entry.path;
    return startsWith(path, d.path);
  }
</script>

<svelte:window onkeydown={onKey} />

<aside class:dragging={dragging !== null} class:resizing style:width="{width}px">
  <!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
  <div
    class="resizer"
    role="separator"
    aria-orientation="vertical"
    aria-label="Ширина списка записей"
    aria-valuemin={WIDTH.min}
    aria-valuemax={WIDTH.max}
    aria-valuenow={width}
    title="Потяните, чтобы изменить ширину. Двойной щелчок — обычная ширина"
    tabindex="0"
    onpointerdown={startResize}
    ondblclick={resetWidth}
    onkeydown={onResizeKey}
  ></div>
  <div class="top">
    <div class="search">
      <Icon name="search" color="var(--text-subtle)" />
      <input
        type="search"
        bind:value={query}
        bind:this={search}
        onkeydown={onSearchKey}
        placeholder="Поиск по названию и тегам"
        aria-label="Поиск"
        disabled={empty}
        spellcheck="false"
      />
    </div>
    {#if tagChips.length > 0}
      <div class="tags" class:open={tagsOpen} bind:this={tagsBox}>
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
    {#if tagsOverflow || tagsOpen}
      <div class="tools">
        <button type="button" class="link" aria-expanded={tagsOpen} onclick={() => (tagsOpen = !tagsOpen)}>
          {tagsOpen ? "Свернуть теги" : `Все теги (${tagChips.length})`}
        </button>
      </div>
    {/if}
  </div>

  <div class="list scroll" bind:this={list}>
    {#if favorites.length > 0}
      <nav class="favorites" aria-label="Избранное">
        {#if favoritesLong}
          <button
            type="button"
            class="fav-head mono"
            class:on={cursorIn === "fav-head"}
            data-nav="fav-head"
            aria-expanded={favoritesOpen}
            onclick={() => {
              keyNav.zone = "list";
              cursorIn = "fav-head";
              favoritesOpen = !favoritesOpen;
            }}
          >
            <span class="chevron" class:open={favoritesOpen} aria-hidden="true"
              ><Icon name="chevron" size={14} stroke={2} /></span
            >
            <Icon name="star" size={12} stroke={2} fill="currentColor" />
            <span class="label">Избранное</span>
          </button>
        {:else}
          <div class="fav-head mono"><Icon name="star" size={12} stroke={2} fill="currentColor" />Избранное</div>
        {/if}
        <div class="fav-scroll scroll" class:open={favoritesOpen} bind:this={favBox}>
          {#each favorites as entry (entry.id)}
            <button
              type="button"
              class="entry fav {dropClass(`f:${entry.id}`)}"
              class:on={favCursor && entry.id === vault.selectedId}
              class:faded={dragging?.dragged.kind === "favorite" && dragging.dragged.entry.id === entry.id}
              data-nav="f:{entry.id}"
              data-drop="f:{entry.id}"
              onpointerdown={(e) => startPress(e, { kind: "favorite", entry }, entry.title)}
              onclick={() => {
                if (suppressClick) return;
                vault.selectedId = entry.id;
                vault.selectedSection = null;
                cursorIn = "fav";
                keyNav.zone = "list";
              }}
            >
              <span class="title">{entry.title}</span>
              <span class="hint mono">{entry.path.join(" / ")}</span>
            </button>
          {/each}
        </div>
      </nav>
    {/if}
    <nav aria-label="Записи">
      {#if groupKeys.length > 0}
        <!-- Кнопка над разделами, к которым относится. Во время поиска разделы
             всегда раскрыты, поэтому она недоступна. -->
        <div class="sections-tools">
          <button
            type="button"
            class="link folders"
            class:on={cursorIn === "folders"}
            data-nav="folders"
            disabled={filtering}
            onclick={() => {
              keyNav.zone = "list";
              cursorIn = "folders";
              toggleAll();
            }}
          >
            <Icon name="chevron" size={12} stroke={2} />
            {allCollapsed ? "Развернуть все" : "Свернуть все"}
          </button>
        </div>
      {/if}
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
            class:on={isCurrent(row)}
            data-drop={row.key}
            data-nav={row.key}
            aria-expanded={open}
            style:padding-left="{6 + row.depth * 16}px"
            onpointerdown={(e) => startPress(e, { kind: "section", path: row.path }, row.label)}
            onclick={(e) => {
              if (suppressClick) return;
              keyNav.zone = "list";
              cursorIn = "list";
              // Первый щелчок открывает справа записи раздела, следующий сворачивает его.
              // Стрелка слева сворачивает сразу.
              const onChevron = (e.target as HTMLElement).closest(".chevron") !== null;
              if (onChevron || isCurrent(row)) toggle(row);
              else if (!open) toggle(row);
              vault.selectedSection = row.path;
            }}
          >
            <span class="chevron" class:open aria-hidden="true"><Icon name="chevron" size={14} stroke={2} /></span>
            <span class="label">{row.label}</span>
          </button>
        {:else}
          <button
            type="button"
            class="entry {dropClass(row.key)}"
            class:on={isCurrent(row)}
            class:faded={isDragged(row)}
            data-drop={row.key}
            data-nav={row.key}
            aria-current={row.entry.id === vault.selectedId ? "true" : undefined}
            style:padding-left="{24 + row.depth * 16}px"
            onpointerdown={(e) => startPress(e, { kind: "entry", entry: row.entry }, row.entry.title)}
            onclick={() => {
              if (suppressClick) return;
              vault.selectedId = row.entry.id;
              vault.selectedSection = null;
              cursorIn = "list";
              keyNav.zone = "list";
            }}
          >
            <span class="title">{row.entry.title}</span>
            <span class="hint mono">{entryHint(row.entry)}</span>
          </button>
        {/if}
      {/each}
      {#if dragging && dragging.dragged.kind !== "favorite"}
        <div class="root-drop mono {dropClass('root')}" data-drop="root">Перетащите сюда, чтобы перенести на верхний уровень</div>
      {/if}
    </nav>
  </div>

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
    position: relative;
    flex: none;
    display: flex;
    flex-direction: column;
    min-height: 0;
    background: var(--bg-chrome);
    border-right: 1px solid var(--line);
  }

  /* Правый край панели: за него тянут ширину. Шире видимой линии, чтобы легко попасть. */
  .resizer {
    position: absolute;
    top: 0;
    right: -4px;
    bottom: 0;
    width: 8px;
    z-index: 5;
    cursor: col-resize;
    touch-action: none;
  }

  .resizer:hover,
  .resizer:focus-visible,
  aside.resizing .resizer {
    background: linear-gradient(to right, transparent 3px, var(--accent) 3px, var(--accent) 5px, transparent 5px);
    outline: none;
  }

  aside.resizing {
    cursor: col-resize;
    user-select: none;
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

  .tags:not(.open) {
    /* две строки чипов */
    max-height: 62px;
    overflow: hidden;
  }

  .tags.open {
    max-height: 40vh;
    overflow-y: auto;
  }

  .tools {
    display: flex;
    align-items: center;
    gap: 12px;
    margin-top: -4px;
  }

  .sections-tools {
    display: flex;
    justify-content: flex-end;
    padding: 4px 2px 0;
  }

  .link {
    font-size: 12px;
    color: var(--accent);
  }

  .link:hover:not(:disabled) {
    text-decoration: underline;
  }

  .link:disabled {
    color: var(--text-faint);
  }

  .folders {
    margin-left: auto;
    display: inline-flex;
    align-items: center;
    gap: 4px;
    padding: 4px 8px;
    border-radius: 6px;
  }

  /* Курсор стрелок на кнопке */
  .folders.on {
    background: color-mix(in srgb, var(--accent) 12%, transparent);
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

  .favorites {
    padding-bottom: 8px;
    margin-bottom: 4px;
    border-bottom: 1px solid var(--line);
  }

  .fav-head {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 6px;
    height: 30px;
    padding: 0 10px;
    font-size: 12px;
    color: var(--accent);
  }

  button.fav-head {
    padding-left: 6px;
    border-radius: 6px;
  }

  button.fav-head:hover {
    background: var(--bg-hover);
  }

  button.fav-head.on {
    background: color-mix(in srgb, var(--accent) 12%, transparent);
  }

  /* Области избранного: 5 строк по 30px, раскрытая — 10. */
  .fav-scroll {
    max-height: 150px;
    overflow-y: auto;
  }

  .fav-scroll.open {
    max-height: 300px;
  }

  .entry.fav {
    padding-left: 28px;
  }

  .fav .hint {
    max-width: 50%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
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
    height: 30px;
    padding-right: 10px;
    border-radius: 6px;
    font-size: 12px;
    color: var(--text-muted);
  }

  .group:hover {
    background: var(--bg-hover);
  }

  /* Выбранный раздел подсвечивается так же, как выбранная запись: в нём создастся новая запись. */
  .group.on {
    background: color-mix(in srgb, var(--accent) 12%, transparent);
    color: var(--text-strong);
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
    height: 30px;
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
