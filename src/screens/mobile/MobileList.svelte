<script lang="ts">
  // Список записей на телефоне: шапка со статусом синхронизации, замком и настройками,
  // под ней — та же панель, что на компьютере: поиск, теги, избранное, разделы
  // с подразделами и «Новая запись». Запись открывается отдельным экраном.
  import type { EntryView } from "../../lib/api";
  import { cloud, refreshCloud, statusText } from "../../lib/cloud.svelte";
  import { go } from "../../lib/nav.svelte";
  import IconButton from "../../lib/ui/IconButton.svelte";
  import Logo from "../../lib/ui/Logo.svelte";
  import { lockVault } from "../../lib/vault.svelte";
  import Sidebar from "../main/Sidebar.svelte";

  interface Props {
    /// Записи после поиска и тега.
    visible: EntryView[];
    query: string;
    tag: string;
  }

  let { visible, query = $bindable(), tag = $bindable() }: Props = $props();

  refreshCloud();

  /// Статус синхронизации — только если хранилище связано с облаком.
  let status = $derived(cloud.view?.linked ? { text: statusText(cloud.view.status), kind: cloud.view.status.kind } : null);
</script>

<div class="screen">
  <header>
    <div class="heading">
      <Logo />
      {#if status}
        <span class="status">
          <span class="dot {status.kind}" aria-hidden="true"></span>
          {status.text}
        </span>
      {/if}
    </div>
    <div class="actions">
      <IconButton icon="lock" label="Заблокировать" onclick={lockVault} />
      <IconButton icon="settings" label="Настройки" onclick={() => go("settings")} />
    </div>
  </header>
  <Sidebar {visible} bind:query bind:tag />
</div>

<style>
  /* Во всю высоту окна: #app — не flex, без высоты список растянул бы страницу. */
  .screen {
    height: 100%;
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    background: var(--bg-chrome);
  }

  header {
    flex: none;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 12px 12px 4px 16px;
  }

  .heading {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }

  .status {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 12px;
    color: var(--text-muted);
  }

  .dot {
    width: 7px;
    height: 7px;
    flex: none;
    border-radius: 50%;
    background: var(--text-subtle);
  }

  .dot.synced {
    background: #5bc2a8;
  }

  .dot.syncing {
    background: var(--info);
  }

  .dot.offline,
  .dot.needsLogin,
  .dot.failed {
    background: var(--danger);
  }

  .actions {
    display: flex;
    gap: 4px;
  }

  /* Панель под шапкой занимает остальную высоту. */
  .screen > :global(aside) {
    flex: 1;
    min-height: 0;
  }
</style>
