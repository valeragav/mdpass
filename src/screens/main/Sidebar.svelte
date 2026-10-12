<script lang="ts">
  import type { EntryView } from "../../lib/api";
  import { go } from "../../lib/nav.svelte";
  import Button from "../../lib/ui/Button.svelte";
  import Icon from "../../lib/ui/Icon.svelte";
  import { entryHint, vault } from "../../lib/vault.svelte";

  interface Props {
    /// Записи после поиска и фильтра по тегу.
    visible: EntryView[];
    query: string;
    tag: string;
  }

  let { visible, query = $bindable(), tag = $bindable() }: Props = $props();

  type Row =
    | { kind: "group"; key: string; depth: number; label: string }
    | { kind: "entry"; key: string; depth: number; entry: EntryView };

  // Записи идут в порядке файла. Заголовок раздела ставим там, где путь меняется.
  let rows = $derived.by(() => {
    const out: Row[] = [];
    let last: string[] = [];
    for (const entry of visible) {
      entry.path.forEach((part, i) => {
        // Если раздел сменился на этом уровне, все более глубокие тоже начинаются заново.
        if (last[i] !== part) {
          out.push({ kind: "group", key: `g:${entry.id}:${i}`, depth: i, label: part });
          last = [...last.slice(0, i), part];
        }
      });
      last = entry.path.slice();
      out.push({ kind: "entry", key: `e:${entry.id}`, depth: entry.path.length, entry });
    }
    return out;
  });

  let tagChips = $derived.by(() => {
    const counts = new Map<string, number>();
    for (const entry of vault.entries) for (const t of entry.tags) counts.set(t, (counts.get(t) ?? 0) + 1);
    return [...counts].map(([name, count]) => ({ name, count }));
  });

  let empty = $derived(vault.entries.length === 0);
</script>

<aside>
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

  <nav class="list scroll" aria-label="Записи">
    {#if empty}
      <p class="none">Записей пока нет</p>
    {:else if visible.length === 0}
      <p class="none">Ничего не найдено</p>
    {/if}
    {#each rows as row (row.key)}
      {#if row.kind === "group"}
        <div class="group mono" style:padding-left="{10 + row.depth * 16}px">
          <span class="hashes">{"#".repeat(row.depth + 1)}</span>
          <span>{row.label}</span>
        </div>
      {:else}
        <button
          type="button"
          class="entry"
          class:on={row.entry.id === vault.selectedId}
          aria-current={row.entry.id === vault.selectedId ? "true" : undefined}
          style:padding-left="{10 + row.depth * 16}px"
          onclick={() => (vault.selectedId = row.entry.id)}
        >
          <span class="title">{row.entry.title}</span>
          <span class="hint mono">{entryHint(row.entry)}</span>
        </button>
      {/if}
    {/each}
  </nav>

  <div class="bottom">
    <Button variant="primary" size="sm" icon="plus" full onclick={() => go("new-entry")}>Новая запись</Button>
  </div>
</aside>

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
    display: flex;
    gap: 6px;
    padding: 14px 8px 6px;
    font-size: 12px;
    color: var(--text-muted);
  }

  .hashes {
    color: var(--text-faint);
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
    color: #fff;
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

  .bottom {
    padding: 12px 16px 16px;
    border-top: 1px solid var(--line);
  }
</style>
