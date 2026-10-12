<script lang="ts">
  import type { Snippet } from "svelte";
  import { app, go } from "../nav.svelte";
  import { lockVault } from "../vault.svelte";
  import Button from "./Button.svelte";
  import Icon from "./Icon.svelte";
  import IconButton from "./IconButton.svelte";
  import Logo from "./Logo.svelte";

  interface Props {
    /// Текст справа в подвале, например «12 записей».
    status: string;
    children: Snippet;
  }

  let { status, children }: Props = $props();
</script>

<div class="shell">
  <header>
    <Logo size="sm" />
    <div class="file mono">
      <Icon name="lock" size={14} />
      <span>{app.vault?.name}</span>
      <span class="dot">·</span>
      <span>зашифровано</span>
    </div>
    <div class="actions">
      <IconButton icon="settings" label="Настройки" bordered size="sm" onclick={() => go("settings")} />
      <Button size="sm" icon="lock" onclick={lockVault}>Заблокировать</Button>
    </div>
  </header>

  <div class="body">
    {@render children()}
  </div>

  <footer class="mono">
    <div class="left">
      <Icon name="lock-memory" size={14} />
      <span>Расшифровано только в памяти</span>
      <span class="dot">·</span>
      <span>{status}</span>
    </div>
    <span>
      {app.settings.autoLockMinutes > 0 ? `автоблокировка через ${app.settings.autoLockMinutes} мин` : "автоблокировка выключена"}
    </span>
  </footer>
</div>

<style>
  .shell {
    height: 100%;
    display: flex;
    flex-direction: column;
  }

  header {
    height: 48px;
    flex: none;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    padding: 0 16px;
    border-bottom: 1px solid var(--line);
    background: var(--bg-chrome);
  }

  .file {
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 14px;
    border-radius: 999px;
    border: 1px solid var(--line);
    background: var(--bg-panel);
    font-size: 12px;
    color: var(--text-subtle);
    white-space: nowrap;
    overflow: hidden;
  }

  .dot {
    color: var(--text-ghost);
  }

  .actions {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .actions :global(.btn) {
    height: 36px;
  }

  .body {
    flex: 1;
    min-height: 0;
    display: flex;
  }

  footer {
    height: 32px;
    flex: none;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    padding: 0 16px;
    border-top: 1px solid var(--line);
    background: var(--bg-chrome);
    font-size: 12px;
    color: var(--text-subtle);
    white-space: nowrap;
  }

  .left {
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
    overflow: hidden;
  }

  @media (max-width: 720px) {
    .file {
      display: none;
    }
  }
</style>
