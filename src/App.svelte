<script lang="ts">
  import { listen } from "@tauri-apps/api/event";
  import { onMount } from "svelte";
  import { api } from "./lib/api";
  import { showToast } from "./lib/toast.svelte";
  import { app, go } from "./lib/nav.svelte";
  import Toast from "./lib/ui/Toast.svelte";
  import { forgetEntries, vault } from "./lib/vault.svelte";
  import EntryEditor from "./screens/EntryEditor.svelte";
  import FileError from "./screens/FileError.svelte";
  import Main from "./screens/Main.svelte";
  import Placeholder from "./screens/Placeholder.svelte";
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
{:else if app.screen === "new-entry"}
  <EntryEditor entry={null} />
{:else if app.screen === "edit-entry"}
  {#key vault.selectedId}
    <EntryEditor entry={vault.entries.find((e) => e.id === vault.selectedId) ?? null} />
  {/key}
{:else}
  <Placeholder screen={app.screen} />
{/if}

<Toast />

<svelte:window onpointerdown={onActivity} onkeydown={onActivity} onwheel={onActivity} onpointermove={onActivity} />
