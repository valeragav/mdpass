<script lang="ts">
  import type { PreviewLine } from "../entryText";

  let { lines }: { lines: PreviewLine[] } = $props();
</script>

<div class="body mono">
  {#each lines as line, i (i)}
    <div class="line">
      <span class="n" aria-hidden="true">{i + 1}</span>
      <span class="text"
        ><span class="prefix">{line.prefix}</span><span class="k">{line.key}</span><span class="v {line.tone}"
          >{line.value}</span
        ></span
      >
    </div>
  {/each}
</div>

<style>
  .body {
    padding: 16px 18px;
    display: flex;
    flex-direction: column;
  }

  .line {
    display: flex;
    gap: 14px;
    font-size: 13px;
    line-height: 22px;
  }

  .n {
    width: 20px;
    flex: none;
    text-align: right;
    color: var(--text-ghost);
    user-select: none;
  }

  .text {
    white-space: pre-wrap;
    word-break: break-all;
  }

  .prefix {
    color: var(--text-faint);
  }

  .k,
  .v.title {
    color: var(--accent);
  }

  .v.muted {
    color: var(--text-subtle);
  }

  .v.placeholder {
    color: var(--text-ghost);
  }
</style>
