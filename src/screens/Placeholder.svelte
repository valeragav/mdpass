<script lang="ts">
  import { api } from "../lib/api";
  import { app, go, type Screen } from "../lib/nav.svelte";
  import Button from "../lib/ui/Button.svelte";

  let { screen }: { screen: Screen } = $props();

  async function lock() {
    await api.lock();
    go("unlock");
  }
</script>

<!-- Заглушка для экранов, которые ещё не сделаны -->
<div class="screen">
  <p class="mono">Экран «{screen}» появится на следующих этапах</p>
  {#if screen === "main"}
    <p class="mono">Открыто: {app.vault?.display}</p>
    <Button icon="lock" onclick={lock}>Заблокировать</Button>
  {:else}
    <Button icon="back" onclick={() => go(app.vault ? "unlock" : "welcome")}>Назад</Button>
  {/if}
</div>

<style>
  .screen {
    height: 100%;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 20px;
    color: var(--text-muted);
  }
</style>
