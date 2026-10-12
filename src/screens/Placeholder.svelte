<script lang="ts">
  import { go, type Screen } from "../lib/nav.svelte";
  import Button from "../lib/ui/Button.svelte";

  let { screen }: { screen: Screen } = $props();
</script>

<!-- Заглушка для экранов, которые ещё не сделаны -->
<div class="screen">
  <p class="mono">Экран «{screen}» появится на следующих этапах</p>
  <Button icon="back" onclick={() => go("unlock")}>К разблокировке</Button>
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
