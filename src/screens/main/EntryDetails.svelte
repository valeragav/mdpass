<script lang="ts">
  import { api, errorText, type EntryView } from "../../lib/api";
  import { app, go } from "../../lib/nav.svelte";
  import { showToast } from "../../lib/toast.svelte";
  import Button from "../../lib/ui/Button.svelte";
  import IconButton from "../../lib/ui/IconButton.svelte";
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
      showToast(field.secret ? "Секрет скопирован · буфер очистится через 30 с" : `Скопировано: ${field.key}`);
    } catch (err) {
      showToast(errorText(err));
    }
  }

  // Так запись выглядит в файле. Секреты скрыты, пока их не показали.
  let lines = $derived.by(() => {
    const out: { prefix: string; key: string; value: string; tone: "muted" | "title" | "plain" }[] = [];
    entry.path.forEach((part, i) => out.push({ prefix: "#".repeat(i + 1) + " ", key: "", value: part, tone: "muted" }));
    out.push({ prefix: "#".repeat(Math.max(3, entry.path.length + 1)) + " ", key: "", value: entry.title, tone: "title" });
    const tagsLine = entry.tags.length ? entry.tags.map((t) => `[${t}]`).join(", ") : null;
    let tagsDone = false;
    entry.fields.forEach((field, index) => {
      if (tagsLine && !tagsDone && field.key.toLowerCase() === "note") {
        out.push({ prefix: "", key: "tags:", value: " " + tagsLine, tone: "plain" });
        tagsDone = true;
      }
      const value = shownValue(index) ?? "••••••••";
      const [first, ...rest] = value.split("\n");
      out.push({ prefix: "", key: field.key + ":", value: first ? " " + first : "", tone: "plain" });
      for (const line of rest) out.push({ prefix: "  ", key: "", value: line, tone: "plain" });
    });
    if (tagsLine && !tagsDone) out.push({ prefix: "", key: "tags:", value: " " + tagsLine, tone: "plain" });
    return out;
  });

  let fileName = $derived(app.vault?.name.replace(/\.age$/, "") ?? "vault.md");
</script>

<main>
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
            {#each plain as { field, index } (index)}
              {@const value = shownValue(index)}
              <div class="field">
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
          <p class="hint">Буфер обмена очищается через 30 секунд после копирования</p>
        {:else if notes.length === 0}
          <p class="hint">В записи нет полей</p>
        {/if}

        {#each notes as { field, index } (index)}
          {@const value = shownValue(index)}
          <div class="note">
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
          <div class="md-body mono">
            {#each lines as line, i (i)}
              <div class="line">
                <span class="n">{i + 1}</span>
                <span class="text"
                  ><span class="prefix">{line.prefix}</span><span class="k">{line.key}</span><span class="v {line.tone}"
                    >{line.value}</span
                  ></span
                >
              </div>
            {/each}
          </div>
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
    color: #fff;
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

  .key {
    font-size: 13px;
    color: var(--text-subtle);
    overflow-wrap: anywhere;
  }

  .value {
    font-size: 14px;
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

  .md-body {
    padding: 16px 18px;
  }

  .line {
    display: flex;
    gap: 16px;
    font-size: 13px;
    line-height: 22px;
  }

  .n {
    width: 20px;
    flex: none;
    text-align: right;
    color: var(--text-ghost);
    user-select: none;
  }

  .text {
    white-space: pre-wrap;
    word-break: break-all;
  }

  .prefix {
    color: var(--text-faint);
  }

  .k {
    color: var(--accent);
  }

  .v.muted {
    color: var(--text-subtle);
  }

  .v.title {
    color: var(--accent);
  }
</style>
