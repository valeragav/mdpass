<script lang="ts">
  import { errorText, type EntryView } from "../../lib/api";
  import { app, go } from "../../lib/nav.svelte";
  import { previewLines } from "../../lib/entryText";
  import { fieldCursor, MASK, onFieldKey, pickField, revealField, shownValue } from "../../lib/fields";
  import { plural } from "../../lib/text";
  import Button from "../../lib/ui/Button.svelte";
  import MarkdownPreview from "../../lib/ui/MarkdownPreview.svelte";
  import IconButton from "../../lib/ui/IconButton.svelte";
  import { showToast } from "../../lib/toast.svelte";
  import { isNote, toggleFavorite } from "../../lib/vault.svelte";
  import FieldRow from "./FieldRow.svelte";

  interface Props {
    entry: EntryView;
    ondelete: () => void;
    /// На телефоне кнопки записи стоят в верхней строке экрана, а не у названия.
    compact?: boolean;
  }

  let { entry, ondelete, compact = false }: Props = $props();

  let mode = $state<"card" | "markdown">("card");

  // Индексы полей нужны Rust-части: по ним он находит значение секрета.
  let indexed = $derived(entry.fields.map((field, index) => ({ field, index })));
  let plain = $derived(indexed.filter(({ field }) => !isNote(field.key)));
  let notes = $derived(indexed.filter(({ field }) => isNote(field.key)));
  // Порядок полей на экране: сначала обычные, потом заметки. По нему ходят стрелки.
  // В виде «Markdown» полей нет, и стрелкам ходить некуда.
  let order = $derived(mode === "card" ? [...plain, ...notes].map(({ index }) => ({ entry, index })) : []);
  let cursor = $derived(fieldCursor(order.length));

  let root: HTMLElement | undefined = $state();

  // Так запись выглядит в файле. Секреты скрыты, пока их не показали.
  let lines = $derived(
    previewLines({
      path: entry.path,
      title: entry.title,
      tags: entry.tags,
      favorite: entry.favorite,
      favoriteOrder: entry.favoriteOrder,
      fields: entry.fields.map((field, index) => ({ key: field.key, value: shownValue(entry, index) })),
    }),
  );

  let fileName = $derived(app.vault?.name.replace(/\.age$/, "") ?? "vault.md");
</script>

<svelte:window onkeydown={(e) => onFieldKey(e, order, root)} />

<main bind:this={root}>
  <div class="head">
    <div class="crumb mono">{entry.path.join(" / ")}</div>
    <div class="title-row">
      <h1>{entry.title}</h1>
      {#if !compact}
      <div class="actions">
        <IconButton
          icon="star"
          label={entry.favorite ? "Убрать из избранного" : "Добавить в избранное"}
          bordered
          size="sm"
          pressed={entry.favorite}
          filled={entry.favorite}
          onclick={() => toggleFavorite(entry).catch((err) => showToast(errorText(err)))}
        />
        <Button size="sm" icon="edit" onclick={() => go("edit-entry")}>Редактировать</Button>
        <IconButton icon="trash" label="Удалить запись" bordered size="sm" onclick={ondelete} />
      </div>
      {/if}
    </div>
    {#if entry.tags.length > 0}
      <div class="tags">
        {#each entry.tags as tag (tag)}
          <span class="tag mono">[{tag}]</span>
        {/each}
      </div>
    {/if}
    <div class="tabs" role="tablist" aria-label="Вид записи">
      <button
        type="button"
        role="tab"
        aria-selected={mode === "card"}
        class:on={mode === "card"}
        onclick={() => (mode = "card")}>Карточка</button
      >
      <button
        type="button"
        role="tab"
        aria-selected={mode === "markdown"}
        class:on={mode === "markdown"}
        onclick={() => (mode = "markdown")}>Markdown</button
      >
    </div>
  </div>

  <div class="content scroll">
    <div class="inner">
      {#if mode === "card"}
        {#if plain.length > 0}
          <div class="fields">
            {#each plain as { index }, pos (index)}
              <FieldRow {entry} {index} {pos} cursor={cursor === pos} />
            {/each}
          </div>
          {#if app.settings.clipboardClearSeconds > 0}
            <p class="hint">
              Буфер обмена очищается через {app.settings.clipboardClearSeconds}
              {plural(app.settings.clipboardClearSeconds, ["секунду", "секунды", "секунд"])} после копирования
            </p>
          {/if}
        {:else if notes.length === 0}
          <p class="hint">В записи нет полей</p>
        {/if}

        {#each notes as { field, index }, i (index)}
          {@const value = shownValue(entry, index)}
          {@const pos = plain.length + i}
          <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
          <div class="note" class:cursor={cursor === pos} data-cursor={pos} onclick={() => pickField(pos)}>
            <span class="note-key mono">{field.key}</span>
            <span class="note-text">{value ?? MASK}</span>
            {#if field.secret}
              <IconButton
                icon={value === null ? "eye" : "eye-off"}
                label={value === null ? `Показать ${field.key}` : `Скрыть ${field.key}`}
                onclick={() => revealField(entry, index)}
              />
            {/if}
          </div>
        {/each}
      {:else}
        <div class="markdown">
          <div class="md-head mono">
            <span>{fileName} · фрагмент</span>
            <span>секреты скрыты, пока вы их не покажете</span>
          </div>
          <MarkdownPreview {lines} />
        </div>
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
    gap: 12px;
  }

  .crumb {
    min-height: 17px;
    font-size: 12px;
    color: var(--text-subtle);
  }

  .title-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
  }

  h1 {
    min-width: 0;
    font-size: 26px;
    font-weight: 600;
    letter-spacing: -0.01em;
    overflow-wrap: anywhere;
  }

  .actions {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .actions :global(.btn) {
    height: 36px;
  }

  .tags {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }

  .tag {
    height: 24px;
    display: flex;
    align-items: center;
    padding: 0 8px;
    border-radius: 6px;
    font-size: 12px;
    color: var(--accent);
    border: 1px solid color-mix(in srgb, var(--accent) 40%, transparent);
  }

  .tabs {
    align-self: flex-start;
    display: flex;
    gap: 2px;
    padding: 3px;
    border-radius: var(--radius);
    background: var(--bg-panel);
    border: 1px solid var(--line);
  }

  .tabs button {
    height: 30px;
    padding: 0 14px;
    border-radius: 6px;
    font-weight: 500;
    color: var(--text-muted);
  }

  .tabs button.on {
    background: var(--bg-hover-strong);
    color: var(--text-strong);
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
    gap: 16px;
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

  .note {
    display: flex;
    align-items: flex-start;
    gap: 16px;
    padding: 14px 18px;
    border: 1px dashed var(--line-dashed);
    border-radius: 10px;
  }

  .note.cursor {
    border-style: solid;
    border-color: var(--accent);
    background: color-mix(in srgb, var(--accent) 8%, transparent);
  }

  .note-key {
    flex: none;
    font-size: 13px;
    color: var(--accent);
  }

  .note-text {
    flex: 1;
    font-size: calc(14px * var(--entry-scale));
    white-space: pre-wrap;
    overflow-wrap: anywhere;
  }

  .markdown {
    border: 1px solid var(--line);
    border-radius: 10px;
    background: var(--bg-inset);
    overflow: hidden;
  }

  .md-head {
    display: flex;
    justify-content: space-between;
    gap: 16px;
    padding: 10px 18px;
    border-bottom: 1px solid var(--line);
    font-size: 12px;
    color: var(--text-subtle);
  }


  /* Узкий экран: название на всю строку, кнопки под ним, поля ближе к краям. */
  @media (max-width: 640px) {
    .head {
      padding: 16px 16px 0;
    }

    .title-row {
      flex-wrap: wrap;
      gap: 10px;
    }

    h1 {
      flex: 1 1 100%;
      font-size: 24px;
    }

    .content {
      padding: 16px 16px 32px;
    }
  }
</style>
