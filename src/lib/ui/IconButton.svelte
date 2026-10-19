<script lang="ts">
  import Icon, { type IconName } from "./Icon.svelte";

  interface Props {
    icon: IconName;
    label: string;
    bordered?: boolean;
    size?: "md" | "sm";
    pressed?: boolean;
    /// Включённая кнопка-переключатель залита цветом акцента.
    filled?: boolean;
    onclick?: () => void;
    /// Кнопка «назад» экрана: её же нажимает системная кнопка «Назад» на телефоне.
    back?: boolean;
  }

  let { icon, label, bordered = false, size = "md", pressed, filled = false, onclick, back = false }: Props = $props();
</script>

<button
  type="button"
  class="icon-btn {size}"
  class:bordered
  class:filled
  aria-label={label}
  aria-pressed={pressed}
  title={label}
  {onclick}
  data-back={back || undefined}
>
  <Icon name={icon} size={18} fill={filled ? "currentColor" : "none"} />
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

  .icon-btn.filled,
  .icon-btn.filled:hover {
    color: var(--accent);
  }
</style>
