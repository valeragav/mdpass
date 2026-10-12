<script lang="ts">
  import Icon, { type IconName } from "./Icon.svelte";

  interface Props {
    icon: IconName;
    label: string;
    bordered?: boolean;
    size?: "md" | "sm";
    pressed?: boolean;
    onclick?: () => void;
  }

  let { icon, label, bordered = false, size = "md", pressed, onclick }: Props = $props();
</script>

<button type="button" class="icon-btn {size}" class:bordered aria-label={label} aria-pressed={pressed} title={label} {onclick}>
  <Icon name={icon} size={18} />
</button>

<style>
  .icon-btn {
    width: 40px;
    height: 40px;
    flex: none;
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: var(--radius);
    color: var(--text-muted);
  }

  .sm {
    width: 36px;
    height: 36px;
  }

  .bordered {
    border: 1px solid var(--line-strong);
  }

  .icon-btn:hover {
    background: var(--bg-hover-strong);
    color: var(--text);
  }
</style>
