<script lang="ts">
  import { onMount } from "svelte";
  import { api } from "./lib/api";
  import { app, go } from "./lib/nav.svelte";
  import FileError from "./screens/FileError.svelte";
  import Placeholder from "./screens/Placeholder.svelte";
  import Unlock from "./screens/Unlock.svelte";
  import Welcome from "./screens/Welcome.svelte";

  onMount(async () => {
    const status = await api.status();
    app.defaultVault = status.defaultVault;
    app.vault = status.lastVault;
    if (status.unlocked) go("main");
    else if (status.lastVault) go("unlock");
    else go("welcome");
  });
</script>

{#if app.screen === "loading"}
  <!-- Пустой экран на долю секунды, пока Rust отвечает на первый запрос -->
{:else if app.screen === "welcome"}
  <Welcome />
{:else if app.screen === "unlock"}
  <Unlock />
{:else if app.screen === "file-error"}
  <FileError />
{:else}
  <Placeholder screen={app.screen} />
{/if}
