<script lang="ts">
  import type { EntryView } from "../../lib/api";
  import { copyField, MASK, pickField, revealField, shownValue } from "../../lib/fields";
  import IconButton from "../../lib/ui/IconButton.svelte";

  interface Props {
    entry: EntryView;
    /// Номер поля в записи.
    index: number;
    /// Номер строки на экране: по нему ходят стрелки.
    pos: number;
    cursor: boolean;
  }

  let { entry, index, pos, cursor }: Props = $props();

  let field = $derived(entry.fields[index]);
  let value = $derived(shownValue(entry, index));
</script>

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div class="field" class:cursor data-cursor={pos} onclick={() => pickField(pos)}>
  <span class="key mono">{field.key}</span>
  <span class="value mono" class:masked={value === null}>{value ?? MASK}</span>
  <div class="field-actions">
    {#if field.secret}
      <IconButton
        icon={value === null ? "eye" : "eye-off"}
        label={value === null ? `Показать ${field.key}` : `Скрыть ${field.key}`}
        onclick={() => revealField(entry, index)}
      />
    {/if}
    <IconButton icon="copy" label={`Скопировать ${field.key}`} onclick={() => copyField(entry, index)} />
  </div>
</div>

<style>
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
</style>
