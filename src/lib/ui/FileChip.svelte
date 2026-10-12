<script lang="ts">
  import Icon from "./Icon.svelte";

  interface Props {
    label: string;
    /// Если задано, плашка — кнопка выбора другого файла.
    onpick?: () => void;
    pickLabel?: string;
  }

  let { label, onpick, pickLabel = "Выбрать другой файл" }: Props = $props();
</script>

{#if onpick}
  <button type="button" class="chip pickable mono" onclick={onpick} title={pickLabel}>
    <Icon name="file" />
    <span class="label">{label}</span>
    <span class="action">Сменить</span>
  </button>
{:else}
  <div class="chip mono">
    <Icon name="file" />
    <span class="label">{label}</span>
  </div>
{/if}

<style>
  .chip {
    width: 100%;
    min-height: var(--control-h);
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 0 14px;
    border-radius: var(--radius);
    background: var(--bg-chrome);
    border: 1px solid var(--line);
    font-size: 13px;
    color: var(--text-muted);
  }

  .label {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .action {
    font-family: var(--font-sans);
    font-size: 12px;
    color: var(--text-subtle);
  }

  .pickable:hover {
    border-color: var(--line-strong);
    background: var(--bg-hover);
  }

  .pickable:hover .action {
    color: var(--text);
  }
</style>
