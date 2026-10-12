<script lang="ts">
  import { nav } from "./lib/nav.svelte";
  import Placeholder from "./screens/Placeholder.svelte";
  import Unlock from "./screens/Unlock.svelte";
</script>

{#if nav.screen === "unlock"}
  <Unlock />
{:else}
  <Placeholder screen={nav.screen} />
{/if}
