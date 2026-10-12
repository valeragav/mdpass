<script lang="ts">
  import { api, errorText, type EntryView } from "../../lib/api";
  import { app, go } from "../../lib/nav.svelte";
  import { showToast } from "../../lib/toast.svelte";
  import { previewLines } from "../../lib/entryText";
  import { plural } from "../../lib/text";
  import Button from "../../lib/ui/Button.svelte";
  import MarkdownPreview from "../../lib/ui/MarkdownPreview.svelte";
  import IconButton from "../../lib/ui/IconButton.svelte";
  import { isNavKey, keyNav, scrollToCursor } from "../../lib/keys.svelte";
  import { isNote, revealKey, toggleReveal, vault } from "../../lib/vault.svelte";

  interface Props {
    entry: EntryView;
    ondelete: () => void;
  }

  let { entry, ondelete }: Props = $props();

  let mode = $state<"card" | "markdown">("card");

  const MASK = "••••••••••••";

  // Индексы полей нужны Rust-части: по ним он находит значение секрета.
  let indexed = $derived(entry.fields.map((field, index) => ({ field, index })));
  let plain = $derived(indexed.filter(({ field }) => !isNote(field.key)));
  let notes = $derived(indexed.filter(({ field }) => isNote(field.key)));
  // Порядок полей на экране: сначала обычные, потом заметки. По нему ходят стрелки.
  let order = $derived([...plain, ...notes]);
  let cursor = $derived(
    keyNav.zone === "fields" && mode === "card" && order.length > 0 ? Math.min(keyNav.field, order.length - 1) : -1,
  );

  function shownValue(index: number): string | null {
    const field = entry.fields[index];
    if (!field.secret) return field.value;
    return vault.revealed[revealKey(entry.id, index)] ?? null;
  }

  async function reveal(index: number) {
    try {
      await toggleReveal(entry.id, index);
    } catch (err) {
      showToast(errorText(err));
    }
  }

  async function copy(index: number) {
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

  let root: HTMLElement | undefined = $state();

  function moveCursor(to: number) {
    keyNav.field = Math.max(0, Math.min(to, order.length - 1));
    scrollToCursor(() => root?.querySelector(`[data-cursor="${keyNav.field}"]`));
  }

  function pick(pos: number) {
    keyNav.zone = "fields";
    keyNav.field = pos;
  }

  // ↑ ↓ — по полям, ← или Esc — назад в список, Ctrl+C или Enter — скопировать,
  // пробел — показать или скрыть секрет.
  function onKey(event: KeyboardEvent) {
    if (keyNav.zone !== "fields" || !isNavKey(event)) return;
    const current = cursor >= 0 ? order[cursor] : undefined;
    if (event.ctrlKey || event.metaKey) {
      const key = event.key.toLowerCase();
      // Выделенный мышью текст Ctrl+C копирует как обычно.
      if ((key === "c" || key === "с") && current && !window.getSelection()?.toString()) {
        event.preventDefault();
        copy(current.index);
      }
      return;
    }
    if (event.shiftKey) return;
    switch (event.key) {
      case "ArrowDown":
        moveCursor(cursor + 1);
        break;
      case "ArrowUp":
        moveCursor(cursor - 1);
        break;
      case "Home":
        moveCursor(0);
        break;
      case "End":
        moveCursor(order.length - 1);
        break;
      case "ArrowLeft":
      case "Escape":
        keyNav.zone = "list";
        break;
      case "Enter":
        if (!current) return;
        copy(current.index);
        break;
      case " ":
        if (!current?.field.secret) return;
        reveal(current.index);
        break;
      default:
        return;
    }
    event.preventDefault();
  }

  // Так запись выглядит в файле. Секреты скрыты, пока их не показали.
  let lines = $derived(
    previewLines({
      path: entry.path,
      title: entry.title,
      tags: entry.tags,
      fields: entry.fields.map((field, index) => ({ key: field.key, value: shownValue(index) })),
    }),
  );

  let fileName = $derived(app.vault?.name.replace(/\.age$/, "") ?? "vault.md");
</script>

<svelte:window onkeydown={onKey} />

<main bind:this={root}>
  <div class="head">
    <div class="crumb mono">{entry.path.join(" / ")}</div>
    <div class="title-row">
      <h1>{entry.title}</h1>
      <div class="actions">
        <Button size="sm" icon="edit" onclick={() => go("edit-entry")}>Редактировать</Button>
        <IconButton icon="trash" label="Удалить запись" bordered size="sm" onclick={ondelete} />
      </div>
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
            {#each plain as { field, index }, pos (index)}
              {@const value = shownValue(index)}
              <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
              <div class="field" class:cursor={cursor === pos} data-cursor={pos} onclick={() => pick(pos)}>
                <span class="key mono">{field.key}</span>
                <span class="value mono" class:masked={value === null}>{value ?? MASK}</span>
                <div class="field-actions">
                  {#if field.secret}
                    <IconButton
                      icon={value === null ? "eye" : "eye-off"}
                      label={value === null ? `Показать ${field.key}` : `Скрыть ${field.key}`}
                      onclick={() => reveal(index)}
                    />
                  {/if}
                  <IconButton icon="copy" label={`Скопировать ${field.key}`} onclick={() => copy(index)} />
                </div>
              </div>
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
          {@const value = shownValue(index)}
          {@const pos = plain.length + i}
          <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
          <div class="note" class:cursor={cursor === pos} data-cursor={pos} onclick={() => pick(pos)}>
            <span class="note-key mono">{field.key}</span>
            <span class="note-text">{value ?? MASK}</span>
            {#if field.secret}
              <IconButton
                icon={value === null ? "eye" : "eye-off"}
                label={value === null ? `Показать ${field.key}` : `Скрыть ${field.key}`}
                onclick={() => reveal(index)}
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

  .field {
    display: grid;
    grid-template-columns: 130px minmax(0, 1fr) auto;
    align-items: center;
    gap: 12px;
    min-height: 56px;
    padding: 0 8px 0 18px;
    background: var(--bg-panel);
  }

  /* Поле под курсором клавиатуры: Ctrl+C или Enter скопирует его. */
  .field.cursor {
    background: color-mix(in srgb, var(--accent) 10%, var(--bg-panel));
    box-shadow: inset 3px 0 0 var(--accent);
  }

  .note.cursor {
    border-style: solid;
    border-color: var(--accent);
    background: color-mix(in srgb, var(--accent) 8%, transparent);
  }

  .key {
    font-size: calc(13px * var(--entry-scale));
    color: var(--text-subtle);
    overflow-wrap: anywhere;
  }

  .value {
    font-size: calc(14px * var(--entry-scale));
    word-break: break-all;
    white-space: pre-wrap;
    padding: 8px 0;
  }

  .value.masked {
    color: var(--text-muted);
  }

  .field-actions {
    display: flex;
    gap: 4px;
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

</style>
