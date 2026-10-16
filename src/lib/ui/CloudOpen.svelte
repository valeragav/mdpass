<script lang="ts">
  // Хранилище из облака на новом компьютере: подключить облако (если ещё не),
  // выбрать файл в облаке, выбрать, куда его положить, и открыть мастер-паролем.
  import { api, errorText } from "../api";
  import { app, go } from "../nav.svelte";
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

  async function choose(name: string) {
    busy = true;
    error = null;
    try {
      const target = await api.pickNewVault(name);
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
    <p>В служебной папке приложения хранилищ нет. Включите синхронизацию на компьютере, где хранилище уже есть.</p>
  {:else if names}
    <p>Выберите хранилище. Его копия ляжет на этот компьютер и будет синхронизироваться с облаком.</p>
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
