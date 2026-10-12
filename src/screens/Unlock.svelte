<script lang="ts">
  import { onDestroy } from "svelte";
  import { api, errorText, isCommandError } from "../lib/api";
  import { app, go } from "../lib/nav.svelte";
  import { plural } from "../lib/text";
  import Button from "../lib/ui/Button.svelte";
  import FileChip from "../lib/ui/FileChip.svelte";
  import Icon from "../lib/ui/Icon.svelte";
  import Logo from "../lib/ui/Logo.svelte";
  import PasswordInput from "../lib/ui/PasswordInput.svelte";

  let password = $state("");
  let busy = $state(false);
  let error = $state<string | null>(null);
  let wrong = $state(false);
  let retryUntil = $state(0);
  let now = $state(Date.now());

  const timer = setInterval(() => (now = Date.now()), 250);
  onDestroy(() => clearInterval(timer));

  let retryLeft = $derived(Math.max(0, Math.ceil((retryUntil - now) / 1000)));
  let missing = $derived(app.vault !== null && !app.vault.exists);
  /// Открытое хранилище открывается без пароля.
  let plain = $derived(app.vault?.encrypted === false);
  let canSubmit = $derived(
    app.vault !== null && !missing && (plain || password.length > 0) && !busy && retryLeft === 0,
  );

  async function submit(event: SubmitEvent) {
    event.preventDefault();
    if (!canSubmit || !app.vault) return;
    busy = true;
    error = null;
    try {
      app.protection = await api.unlock(app.vault.path, plain ? null : password);
      password = "";
      go("main");
    } catch (err) {
      if (isCommandError(err) && (err.kind === "wrongPassphrase" || err.kind === "tooSoon")) {
        wrong = true;
        retryUntil = Date.now() + err.retryInMs;
      } else if (isCommandError(err) && err.kind === "damaged") {
        app.fileErrorDetail = err.detail;
        go("file-error");
      } else {
        error = errorText(err);
      }
    } finally {
      busy = false;
    }
  }

  async function pickFile() {
    const file = await api.pickExistingVault();
    if (file) {
      app.vault = file;
      wrong = false;
      error = null;
    }
  }
</script>

<div class="screen">
  <Logo size="lg" vertical />

  <form class="card" onsubmit={submit}>
    <FileChip label={app.vault?.name ?? "Файл не выбран"} onpick={pickFile} />
    {#if missing}
      <p class="error" role="alert">
        <Icon name="error" />
        <span>Файл не найден: {app.vault?.display}. Выберите другой файл.</span>
      </p>
    {/if}

    {#if plain}
      <p class="plain-note">
        <Icon name="warning" color="var(--danger)" />
        <span>Хранилище без шифрования: пароль не нужен, файл может прочитать любой.</span>
      </p>
    {:else}
    <div class="field">
      <label for="master">Мастер-пароль</label>
      <PasswordInput
        id="master"
        bind:value={password}
        placeholder="Введите мастер-пароль"
        invalid={wrong}
        describedby={wrong ? "unlock-error" : undefined}
        autofocus
      />
      {#if wrong}
        <p id="unlock-error" class="error" role="alert">
          <Icon name="error" />
          <span>
            Неверный мастер-пароль.
            {#if retryLeft > 0}
              Следующая попытка через {retryLeft}
              {plural(retryLeft, ["секунду", "секунды", "секунд"])}.
            {/if}
          </span>
        </p>
      {/if}
    </div>
    {/if}
    {#if error}
      <p class="error" role="alert"><Icon name="error" /><span>{error}</span></p>
    {/if}

    <Button variant="primary" type="submit" icon="unlock" full disabled={!canSubmit}>
      {busy ? "Проверяем…" : plain ? "Открыть" : "Разблокировать"}
    </Button>
    <Button variant="ghost" size="sm" full onclick={() => go("welcome")}>Создать новое хранилище</Button>
  </form>

  <p class="hint">
    {#if wrong}
      Мастер-пароль нельзя восстановить. Если он утерян, данные из файла не вернуть
    {:else}
      Файл расшифровывается только в памяти{app.settings.autoLockMinutes > 0
        ? ` и блокируется автоматически через ${app.settings.autoLockMinutes} ${plural(app.settings.autoLockMinutes, ["минуту", "минуты", "минут"])} бездействия`
        : ""}
    {/if}
  </p>
</div>

<style>
  .screen {
    height: 100%;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 28px;
    padding: 24px;
  }

  .card {
    width: 100%;
    max-width: 400px;
    display: flex;
    flex-direction: column;
    gap: 20px;
    padding: 28px;
    border-radius: var(--radius-card);
    background: var(--bg-panel);
    border: 1px solid var(--line);
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  label {
    font-weight: 500;
  }

  .error {
    display: flex;
    align-items: flex-start;
    gap: 8px;
    color: var(--danger-text);
    font-size: 13px;
  }

  .error :global(svg) {
    margin-top: 1px;
  }

  .plain-note {
    display: flex;
    gap: 8px;
    align-items: flex-start;
    color: var(--danger-text);
    font-size: 13px;
  }

  .hint {
    max-width: 400px;
    text-align: center;
    font-size: 12px;
    color: var(--text-subtle);
  }
</style>
