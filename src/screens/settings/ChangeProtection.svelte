<script lang="ts">
  import { api, errorText, isCommandError } from "../../lib/api";
  import { app } from "../../lib/nav.svelte";
  import { showToast } from "../../lib/toast.svelte";
  import { MIN_MASTER_LENGTH, strength } from "../../lib/text";
  import Button from "../../lib/ui/Button.svelte";
  import ConfirmDialog from "../../lib/ui/ConfirmDialog.svelte";
  import PasswordInput from "../../lib/ui/PasswordInput.svelte";
  import ProtectionPicker, { PROTECTION_LABEL, type Protection } from "../../lib/ui/ProtectionPicker.svelte";

  let { open, onclose }: { open: boolean; onclose: () => void } = $props();

  // Открытое хранилище: диалог открыт ради шифрования, поэтому сразу предлагаем надёжное.
  const startWith = (): Protection => (app.protection && app.protection !== "none" ? app.protection : "strong");
  let protection = $state<Protection>(startWith());
  let current = $state("");
  let next = $state("");
  let repeat = $state("");
  let busy = $state(false);
  let error = $state<string | null>(null);
  let wrongCurrent = $state(false);

  // При каждом открытии диалог начинает с текущей защиты.
  $effect(() => {
    if (open) protection = startWith();
  });

  let encryptedNow = $derived(app.protection !== null && app.protection !== "none");
  let encryptNext = $derived(protection !== "none");
  let meter = $derived(strength(next));
  let mismatch = $derived(repeat.length > 0 && repeat !== next);
  let canSave = $derived(
    !busy &&
      (encryptedNow || encryptNext) &&
      (!encryptedNow || current.length > 0) &&
      (!encryptNext || (next.length >= MIN_MASTER_LENGTH && repeat === next)),
  );

  function close() {
    current = next = repeat = "";
    error = null;
    wrongCurrent = false;
    onclose();
  }

  async function save() {
    if (!canSave) return;
    busy = true;
    error = null;
    wrongCurrent = false;
    try {
      const before = app.vault?.name;
      const changed = await api.changeProtection(encryptedNow ? current : null, encryptNext ? next : null, protection);
      app.protection = changed.protection;
      app.vault = changed.vault;
      const what = encryptNext ? `${PROTECTION_LABEL[protection]}: файл зашифрован заново` : "Шифрование выключено";
      showToast(changed.vault.name !== before ? `${what}, файл теперь ${changed.vault.name}` : what);
      close();
    } catch (err) {
      wrongCurrent = isCommandError(err) && (err.kind === "wrongPassphrase" || err.kind === "tooSoon");
      error = wrongCurrent ? "Текущий мастер-пароль неверный. Попробуйте через 3 секунды" : errorText(err);
    } finally {
      busy = false;
    }
  }
</script>

<ConfirmDialog {open} title="Защита файла" icon="lock" oncancel={close}>
  <form
    class="form"
    onsubmit={(e) => {
      e.preventDefault();
      save();
    }}
  >
    <ProtectionPicker bind:value={protection} />

    {#if encryptedNow}
      <div class="field">
        <label for="cp-current">Текущий мастер-пароль</label>
        <PasswordInput id="cp-current" bind:value={current} invalid={wrongCurrent} autofocus />
      </div>
    {/if}

    {#if encryptNext}
      <div class="field">
        <label for="cp-new">{encryptedNow ? "Новый мастер-пароль" : "Мастер-пароль"}</label>
        <PasswordInput id="cp-new" bind:value={next} placeholder={`Не короче ${MIN_MASTER_LENGTH} символов`} />
        {#if next}<span class="note">Надёжность пароля: {meter.label.toLowerCase() || "—"}</span>{/if}
        {#if encryptedNow}<span class="note">Можно ввести тот же пароль, чтобы сменить только силу шифрования.</span>{/if}
      </div>
      <div class="field">
        <label for="cp-repeat">Повторите пароль</label>
        <PasswordInput id="cp-repeat" bind:value={repeat} invalid={mismatch} />
        {#if mismatch}<span class="note bad">Пароли не совпадают</span>{/if}
      </div>
    {/if}

    {#if error}<p class="bad" role="alert">{error}</p>{/if}
    <button type="submit" hidden aria-hidden="true"></button>
  </form>
  {#snippet actions()}
    <Button onclick={close}>Отмена</Button>
    <Button variant={encryptNext ? "primary" : "danger"} disabled={!canSave} onclick={save}>
      {busy ? "Сохраняем…" : encryptNext ? "Сохранить" : "Выключить шифрование"}
    </Button>
  {/snippet}
</ConfirmDialog>

<style>
  .form {
    display: flex;
    flex-direction: column;
    gap: 14px;
    margin-top: 6px;
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  label {
    font-weight: 500;
    color: var(--text);
  }

  .note {
    font-size: 12px;
    color: var(--text-subtle);
  }

  .bad {
    color: var(--danger-text);
  }
</style>
