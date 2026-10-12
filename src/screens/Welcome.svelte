<script lang="ts">
  import { api, errorText, isCommandError, type VaultFile } from "../lib/api";
  import { app, go } from "../lib/nav.svelte";
  import { MIN_MASTER_LENGTH, strength } from "../lib/text";
  import Button from "../lib/ui/Button.svelte";
  import FileChip from "../lib/ui/FileChip.svelte";
  import Icon from "../lib/ui/Icon.svelte";
  import Logo from "../lib/ui/Logo.svelte";
  import PasswordInput from "../lib/ui/PasswordInput.svelte";

  let file = $state<VaultFile | null>(app.defaultVault);
  let master = $state("");
  let repeat = $state("");
  let busy = $state(false);
  let error = $state<string | null>(null);
  let existing = $state(false);

  let meter = $derived(strength(master));
  let tooShort = $derived(master.length > 0 && master.length < MIN_MASTER_LENGTH);
  let mismatch = $derived(repeat.length > 0 && repeat !== master);
  let canCreate = $derived(file !== null && master.length >= MIN_MASTER_LENGTH && repeat === master && !busy);
  let barColor = $derived(meter.score <= 1 ? "var(--danger)" : meter.score === 2 ? "var(--accent)" : "var(--info)");

  async function pickFile() {
    const picked = await api.pickNewVault(file?.path);
    if (picked) {
      file = picked;
      error = null;
      existing = false;
    }
  }

  async function openExisting() {
    const picked = await api.pickExistingVault();
    if (picked) {
      app.vault = picked;
      go("unlock");
    }
  }

  /// «У меня уже есть .md-файл»: сначала создаём хранилище, потом импорт как шаг 2.
  async function createAndImport() {
    if (!canCreate) {
      error = "Сначала задайте файл хранилища и мастер-пароль: импорт пойдёт в новое хранилище";
      existing = false;
      return;
    }
    if (await createVault()) {
      app.importFrom = "welcome";
      go("import");
    }
  }

  async function create(event: SubmitEvent) {
    event.preventDefault();
    if (await createVault()) go("main");
  }

  async function createVault(): Promise<boolean> {
    if (!canCreate || !file) return false;
    busy = true;
    error = null;
    existing = false;
    try {
      await api.createVault(file.path, master);
      master = "";
      repeat = "";
      app.vault = await api.describePath(file.path);
      return true;
    } catch (err) {
      existing = isCommandError(err) && err.kind === "alreadyExists";
      error = errorText(err);
      return false;
    } finally {
      busy = false;
    }
  }

  function unlockExisting() {
    app.vault = file;
    go("unlock");
  }
</script>

<div class="screen scroll">
  <Logo />

  <form class="card" onsubmit={create}>
    <div class="intro">
      <h1>Создайте хранилище</h1>
      <p>Все записи лежат в одном зашифрованном файле. Внутри обычный Markdown.</p>
    </div>

    <div class="field">
      <span class="label">Файл хранилища</span>
      <FileChip label={file?.display ?? "Выберите, где сохранить файл"} onpick={pickFile} pickLabel="Выбрать место" />
      {#if error}
        <p class="error" role="alert">
          <Icon name="error" />
          <span>
            {error}
            {#if existing}
              <button type="button" class="link" onclick={unlockExisting}>Открыть этот файл</button>
            {/if}
          </span>
        </p>
      {/if}
    </div>

    <div class="field">
      <label for="m1">Мастер-пароль</label>
      <PasswordInput id="m1" bind:value={master} placeholder="Лучше длинная фраза из нескольких слов" autofocus />
      <div class="meter">
        <div class="bars" aria-hidden="true">
          {#each [0, 1, 2, 3] as i (i)}
            <div class="bar" style:background={i < meter.score ? barColor : "var(--bg-hover-strong)"}></div>
          {/each}
        </div>
        <span class="meter-label" aria-live="polite">{meter.label}</span>
      </div>
      {#if tooShort}
        <span class="note">Не короче {MIN_MASTER_LENGTH} символов</span>
      {/if}
    </div>

    <div class="field">
      <label for="m2">Повторите пароль</label>
      <PasswordInput id="m2" bind:value={repeat} placeholder="Ещё раз" invalid={mismatch} />
      {#if mismatch}
        <span class="note bad">Пароли не совпадают</span>
      {/if}
    </div>

    <div class="warning">
      <Icon name="warning" size={18} color="var(--accent)" />
      <span>Мастер-пароль нельзя восстановить. Если вы его забудете, расшифровать данные не получится.</span>
    </div>

    <div class="actions">
      <Button variant="primary" type="submit" full disabled={!canCreate}>
        {busy ? "Создаём…" : "Создать хранилище"}
      </Button>
      <Button icon="import" full onclick={createAndImport}>У меня уже есть .md-файл</Button>
      <Button variant="ghost" size="sm" full onclick={openExisting}>Открыть существующее хранилище</Button>
    </div>
  </form>
</div>

<style>
  .screen {
    height: 100%;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 24px;
    padding: 24px;
    overflow-y: auto;
  }

  .card {
    width: 100%;
    max-width: 480px;
    display: flex;
    flex-direction: column;
    gap: 18px;
    padding: 32px;
    border-radius: var(--radius-card);
    background: var(--bg-panel);
    border: 1px solid var(--line);
  }

  .intro {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  h1 {
    font-size: 22px;
    font-weight: 600;
    letter-spacing: -0.01em;
  }

  .intro p {
    color: var(--text-muted);
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  label,
  .label {
    font-weight: 500;
  }

  .meter {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .bars {
    flex: 1;
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    gap: 4px;
  }

  .bar {
    height: 6px;
    border-radius: 3px;
  }

  .meter-label {
    width: 80px;
    font-size: 12px;
    color: var(--text-muted);
    text-align: right;
  }

  .note {
    font-size: 12px;
    color: var(--text-subtle);
  }

  .note.bad {
    color: var(--danger-text);
  }

  .error {
    display: flex;
    align-items: flex-start;
    gap: 8px;
    color: var(--danger-text);
    font-size: 13px;
  }

  .link {
    color: var(--accent);
    text-decoration: underline;
  }

  .warning {
    display: flex;
    gap: 12px;
    padding: 12px 14px;
    border-radius: var(--radius);
    background: var(--bg-chrome);
    border: 1px dashed var(--line-dashed);
    color: var(--text-2);
    font-size: 13px;
  }

  .warning :global(svg) {
    margin-top: 1px;
  }

  .actions {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
</style>
