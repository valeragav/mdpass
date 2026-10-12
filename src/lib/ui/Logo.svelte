<script lang="ts">
  import Icon from "./Icon.svelte";

  interface Props {
    size?: "sm" | "md" | "lg";
    vertical?: boolean;
  }

  let { size = "md", vertical = false }: Props = $props();

  const ICON = { sm: 20, md: 32, lg: 44 };
  const TEXT = { sm: 15, md: 20, lg: 22 };
</script>

<div class="logo" class:vertical>
  <Icon name="key" size={ICON[size]} stroke={size === "sm" ? 1.75 : 1.5} color="var(--accent)" />
  <span style:font-size="{TEXT[size]}px">mdpass</span>
</div>

<style>
  .logo {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .vertical {
    flex-direction: column;
    gap: 14px;
  }

  span {
    font-family: var(--font-mono);
    font-weight: 500;
    letter-spacing: 0.02em;
  }
</style>
