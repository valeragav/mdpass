<script lang="ts">
  import type { EntryView } from "../../lib/api";
  import { fieldCursor, onFieldKey, type FieldRef } from "../../lib/fields";
  import { keyNav } from "../../lib/keys.svelte";
  import { plural } from "../../lib/text";
  import { vault } from "../../lib/vault.svelte";
  import FieldRow from "./FieldRow.svelte";

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

  let subsections = $derived.by(() => {
    const names = new Set<string>();
    for (const e of visible) {
      if (e.path.length > path.length && path.every((part, i) => e.path[i] === part)) names.add(e.path[path.length]);
    }
    return [...names];
  });

  // Поля всех записей подряд: по ним ходят стрелки.
  let order = $derived<FieldRef[]>(entries.flatMap((entry) => entry.fields.map((_, index) => ({ entry, index }))));
  let cursor = $derived(fieldCursor(order.length));
  // Номер первого поля каждой записи в общем порядке.
  let starts = $derived.by(() => {
    const out = new Map<number, number>();
    let pos = 0;
    for (const e of entries) {
      out.set(e.id, pos);
      pos += e.fields.length;
    }
    return out;
  });

  function openEntry(entry: EntryView) {
    vault.selectedId = entry.id;
    vault.selectedSection = null;
    keyNav.zone = "list";
  }

  let root: HTMLElement | undefined = $state();
</script>

<svelte:window onkeydown={(e) => onFieldKey(e, order, root)} />

<main bind:this={root}>
  <div class="head">
    <div class="crumb mono">{path.slice(0, -1).join(" / ")}</div>
    <h1>{path.at(-1)}</h1>
    <p class="count">
      {entries.length}
      {plural(entries.length, ["запись", "записи", "записей"])} в разделе{#if subsections.length > 0}
        · подразделы:
        {#each subsections as name, i (name)}
          {i > 0 ? ", " : ""}<button type="button" class="sub mono" onclick={() => (vault.selectedSection = [...path, name])}
            >{name}</button
          >
        {/each}
      {/if}
    </p>
  </div>

  <div class="content scroll">
    <div class="inner">
      {#if entries.length === 0}
        <p class="hint">Прямо в этом разделе записей нет{subsections.length > 0 ? ", только подразделы" : ""}</p>
      {/if}
      {#each entries as entry (entry.id)}
        {@const start = starts.get(entry.id) ?? 0}
        <section class="card" aria-label={entry.title}>
          <div class="card-head">
            <button type="button" class="title" onclick={() => openEntry(entry)}>{entry.title}</button>
            {#each entry.tags as tag (tag)}
              <span class="tag mono">[{tag}]</span>
            {/each}
          </div>
          {#if entry.fields.length > 0}
            <div class="fields">
              {#each entry.fields as _, index (index)}
                <FieldRow {entry} {index} pos={start + index} cursor={cursor === start + index} />
              {/each}
            </div>
          {:else}
            <p class="hint">В записи нет полей</p>
          {/if}
        </section>
      {/each}
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

  .crumb {
    min-height: 17px;
    font-size: 12px;
    color: var(--text-subtle);
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

  .sub {
    color: var(--accent);
    font-size: 13px;
  }

  .sub:hover {
    text-decoration: underline;
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
</style>
