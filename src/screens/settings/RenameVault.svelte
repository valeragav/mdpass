<script lang="ts">
  import { api, errorText } from "../../lib/api";
  import { app } from "../../lib/nav.svelte";
  import { showToast } from "../../lib/toast.svelte";
  import Button from "../../lib/ui/Button.svelte";
  import ConfirmDialog from "../../lib/ui/ConfirmDialog.svelte";

  let { open, onclose }: { open: boolean; onclose: () => void } = $props();

  const stem = () => (app.vault?.name ?? "").replace(/(\.md)?(\.age)?$/, "");
  let name = $state(stem());
  let busy = $state(false);
  let error = $state<string | null>(null);

  $effect(() => {
    if (open) {
      name = stem();
      error = null;
    }
  });

  let extension = $derived(app.protection === "none" ? ".md" : ".md.age");
  let canSave = $derived(name.trim().length > 0 && name.trim() !== stem() && !busy);

  async function save() {
    if (!canSave) return;
    busy = true;
    error = null;
    try {
      app.vault = await api.renameVault(name);
      showToast(`Файл переименован: ${app.vault.name}`);
      onclose();
    } catch (err) {
      error = errorText(err);
    } finally {
      busy = false;
    }
  }
</script>

<ConfirmDialog {open} title="Переименовать хранилище" icon="edit" oncancel={onclose}>
  <form
    class="form"
    onsubmit={(e) => {
      e.preventDefault();
      save();
    }}
  >
    <p>Файл останется в той же папке, резервные копии переедут вместе с ним.</p>
    <label class="field">
      <span>Имя файла</span>
      <span class="input-row">
        <!-- svelte-ignore a11y_autofocus -->
        <input class="input mono" bind:value={name} spellcheck="false" autofocus />
        <span class="ext mono">{extension}</span>
      </span>
    </label>
    {#if error}<p class="bad" role="alert">{error}</p>{/if}
  </form>
  {#snippet actions()}
    <Button onclick={onclose}>Отмена</Button>
    <Button variant="primary" disabled={!canSave} onclick={save}>{busy ? "Переименовываем…" : "Переименовать"}</Button>
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
    font-weight: 500;
    color: var(--text);
  }

  .input-row {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .input {
    flex: 1;
    min-width: 0;
    height: var(--control-h);
    padding: 0 14px;
    border-radius: var(--radius);
    border: 1px solid var(--line-strong);
    background: var(--bg-chrome);
    color: var(--text);
  }

  .ext {
    color: var(--text-subtle);
    font-weight: 400;
  }

  .bad {
    color: var(--danger-text);
  }
</style>
