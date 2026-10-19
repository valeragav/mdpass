<script lang="ts">
  import type { EntryView } from "../../lib/api";
  import { fieldCursor, onFieldKey, type CursorItem } from "../../lib/fields";
  import { keyNav } from "../../lib/keys.svelte";
  import { layout as screen } from "../../lib/layout.svelte";
  import { plural } from "../../lib/text";
  import { vault } from "../../lib/vault.svelte";
  import Icon from "../../lib/ui/Icon.svelte";
  import FieldRow from "./FieldRow.svelte";
  import Crumb from "./Crumb.svelte";

  interface Props {
    /// Выбранный раздел.
    path: string[];
    /// Записи после поиска и фильтра по тегу.
    visible: EntryView[];
  }

  let { path, visible }: Props = $props();

  const same = (a: string[], b: string[]) => a.length === b.length && a.every((part, i) => part === b[i]);

  // Только записи самого раздела: записи подразделов видны, когда выбирают подраздел.
  let entries = $derived(visible.filter((e) => same(e.path, path)));

  /// Подразделы первого уровня и сколько записей в каждом, со всеми вложенными.
  let subsections = $derived.by(() => {
    const counts = new Map<string, number>();
    for (const e of visible) {
      if (e.path.length > path.length && path.every((part, i) => e.path[i] === part)) {
        const name = e.path[path.length];
        counts.set(name, (counts.get(name) ?? 0) + 1);
      }
    }
    return [...counts].map(([name, count]) => ({ name, count }));
  });

  function openSection(name: string) {
    vault.selectedSection = [...path, name];
  }

  function openEntry(entry: EntryView) {
    vault.selectedId = entry.id;
    vault.selectedSection = null;
  }

  // Порядок строк для стрелок: у каждой записи название и поля, потом подразделы.
  // Номер первой строки каждой записи — её название.
  let layout = $derived.by(() => {
    const order: CursorItem[] = [];
    const starts = new Map<number, number>();
    for (const entry of entries) {
      starts.set(entry.id, order.length);
      order.push({ open: () => openEntry(entry) });
      entry.fields.forEach((_, index) => order.push({ entry, index }));
    }
    const subStart = order.length;
    for (const { name } of subsections) order.push({ open: () => openSection(name) });
    return { order, starts, subStart };
  });
  let cursor = $derived(fieldCursor(layout.order.length));

  let root: HTMLElement | undefined = $state();
</script>

<svelte:window onkeydown={(e) => onFieldKey(e, layout.order, root)} />

<main bind:this={root}>
  <div class="head">
    <!-- На телефоне пустой путь не занимает строку: место нужнее под записи. -->
    {#if path.length > 1 || !screen.narrow}<Crumb path={path.slice(0, -1)} />{/if}
    <h1>{path.at(-1)}</h1>
    <p class="count">
      {entries.length}
      {plural(entries.length, ["запись", "записи", "записей"])} в разделе{subsections.length > 0
        ? ` · ${subsections.length} ${plural(subsections.length, ["подраздел", "подраздела", "подразделов"])}`
        : ""}
    </p>
  </div>

  <div class="content scroll">
    <div class="inner">
      {#if entries.length === 0}
        <p class="hint">Прямо в этом разделе записей нет{subsections.length > 0 ? ", только подразделы" : ""}</p>
      {/if}
      {#each entries as entry (entry.id)}
        {@const start = layout.starts.get(entry.id) ?? 0}
        <section class="card" aria-label={entry.title}>
          <div class="card-head">
            <button
              type="button"
              class="title"
              class:cursor={cursor === start}
              data-cursor={start}
              onclick={() => {
                openEntry(entry);
                keyNav.zone = "list";
              }}>{entry.title}</button
            >
            {#each entry.tags as tag (tag)}
              <span class="tag mono">[{tag}]</span>
            {/each}
          </div>
          {#if entry.fields.length > 0}
            <div class="fields">
              {#each entry.fields as _, index (index)}
                <FieldRow {entry} {index} pos={start + 1 + index} cursor={cursor === start + 1 + index} />
              {/each}
            </div>
          {:else}
            <p class="hint">В записи нет полей</p>
          {/if}
        </section>
      {/each}
      {#if subsections.length > 0}
        <nav class="subs" aria-label="Подразделы">
          {#each subsections as sub, pos (sub.name)}
            <button
              type="button"
              class="sub"
              class:cursor={cursor === layout.subStart + pos}
              data-cursor={layout.subStart + pos}
              onclick={() => {
                openSection(sub.name);
                keyNav.zone = "list";
              }}
            >
              <span class="chevron" aria-hidden="true"><Icon name="chevron" size={14} stroke={2} /></span>
              <span class="sub-name mono">{sub.name}</span>
              <span class="sub-count">{sub.count} {plural(sub.count, ["запись", "записи", "записей"])}</span>
            </button>
          {/each}
        </nav>
      {/if}
    </div>
  </div>
</main>

<style>
  main {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }

  .head {
    padding: 28px 40px 0;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  h1 {
    font-size: 26px;
    font-weight: 600;
    letter-spacing: -0.01em;
    overflow-wrap: anywhere;
  }

  .count {
    font-size: 13px;
    color: var(--text-muted);
  }

  .subs {
    display: flex;
    flex-direction: column;
    gap: 1px;
    background: var(--line);
    border: 1px solid var(--line);
    border-radius: 10px;
    overflow: hidden;
  }

  .sub {
    display: flex;
    align-items: center;
    gap: 10px;
    min-height: 44px;
    padding: 0 18px;
    background: var(--bg-panel);
    text-align: left;
  }

  .sub:hover {
    background: var(--bg-hover);
  }

  .chevron {
    display: inline-flex;
    color: var(--text-faint);
  }

  .sub-name {
    flex: 1;
    min-width: 0;
    font-size: 13px;
    color: var(--text-2);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .sub-count {
    flex: none;
    font-size: 12px;
    color: var(--text-subtle);
  }

  /* Строка под курсором клавиатуры: Enter или → откроет её. */
  .sub.cursor {
    background: color-mix(in srgb, var(--accent) 10%, var(--bg-panel));
    box-shadow: inset 3px 0 0 var(--accent);
  }

  .title.cursor {
    color: var(--accent);
    text-decoration: underline;
    text-underline-offset: 4px;
  }

  .content {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: 24px 40px 40px;
  }

  .inner {
    max-width: 780px;
    display: flex;
    flex-direction: column;
    gap: 20px;
  }

  .card {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .card-head {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
  }

  .title {
    font-size: 16px;
    font-weight: 600;
    color: var(--text-strong);
    text-align: left;
    overflow-wrap: anywhere;
  }

  .title:hover {
    color: var(--accent);
  }

  .tag {
    height: 22px;
    display: flex;
    align-items: center;
    padding: 0 7px;
    border-radius: 6px;
    font-size: 11px;
    color: var(--accent);
    border: 1px solid color-mix(in srgb, var(--accent) 40%, transparent);
  }

  .fields {
    display: flex;
    flex-direction: column;
    gap: 1px;
    background: var(--line);
    border: 1px solid var(--line);
    border-radius: 10px;
    overflow: hidden;
  }

  .hint {
    font-size: 12px;
    color: var(--text-subtle);
  }

  /* Телефон: поля и отступ сверху как у карточки записи. */
  @media (max-width: 640px) {
    .head {
      padding: 16px 16px 0;
    }

    h1 {
      font-size: 24px;
    }

    .content {
      padding: 16px 16px 32px;
    }
  }
</style>
