<script lang="ts">
  import { onMount } from "svelte";
  import { api } from "../lib/api";
  import { app, go } from "../lib/nav.svelte";
  import Button from "../lib/ui/Button.svelte";
  import Icon from "../lib/ui/Icon.svelte";
  import Logo from "../lib/ui/Logo.svelte";

  let detailsOpen = $state(true);
  let hasBackups = $state(false);

  onMount(async () => {
    if (!app.vault) return;
    try {
      hasBackups = (await api.listBackups(app.vault.path)).length > 0;
    } catch {
      hasBackups = false;
    }
  });

  function openBackups() {
    app.backupsFrom = "file-error";
    go("backups");
  }

  async function pickFile() {
    const file = await api.pickExistingVault();
    if (file) {
      app.vault = file;
      go("unlock");
    }
  }
</script>

<div class="screen">
  <Logo />

  <div class="card" role="alert">
    <div class="head">
      <Icon name="warning" size={28} stroke={1.5} color="var(--danger)" />
      <h1>Не удалось открыть {app.vault?.name}</h1>
      <p>Проверка целостности не прошла. Файл повреждён или был изменён вне приложения. Это не похоже на неверный пароль.</p>
    </div>

    <div class="actions">
      {#if hasBackups}
        <Button variant="primary" icon="generate" full onclick={openBackups}>Восстановить из резервной копии</Button>
      {/if}
      <Button icon="file" full onclick={pickFile}>Выбрать другой файл</Button>
      <Button variant="ghost" size="sm" full onclick={() => go("unlock")}>Попробовать ещё раз</Button>
    </div>

    <div class="details">
      <button
        type="button"
        class="toggle"
        aria-expanded={detailsOpen}
        aria-controls="file-error-details"
        onclick={() => (detailsOpen = !detailsOpen)}
      >
        <span class="arrow" class:open={detailsOpen} aria-hidden="true">▸</span>
        Подробности
      </button>
      {#if detailsOpen}
        <dl id="file-error-details" class="mono">
          <dt>файл:</dt>
          <dd>{app.vault?.display}</dd>
          <dt>ошибка:</dt>
          <dd>{app.fileErrorDetail}</dd>
        </dl>
      {/if}
    </div>
  </div>

  <p class="hint">Повреждённый файл не перезаписывается. Пока вы не выберете действие, он остаётся как есть</p>
</div>

<style>
  .screen {
    height: 100%;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 24px;
    padding: 24px;
    overflow-y: auto;
  }

  /* По центру, пока помещается. Если экран ниже содержимого, оно начинается сверху
     и прокручивается целиком: justify-content: center увёл бы верх за край. */
  .screen > :global(:first-child) {
    margin-top: auto;
  }

  .screen > :global(:last-child) {
    margin-bottom: auto;
  }

  .card {
    width: 100%;
    max-width: 480px;
    display: flex;
    flex-direction: column;
    gap: 22px;
    padding: 32px;
    border-radius: var(--radius-card);
    background: var(--bg-panel);
    border: 1px solid var(--line);
  }

  .head {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  h1 {
    font-size: 22px;
    font-weight: 600;
    letter-spacing: -0.01em;
    overflow-wrap: anywhere;
  }

  .head p {
    color: var(--text-muted);
  }

  .actions {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .details {
    border-top: 1px solid var(--line);
    padding-top: 14px;
  }

  .toggle {
    display: flex;
    align-items: center;
    gap: 6px;
    color: var(--text-muted);
    font-weight: 500;
    min-height: 32px;
  }

  .arrow {
    display: inline-block;
    transition: transform 0.15s;
  }

  .arrow.open {
    transform: rotate(90deg);
  }

  dl {
    margin: 10px 0 0;
    display: grid;
    grid-template-columns: auto minmax(0, 1fr);
    gap: 6px 12px;
    font-size: 12px;
    color: var(--text-muted);
  }

  dt {
    color: var(--text-subtle);
  }

  dd {
    margin: 0;
    overflow-wrap: anywhere;
  }

  .hint {
    max-width: 480px;
    text-align: center;
    font-size: 12px;
    color: var(--text-subtle);
  }
</style>
