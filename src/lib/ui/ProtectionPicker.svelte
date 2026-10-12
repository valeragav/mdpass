<script lang="ts" module>
  export type Protection = "strong" | "weak" | "none";

  export const PROTECTION_LABEL: Record<Protection, string> = {
    strong: "Надёжное шифрование",
    weak: "Слабое шифрование",
    none: "Без шифрования",
  };
</script>

<script lang="ts">
  import Icon from "./Icon.svelte";

  let { value = $bindable() }: { value: Protection } = $props();

  const OPTIONS: { id: Protection; title: string; text: string }[] = [
    { id: "strong", title: "Надёжное", text: "Открывается около секунды. Подобрать пароль очень дорого" },
    { id: "weak", title: "Слабое", text: "Открывается мгновенно. Короткий пароль подбирается быстро" },
    { id: "none", title: "Без шифрования", text: "Обычный .md-файл. Прочитает любой, у кого есть файл" },
  ];
</script>

<div class="picker" role="radiogroup" aria-label="Защита файла">
  {#each OPTIONS as option (option.id)}
    <button
      type="button"
      role="radio"
      class="option"
      class:on={value === option.id}
      class:danger={option.id === "none"}
      aria-checked={value === option.id}
      onclick={() => (value = option.id)}
    >
      <span class="title">{option.title}</span>
      <span class="text">{option.text}</span>
    </button>
  {/each}
</div>

{#if value === "none"}
  <p class="warn">
    <Icon name="warning" size={18} color="var(--danger)" />
    <span>Пароли будут лежать в файле открытым текстом. Подходит только для того, что не жалко показать.</span>
  </p>
{:else if value === "weak"}
  <p class="note">Выбирайте длинную фразу: слабое шифрование держится в основном на длине пароля.</p>
{/if}

<style>
  .picker {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 8px;
  }

  .option {
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding: 10px 12px;
    border-radius: var(--radius);
    border: 1px solid var(--line-strong);
    color: var(--text-2);
  }

  .option:hover {
    background: var(--bg-hover-strong);
  }

  .option.on {
    border-color: var(--accent);
    background: color-mix(in srgb, var(--accent) 12%, transparent);
  }

  .option.on.danger {
    border-color: var(--danger);
    background: color-mix(in srgb, var(--danger) 12%, transparent);
  }

  .title {
    font-weight: 600;
    color: var(--text);
  }

  .text {
    font-size: 12px;
    color: var(--text-subtle);
  }

  .warn {
    display: flex;
    gap: 10px;
    align-items: flex-start;
    padding: 10px 12px;
    border-radius: var(--radius);
    border: 1px solid color-mix(in srgb, var(--danger) 50%, transparent);
    color: var(--danger-text);
    font-size: 13px;
  }

  .note {
    font-size: 12px;
    color: var(--text-subtle);
  }

  @media (max-width: 520px) {
    .picker {
      grid-template-columns: 1fr;
    }
  }
</style>
