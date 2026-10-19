<script lang="ts">
  import { vault } from "../../lib/vault.svelte";

  interface Props {
    /// Путь раздела. Щелчок по части открывает этот раздел.
    path: string[];
  }

  let { path }: Props = $props();
</script>

<nav class="crumb mono" aria-label="Путь">
  {#each path as part, i (i)}
    {#if i > 0}<span aria-hidden="true">{" / "}</span>{/if}
    <button type="button" onclick={() => (vault.selectedSection = path.slice(0, i + 1))}>{part}</button>
  {/each}
</nav>

<style>
  .crumb {
    min-height: 17px;
    font-size: 12px;
    color: var(--text-subtle);
  }

  button {
    font: inherit;
    color: inherit;
  }

  /* Под палец: область нажатия выше строки, а строка остаётся той же высоты. */
  @media (pointer: coarse) {
    button {
      padding: 10px 0;
      margin: -10px 0;
    }
  }

  button:hover {
    color: var(--accent);
    text-decoration: underline;
    text-underline-offset: 3px;
  }
</style>
