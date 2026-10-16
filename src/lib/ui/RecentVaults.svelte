<script lang="ts">
  import type { VaultFile } from "../api";
  import { app } from "../nav.svelte";
  import { IS_MOBILE } from "../platform";
  import Icon from "./Icon.svelte";

  /// Выбор хранилища из недавних. Текущее в списке не показывается.
  let { onpick }: { onpick: (file: VaultFile) => void } = $props();

  let others = $derived(app.recentVaults.filter((file) => file.path !== app.vault?.path));
</script>

{#if others.length > 0}
  <div class="recent">
    <div class="title">Недавние хранилища</div>
    {#each others as file (file.path)}
      <button type="button" class="item" class:missing={!file.exists} title={file.display} onclick={() => onpick(file)}>
        <Icon name={file.encrypted === false ? "unlock" : "lock"} size={14} />
        <span class="name">{file.name}</span>
        <!-- На телефоне все хранилища в папке приложения: путь ничего не говорит. -->
        {#if !file.exists}
          <span class="where mono">файл не найден</span>
        {:else if !IS_MOBILE}
          <span class="where mono">{file.display}</span>
        {/if}
      </button>
    {/each}
  </div>
{/if}

<style>
  .recent {
    width: 100%;
    max-width: 400px;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .title {
    padding: 0 10px 6px;
    font-size: 12px;
    color: var(--text-subtle);
  }

  .item {
    display: grid;
    grid-template-columns: auto auto minmax(0, 1fr);
    align-items: center;
    gap: 8px;
    min-height: 36px;
    padding: 0 10px;
    border-radius: var(--radius);
    color: var(--text-muted);
  }

  .item:hover {
    background: var(--bg-hover-strong);
    color: var(--text);
  }

  .item.missing {
    opacity: 0.6;
  }

  .name {
    font-weight: 500;
  }

  .where {
    font-size: 11px;
    color: var(--text-subtle);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    text-align: right;
  }
</style>
