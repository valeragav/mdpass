<script lang="ts">
  // Хранилище из облака на новом устройстве: подключить облако (если ещё не),
  // выбрать файл в облаке, выбрать, куда его положить, и открыть мастер-паролем.
  // На телефоне место не выбирается: файл ложится в папку приложения.
  import { api, errorText, type VaultFile } from "../api";
  import { app, go } from "../nav.svelte";
  import { IS_MOBILE } from "../platform";
  import Button from "./Button.svelte";
  import CloudConnect from "./CloudConnect.svelte";
  import ConfirmDialog from "./ConfirmDialog.svelte";

  interface Props {
    open: boolean;
    onclose: () => void;
  }

  let { open, onclose }: Props = $props();

  let step = $state<"checking" | "connect" | "list">("checking");
  let names = $state<string[] | null>(null);
  let error = $state<string | null>(null);
  let busy = $state(false);

  $effect(() => {
    if (open) check();
  });

  async function check() {
    step = "checking";
    names = null;
    error = null;
    try {
      const status = await api.cloudStatus();
      if (status.connected) await list();
      else step = "connect";
    } catch (err) {
      error = errorText(err);
      step = "list";
    }
  }

  async function list() {
    step = "list";
    try {
      names = await api.cloudList();
    } catch (err) {
      error = errorText(err);
    }
  }

  /// Свободное имя в папке приложения: `vault.md.age`, `vault (2).md.age`…
  async function freeTarget(name: string): Promise<VaultFile | null> {
    const base = app.defaultVault?.path;
    if (!base) return null;
    const dir = base.slice(0, Math.max(base.lastIndexOf("/"), base.lastIndexOf("\\")) + 1);
    const dot = name.indexOf(".");
    const [stem, ext] = dot > 0 ? [name.slice(0, dot), name.slice(dot)] : [name, ""];
    for (let n = 1; ; n++) {
      const file = await api.describePath(`${dir}${n === 1 ? name : `${stem} (${n})${ext}`}`);
      if (!file.exists) return file;
    }
  }

  async function choose(name: string) {
    busy = true;
    error = null;
    try {
      const target = IS_MOBILE ? await freeTarget(name) : await api.pickNewVault(name);
      if (!target) return;
      app.vault = await api.cloudDownload(name, target.path);
      onclose();
      go("unlock");
    } catch (err) {
      error = errorText(err);
    } finally {
      busy = false;
    }
  }
</script>

<ConfirmDialog
  open={open && step === "connect"}
  title="Подключить облако"
  icon="lock"
  oncancel={onclose}
>
  <CloudConnect ondone={list} />
  {#snippet actions()}
    <Button onclick={onclose}>Отмена</Button>
  {/snippet}
</ConfirmDialog>

<ConfirmDialog open={open && step === "list"} title="Открыть из облака" icon="lock" oncancel={onclose}>
  {#if error}
    <p class="error">{error}</p>
  {/if}
  {#if names === null && !error}
    <p>Смотрим, что лежит в служебной папке приложения…</p>
  {:else if names && names.length === 0}
    <p>В служебной папке приложения хранилищ нет. Включите синхронизацию на устройстве, где хранилище уже есть.</p>
  {:else if names}
    <p>Выберите хранилище. Его копия ляжет на это устройство и будет синхронизироваться с облаком.</p>
    <ul class="names">
      {#each names as name (name)}
        <li><Button full disabled={busy} onclick={() => choose(name)}>{name}</Button></li>
      {/each}
    </ul>
  {/if}
  {#snippet actions()}
    <Button onclick={onclose}>Отмена</Button>
  {/snippet}
</ConfirmDialog>

<style>
  .names {
    display: flex;
    flex-direction: column;
    gap: 8px;
    margin: 12px 0 0;
    padding: 0;
    list-style: none;
  }

  .error {
    color: var(--danger);
  }
</style>
