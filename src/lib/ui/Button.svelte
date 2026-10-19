<script lang="ts">
  import type { Snippet } from "svelte";
  import Icon, { type IconName } from "./Icon.svelte";

  interface Props {
    variant?: "primary" | "secondary" | "ghost" | "danger";
    size?: "md" | "sm";
    icon?: IconName;
    full?: boolean;
    type?: "button" | "submit";
    disabled?: boolean;
    /// Фокус на кнопке при открытии диалога.
    autofocus?: boolean;
    onclick?: () => void;
    /// Кнопка «назад» экрана: её же нажимает системная кнопка «Назад» на телефоне.
    back?: boolean;
    children: Snippet;
  }

  let {
    variant = "secondary",
    size = "md",
    icon,
    full = false,
    type = "button",
    disabled = false,
    autofocus = false,
    onclick,
    back = false,
    children,
  }: Props = $props();
</script>

<!-- svelte-ignore a11y_autofocus -->
<button class="btn {variant} {size}" class:full {type} {disabled} {autofocus} {onclick} data-back={back || undefined}>
  {#if icon}
    <Icon name={icon} stroke={variant === "primary" ? 2 : 1.75} />
  {/if}
  {@render children()}
</button>

<style>
  .btn {
    height: var(--control-h);
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
    padding: 0 20px;
    border-radius: var(--radius);
    font-weight: 500;
    white-space: nowrap;
  }

  .sm {
    height: var(--control-h-sm);
    padding: 0 14px;
  }

  .full {
    width: 100%;
  }

  .primary {
    background: var(--accent);
    color: var(--on-accent);
    font-weight: 600;
  }

  .primary:hover:not(:disabled) {
    filter: brightness(1.08);
  }

  .secondary {
    border: 1px solid var(--line-strong);
  }

  .ghost {
    color: var(--text-muted);
  }

  .danger {
    background: var(--danger);
    color: var(--on-accent);
    font-weight: 600;
  }

  .secondary:hover:not(:disabled),
  .ghost:hover:not(:disabled) {
    background: var(--bg-hover-strong);
  }
</style>
