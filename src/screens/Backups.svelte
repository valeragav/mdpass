<script lang="ts">
  import { onMount } from "svelte";
  import {
    api,
    errorText,
    isCommandError,
    type BackupChange,
    type BackupKind,
    type BackupView,
    type FieldChange,
  } from "../lib/api";
  import { app, go } from "../lib/nav.svelte";
  import { showToast } from "../lib/toast.svelte";
  import Button from "../lib/ui/Button.svelte";
  import Icon from "../lib/ui/Icon.svelte";
  import IconButton from "../lib/ui/IconButton.svelte";
  import Logo from "../lib/ui/Logo.svelte";
  import PasswordInput from "../lib/ui/PasswordInput.svelte";
  import { loadEntries } from "../lib/vault.svelte";

  /// Файл не открывается: восстанавливаем без открытого хранилища, по паролю.
  const damaged = app.backupsFrom === "file-error";
  const vaultPath = damaged ? app.vault?.path : undefined;

  const REASONS: Record<BackupKind, string> = {
    add: "Перед добавлением записи",
    update: "Перед изменением записи",
    delete: "Перед удалением записи",
    passphrase: "Перед сменой мастер-пароля",
    restore: "Перед восстановлением копии",
    import: "Перед импортом",
    move: "Перед перемещением",
  };

  const dateFormat = new Intl.DateTimeFormat("ru", {
    day: "numeric",
    month: "short",
    hour: "2-digit",
    minute: "2-digit",
  });
  const formatDate = (ms: number) => dateFormat.format(new Date(ms));

  let backups = $state<BackupView[]>([]);
  let selectedId = $state<string | null>(null);
  let loading = $state(true);
  let error = $state<string | null>(null);

  let preview = $state<BackupChange[] | null>(null);
  let previewError = $state<string | null>(null);
  let previewing = $state(false);
  let otherPassphrase = $state(false);

  let password = $state("");
  let busy = $state(false);

  let selected = $derived(backups.find((b) => b.id === selectedId) ?? null);

  onMount(async () => {
    try {
      backups = await api.listBackups(vaultPath);
      if (backups[0]) select(backups[0].id);
    } catch (err) {
      error = errorText(err);
    } finally {
      loading = false;
    }
  });

  /// Показанные прежние значения секретов: ключ `${номер изменения}:${ключ поля}`.
  let revealed = $state<Record<string, string>>({});

  let previewToken = 0;
  async function select(id: string) {
    selectedId = id;
    error = null;
    revealed = {};
    if (damaged) return;
    // Расшифровка занимает около секунды: устаревший ответ не должен перебить новый.
    const token = ++previewToken;
    preview = null;
    previewError = null;
    otherPassphrase = false;
    previewing = true;
    try {
      const changes = await api.previewBackup(id);
      if (token === previewToken) preview = changes;
    } catch (err) {
      if (token !== previewToken) return;
      otherPassphrase = isCommandError(err) && err.kind === "otherPassphrase";
      previewError = errorText(err);
    } finally {
      if (token === previewToken) previewing = false;
    }
  }

  function changeText(change: BackupChange): { sign: string; tone: string; text: string } {
    switch (change.kind) {
      case "added":
        return { sign: "+", tone: "added", text: `«${change.title}» вернётся` };
      case "removed":
        return { sign: "−", tone: "removed", text: `«${change.title}» будет удалена: в этой копии её нет` };
      case "changed": {
        const keys = change.fields.map((f) => f.key).join(", ");
        const what = change.fields.length === 1 ? `поле ${keys} вернётся` : `поля ${keys} вернутся`;
        return { sign: "~", tone: "changed", text: `«${change.title}»: ${what} к прежнему значению` };
      }
    }
  }

  // `tags` и `favorite` — не поля, а строки записи: их показывают как есть, без «показать».
  const isMark = (key: string) => key === "tags" || key === "favorite";

  function fieldValue(change: number, field: FieldChange): string {
    if (field.missing) return "в копии нет — пропадёт";
    if (field.secret) return revealed[`${change}:${field.key}`] ?? "••••••••";
    return field.before || "пусто";
  }

  async function toggleOld(i: number, change: BackupChange, field: FieldChange) {
    const key = `${i}:${field.key}`;
    if (key in revealed) {
      delete revealed[key];
      return;
    }
    try {
      revealed[key] = await api.revealBackupField(selected!.id, change.path, change.title, field.key);
    } catch (err) {
      error = errorText(err);
    }
  }

  async function copyOld(change: BackupChange, field: FieldChange) {
    try {
      await api.copyBackupField(selected!.id, change.path, change.title, field.key);
      showToast(`Скопировано прежнее значение ${field.key}`);
    } catch (err) {
      error = errorText(err);
    }
  }

  let restoringEntry = $state<string | null>(null);

  /// Возвращает из копии одну запись. Перед этим, как и перед любой записью файла,
  /// создаётся копия, поэтому список копий перечитывается.
  async function restoreEntry(change: BackupChange) {
    if (!selected || restoringEntry) return;
    const id = selected.id;
    restoringEntry = change.title;
    error = null;
    try {
      await api.restoreBackupEntry(id, change.path, change.title);
      await loadEntries();
      showToast(`Запись «${change.title}» восстановлена из копии`);
      backups = await api.listBackups();
      await select(id);
    } catch (err) {
      error = errorText(err);
    } finally {
      restoringEntry = null;
    }
  }

  async function restore(event?: SubmitEvent) {
    event?.preventDefault();
    if (!selected || busy) return;
    busy = true;
    error = null;
    try {
      if (damaged && vaultPath) {
        await api.restoreDamagedVault(vaultPath, selected.id, password || null);
        password = "";
      } else {
        await api.restoreBackup(selected.id);
      }
      await loadEntries();
      showToast(`Восстановлена копия от ${formatDate(selected.createdMs)}`);
      go("main");
    } catch (err) {
      error = errorText(err);
    } finally {
      busy = false;
    }
  }

  async function exportFile() {
    if (!selected) return;
    try {
      if (await api.exportBackup(selected.id, vaultPath)) showToast("Копия сохранена в файл");
    } catch (err) {
      error = errorText(err);
    }
  }

  let canRestore = $derived(
    selected !== null &&
      !busy &&
      (damaged || (preview !== null && preview.length > 0 && !previewing)),
  );
</script>

<div class="screen">
  <header>
    <Button variant="ghost" size="sm" icon="back" onclick={() => go(app.backupsFrom)}>
      {app.backupsFrom === "settings" ? "Настройки" : "Назад"}
    </Button>
    <Logo size="sm" />
    <span class="spacer"></span>
  </header>

  <div class="body">
    <div class="list-pane">
      <div class="list-head">
        <h1>Резервные копии</h1>
        <span>Каждая копия зашифрована тем мастер-паролем, который был в момент копирования</span>
      </div>
      <div class="list scroll">
        {#if loading}
          <p class="muted">Загружаем список…</p>
        {:else if backups.length === 0}
          <p class="muted">Копий пока нет. Они появятся после первой записи в хранилище.</p>
        {/if}
        {#each backups as backup (backup.id)}
          <button
            type="button"
            class="item"
            class:on={backup.id === selectedId}
            aria-pressed={backup.id === selectedId}
            onclick={() => select(backup.id)}
          >
            <span class="date mono">{formatDate(backup.createdMs)}</span>
            <span>{REASONS[backup.kind]}</span>
          </button>
        {/each}
      </div>
    </div>

    <div class="details scroll">
      {#if selected}
        <form class="inner" onsubmit={restore}>
          <div class="title">
            <div class="mono when">Копия от {formatDate(selected.createdMs)}</div>
            <h2>{REASONS[selected.kind]}</h2>
          </div>

          {#if damaged}
            <p class="muted">
              Файл {app.vault?.name} не открывается. Копия заменит его, а повреждённый файл останется рядом с
              пометкой «damaged».
            </p>
            <div class="field">
              <label for="restore-pass">Мастер-пароль этой копии</label>
              <PasswordInput
                id="restore-pass"
                bind:value={password}
                placeholder="Оставьте пустым, если копия без шифрования"
                autofocus
              />
            </div>
          {:else}
            <div class="group">
              <div class="label">Что изменится при восстановлении</div>
              {#if previewing}
                <p class="muted">Расшифровываем копию…</p>
              {:else if previewError}
                <p class="warn" role="alert"><Icon name="warning" /> <span>{previewError}</span></p>
                {#if otherPassphrase}
                  <p class="muted">Её можно сохранить как файл и открыть прежним паролем.</p>
                {/if}
              {:else if preview && preview.length === 0}
                <p class="muted">Копия совпадает с текущей версией: восстанавливать нечего.</p>
              {:else if preview}
                <div class="changes">
                  {#each preview as change, i (i)}
                    {@const c = changeText(change)}
                    <div class="change">
                      <span class="sign mono {c.tone}">{c.sign}</span>
                      <div class="change-body">
                        <div class="change-head">
                          <span>{c.text}</span>
                          {#if change.kind !== "removed"}
                            <Button
                              size="sm"
                              icon="generate"
                              disabled={restoringEntry !== null}
                              onclick={() => restoreEntry(change)}
                            >
                              {restoringEntry === change.title ? "Возвращаем…" : "Вернуть запись"}
                            </Button>
                          {/if}
                        </div>
                        {#if change.kind === "changed"}
                          <div class="old-fields">
                            {#each change.fields as field (field.key)}
                              <div class="old" aria-label={`Прежнее значение ${field.key}`}>
                                <span class="old-key mono">{field.key}</span>
                                <span class="old-value mono" class:faint={field.missing}>{fieldValue(i, field)}</span>
                                {#if !field.missing && !isMark(field.key)}
                                  <span class="old-actions">
                                    {#if field.secret}
                                      {@const shown = `${i}:${field.key}` in revealed}
                                      <IconButton
                                        icon={shown ? "eye-off" : "eye"}
                                        size="sm"
                                        label={`${shown ? "Скрыть" : "Показать"} прежний ${field.key}`}
                                        onclick={() => toggleOld(i, change, field)}
                                      />
                                    {/if}
                                    <IconButton
                                      icon="copy"
                                      size="sm"
                                      label={`Скопировать прежний ${field.key}`}
                                      onclick={() => copyOld(change, field)}
                                    />
                                  </span>
                                {/if}
                              </div>
                            {/each}
                          </div>
                        {/if}
                      </div>
                    </div>
                  {/each}
                </div>
                <p class="muted small">
                  «Вернуть запись» меняет только эту запись. «Восстановить эту копию» возвращает всё хранилище целиком.
                </p>
              {/if}
            </div>
          {/if}

          <div class="note">
            <Icon name="warning" size={18} color="var(--accent)" />
            <span>
              {damaged
                ? "Повреждённый файл не удаляется: его можно отдать на проверку или открыть позже."
                : "Текущая версия сохранится как новая копия, так что восстановление можно отменить."}
            </span>
          </div>

          {#if error}
            <p class="warn" role="alert"><Icon name="error" /> <span>{error}</span></p>
          {/if}

          <div class="actions">
            <Button variant="primary" type="submit" icon="generate" disabled={!canRestore}>
              {busy ? "Восстанавливаем…" : "Восстановить эту копию"}
            </Button>
            <Button onclick={exportFile}>Сохранить как файл…</Button>
          </div>
        </form>
      {/if}
    </div>
  </div>
</div>

<style>
  .screen {
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
    padding: 0 16px;
    border-bottom: 1px solid var(--line);
    background: var(--bg-chrome);
  }

  header :global(.btn) {
    height: 36px;
  }

  .spacer {
    width: 120px;
  }

  .body {
    flex: 1;
    min-height: 0;
    display: flex;
  }

  .list-pane {
    width: 480px;
    flex: none;
    display: flex;
    flex-direction: column;
    min-height: 0;
    border-right: 1px solid var(--line);
    background: var(--bg-chrome);
  }

  .list-head {
    padding: 28px 24px 16px;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  h1 {
    font-size: 24px;
    font-weight: 600;
    letter-spacing: -0.01em;
  }

  .list-head span {
    font-size: 13px;
    color: var(--text-subtle);
  }

  .list {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: 0 12px 16px;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .item {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 12px 14px;
    border-radius: var(--radius);
  }

  .item:hover {
    background: var(--bg-hover);
  }

  .item.on {
    background: color-mix(in srgb, var(--accent) 12%, transparent);
  }

  .date {
    font-size: 13px;
    color: var(--text-subtle);
  }

  .item.on .date {
    color: var(--accent);
  }

  .details {
    flex: 1;
    min-width: 0;
    overflow-y: auto;
    padding: 32px 48px 40px;
  }

  .inner {
    max-width: 640px;
    display: flex;
    flex-direction: column;
    gap: 24px;
  }

  .title {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .when {
    font-size: 12px;
    color: var(--text-subtle);
  }

  h2 {
    font-size: 22px;
    font-weight: 600;
    letter-spacing: -0.01em;
  }

  .group,
  .field {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .label,
  label {
    font-weight: 500;
  }

  .changes {
    display: flex;
    flex-direction: column;
    gap: 1px;
    background: var(--line);
    border: 1px solid var(--line);
    border-radius: 10px;
    overflow: hidden;
  }

  .change {
    display: flex;
    align-items: flex-start;
    gap: 14px;
    padding: 14px 16px;
    background: var(--bg-panel);
  }

  .sign {
    width: 20px;
    flex: none;
    font-weight: 500;
  }

  .change-body {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .change-head {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 12px;
  }

  .change-head :global(.btn) {
    flex: none;
  }

  .old-fields {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .old {
    display: grid;
    grid-template-columns: 120px minmax(0, 1fr) auto;
    align-items: center;
    gap: 12px;
    min-height: 36px;
    padding: 0 4px 0 12px;
    border-radius: 6px;
    background: var(--bg-chrome);
  }

  .old-key {
    font-size: 12px;
    color: var(--text-subtle);
  }

  .old-value {
    font-size: 13px;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
    padding: 8px 0;
  }

  .old-value.faint {
    color: var(--text-faint);
  }

  .old-actions {
    display: flex;
  }

  .small {
    font-size: 12px;
  }

  .sign.added {
    color: var(--info);
  }

  .sign.removed {
    color: var(--danger);
  }

  .sign.changed {
    color: var(--accent);
  }

  .note {
    display: flex;
    gap: 12px;
    padding: 12px 14px;
    border-radius: var(--radius);
    background: var(--bg-chrome);
    border: 1px dashed var(--line-dashed);
    color: var(--text-2);
    font-size: 13px;
  }

  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: 10px;
  }

  .muted {
    color: var(--text-subtle);
  }

  .warn {
    display: flex;
    gap: 8px;
    align-items: flex-start;
    color: var(--danger-text);
  }

  @media (max-width: 900px) {
    .list-pane {
      width: 300px;
    }
  }
</style>
