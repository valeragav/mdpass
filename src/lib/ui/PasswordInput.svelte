<script lang="ts">
  import Icon from "./Icon.svelte";

  interface Props {
    id: string;
    value: string;
    placeholder?: string;
    invalid?: boolean;
    describedby?: string;
    autofocus?: boolean;
  }

  let {
    id,
    value = $bindable(),
    placeholder,
    invalid = false,
    describedby,
    autofocus = false,
  }: Props = $props();

  let shown = $state(false);
</script>

<div class="wrap">
  <!-- svelte-ignore a11y_autofocus -->
  <input
    {id}
    type={shown ? "text" : "password"}
    bind:value
    {placeholder}
    {autofocus}
    autocomplete="off"
    spellcheck="false"
    aria-invalid={invalid}
    aria-describedby={describedby}
    class:invalid
  />
  <button
    type="button"
    class="eye"
    aria-label={shown ? "Скрыть пароль" : "Показать пароль"}
    onclick={() => (shown = !shown)}
  >
    <Icon name={shown ? "eye-off" : "eye"} size={18} />
  </button>
</div>

<style>
  .wrap {
    position: relative;
    display: flex;
    align-items: center;
  }

  input {
    width: 100%;
    height: var(--control-h);
    padding: 0 52px 0 14px;
    border-radius: var(--radius);
    border: 1px solid var(--line-strong);
    background: var(--bg-chrome);
    color: var(--text);
  }

  input.invalid {
    border-color: var(--danger);
  }

  .eye {
    position: absolute;
    right: 2px;
    width: 40px;
    height: 40px;
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: var(--radius);
    color: var(--text-muted);
  }

  .eye:hover {
    background: var(--bg-hover-strong);
  }
</style>
