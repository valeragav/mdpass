<script lang="ts">
  import { toast } from "../toast.svelte";
</script>

<div class="region" role="status" aria-live="polite">
  {#if toast.text}
    <div class="toast">{toast.text}</div>
  {/if}
</div>

<style>
  .region {
    position: fixed;
    left: 50%;
    bottom: 52px;
    transform: translateX(-50%);
    pointer-events: none;
    z-index: 10;
  }

  .toast {
    padding: 10px 18px;
    border-radius: var(--radius);
    background: var(--text);
    color: var(--bg);
    font-weight: 500;
    white-space: nowrap;
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.4);
  }
</style>
