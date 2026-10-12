<script lang="ts">
  import { api, errorText, isCommandError } from "../../lib/api";
  import { showToast } from "../../lib/toast.svelte";
  import { MIN_MASTER_LENGTH, strength } from "../../lib/text";
  import Button from "../../lib/ui/Button.svelte";
  import ConfirmDialog from "../../lib/ui/ConfirmDialog.svelte";
  import PasswordInput from "../../lib/ui/PasswordInput.svelte";

  let { open, onclose }: { open: boolean; onclose: () => void } = $props();

  let current = $state("");
  let next = $state("");
  let repeat = $state("");
  let busy = $state(false);
  let error = $state<string | null>(null);
  let wrongCurrent = $state(false);

  let meter = $derived(strength(next));
  let mismatch = $derived(repeat.length > 0 && repeat !== next);
  let canSave = $derived(current.length > 0 && next.length >= MIN_MASTER_LENGTH && repeat === next && !busy);

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
      await api.changePassphrase(current, next);
      showToast("Мастер-пароль изменён, файл зашифрован заново");
      close();
    } catch (err) {
      wrongCurrent = isCommandError(err) && (err.kind === "wrongPassphrase" || err.kind === "tooSoon");
      error = wrongCurrent ? "Текущий мастер-пароль неверный. Попробуйте через 3 секунды" : errorText(err);
    } finally {
      busy = false;
    }
  }
</script>

<ConfirmDialog {open} title="Сменить мастер-пароль" icon="lock" oncancel={close}>
  <form
    class="form"
    onsubmit={(e) => {
      e.preventDefault();
      save();
    }}
  >
    <p>Файл будет зашифрован заново. Старый пароль перестанет подходить.</p>
    <div class="field">
      <label for="cp-current">Текущий мастер-пароль</label>
      <PasswordInput id="cp-current" bind:value={current} invalid={wrongCurrent} autofocus />
    </div>
    <div class="field">
      <label for="cp-new">Новый мастер-пароль</label>
      <PasswordInput id="cp-new" bind:value={next} placeholder={`Не короче ${MIN_MASTER_LENGTH} символов`} />
      {#if next}<span class="note">Надёжность: {meter.label.toLowerCase() || "—"}</span>{/if}
    </div>
    <div class="field">
      <label for="cp-repeat">Повторите новый пароль</label>
      <PasswordInput id="cp-repeat" bind:value={repeat} invalid={mismatch} />
      {#if mismatch}<span class="note bad">Пароли не совпадают</span>{/if}
    </div>
    {#if error}<p class="bad" role="alert">{error}</p>{/if}
    <button type="submit" hidden aria-hidden="true"></button>
  </form>
  {#snippet actions()}
    <Button onclick={close}>Отмена</Button>
    <Button variant="primary" disabled={!canSave} onclick={save}>{busy ? "Шифруем…" : "Сменить пароль"}</Button>
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
