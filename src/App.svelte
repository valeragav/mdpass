<script lang="ts">
  import { listen } from "@tauri-apps/api/event";
  import { onMount } from "svelte";
  import { api, errorText, isCommandError, whenChangedOnDisk } from "./lib/api";
  import Button from "./lib/ui/Button.svelte";
  import ConfirmDialog from "./lib/ui/ConfirmDialog.svelte";
  import { showToast } from "./lib/toast.svelte";
  import { app, go } from "./lib/nav.svelte";
  import Toast from "./lib/ui/Toast.svelte";
  import { forgetEntries, loadEntries, vault } from "./lib/vault.svelte";
  import Backups from "./screens/Backups.svelte";
  import EntryEditor from "./screens/EntryEditor.svelte";
  import FileError from "./screens/FileError.svelte";
  import Import from "./screens/Import.svelte";
  import Main from "./screens/Main.svelte";
  import Settings from "./screens/Settings.svelte";
  import Unlock from "./screens/Unlock.svelte";
  import Welcome from "./screens/Welcome.svelte";

  // Пока хранилище открыто, интерфейс сообщает Rust об активности не чаще раза в 20 секунд.
  // Блокирует по таймеру сам Rust.
  const TOUCH_EVERY_MS = 20_000;
  let lastTouch = 0;
  function onActivity() {
    const open = !["loading", "welcome", "unlock", "file-error"].includes(app.screen);
    const now = Date.now();
    if (open && now - lastTouch > TOUCH_EVERY_MS) {
      lastTouch = now;
      api.touch().catch(() => {});
    }
  }

  // Тема, акцент и размер текста из настроек.
  let systemDark = $state(window.matchMedia("(prefers-color-scheme: dark)").matches);
  $effect(() => {
    const media = window.matchMedia("(prefers-color-scheme: dark)");
    const onChange = () => (systemDark = media.matches);
    media.addEventListener("change", onChange);
    return () => media.removeEventListener("change", onChange);
  });
  $effect(() => {
    const root = document.documentElement;
    const { theme, accent, textSize } = app.settings;
    root.dataset.theme = theme === "system" ? (systemDark ? "dark" : "light") : theme;
    root.dataset.textSize = textSize;
    root.style.setProperty("--accent-base", accent);
  });

  // Файл изменили вне приложения: синхронизация или другая копия mdpass.
  let changedOnDisk = $state(false);
  let reloading = $state(false);
  whenChangedOnDisk(() => (changedOnDisk = true));

  async function reloadVault() {
    reloading = true;
    try {
      await api.reloadVault();
      await loadEntries();
      // Новую запись можно сохранить ещё раз из той же формы. У правки существующей
      // записи номер записи после перечитывания другой, поэтому — к списку.
      if (app.screen === "new-entry") {
        showToast("Файл перечитан. Нажмите «Сохранить запись» ещё раз");
      } else {
        go("main");
        showToast("Файл перечитан. Повторите последнее изменение");
      }
    } catch (err) {
      if (isCommandError(err) && (err.kind === "wrongPassphrase" || err.kind === "passphraseRequired")) {
        forgetEntries();
        showToast("Файл перешифровали другим паролем: откройте его заново");
      } else {
        showToast(errorText(err));
      }
    } finally {
      reloading = false;
      changedOnDisk = false;
    }
  }

  onMount(() => {
    const unlisten = listen<string>("vault-locked", (event) => {
      forgetEntries();
      showToast(event.payload === "minimized" ? "Хранилище заблокировано: окно свернули" : "Хранилище заблокировано из-за бездействия");
    });
    init();
    return () => {
      unlisten.then((stop) => stop());
    };
  });

  async function init() {
    const status = await api.status();
    app.settings = status.settings;
    app.protection = status.protection;
    app.defaultVault = status.defaultVault;
    app.vault = status.lastVault;
    if (status.unlocked) go("main");
    else if (status.lastVault) go("unlock");
    else go("welcome");
  }
</script>

{#if app.screen === "loading"}
  <!-- Пустой экран на долю секунды, пока Rust отвечает на первый запрос -->
{:else if app.screen === "welcome"}
  <Welcome />
{:else if app.screen === "unlock"}
  <Unlock />
{:else if app.screen === "file-error"}
  <FileError />
{:else if app.screen === "main"}
  <Main />
{:else if app.screen === "import"}
  <Import />
{:else if app.screen === "settings"}
  <Settings />
{:else if app.screen === "backups"}
  <Backups />
{:else if app.screen === "new-entry"}
  <EntryEditor entry={null} />
{:else if app.screen === "edit-entry"}
  {#key vault.selectedId}
    <EntryEditor entry={vault.entries.find((e) => e.id === vault.selectedId) ?? null} />
  {/key}
{/if}

<Toast />

<ConfirmDialog open={changedOnDisk} title="Файл изменён вне приложения" oncancel={() => (changedOnDisk = false)}>
  Похоже, файл обновила синхронизация или другая копия mdpass. Последнее изменение не записано, чтобы не затереть
  чужие правки. Перечитайте файл и повторите изменение.
  {#snippet actions()}
    <Button onclick={() => (changedOnDisk = false)}>Позже</Button>
    <Button variant="primary" autofocus disabled={reloading} onclick={reloadVault}>
      {reloading ? "Перечитываем…" : "Перечитать файл"}
    </Button>
  {/snippet}
</ConfirmDialog>

<svelte:window onpointerdown={onActivity} onkeydown={onActivity} onwheel={onActivity} onpointermove={onActivity} />
