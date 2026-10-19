<script lang="ts">
  import type { Snippet } from "svelte";
  import Icon, { type IconName } from "./Icon.svelte";

  interface Props {
    open: boolean;
    title: string;
    icon?: IconName;
    tone?: "danger" | "accent";
    /// Закрытие по Esc или клику мимо окна.
    oncancel: () => void;
    /// Окно пошире: для таблиц.
    wide?: boolean;
    children: Snippet;
    actions: Snippet;
  }

  let { open, title, icon = "warning", tone = "accent", oncancel, wide = false, children, actions }: Props = $props();

  let dialog: HTMLDialogElement | undefined = $state();
  // У каждого диалога свой id заголовка: на экране их бывает несколько сразу.
  const uid = $props.id();
  const titleId = `${uid}-title`;

  // Нативный <dialog> сам держит фокус внутри окна и закрывается по Esc.
  $effect(() => {
    if (!dialog) return;
    if (open && !dialog.open) dialog.showModal();
    if (!open && dialog.open) dialog.close();
  });

  function onBackdrop(event: MouseEvent) {
    if (event.target === dialog) oncancel();
  }
</script>

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_noninteractive_element_interactions -->
<dialog
  bind:this={dialog}
  aria-labelledby={titleId}
  oncancel={(e) => {
    e.preventDefault();
    oncancel();
  }}
  onclick={onBackdrop}
>
  <div class="box" class:wide>
    <div class="head">
      <Icon name={icon} size={22} color={tone === "danger" ? "var(--danger)" : "var(--accent)"} />
      <div class="text">
        <h2 id={titleId}>{title}</h2>
        <div class="body">{@render children()}</div>
      </div>
    </div>
    <div class="actions">{@render actions()}</div>
  </div>
</dialog>

<style>
  dialog {
    padding: 0;
    border: 0;
    background: none;
    color: inherit;
    max-width: calc(100vw - 32px);
  }

  dialog::backdrop {
    background: rgba(5, 6, 8, 0.7);
  }

  .box.wide {
    width: 640px;
  }

  .box {
    width: 480px;
    max-width: 100%;
    display: flex;
    flex-direction: column;
    gap: 20px;
    padding: 28px;
    border-radius: var(--radius-card);
    background: var(--bg-panel);
    border: 1px solid var(--line-strong);
    box-shadow: 0 24px 64px rgba(0, 0, 0, 0.5);
  }

  .head {
    display: flex;
    gap: 14px;
    align-items: flex-start;
  }

  .head :global(svg) {
    margin-top: 2px;
  }

  .text {
    display: flex;
    flex-direction: column;
    gap: 6px;
    min-width: 0;
  }

  h2 {
    font-size: 18px;
    font-weight: 600;
    overflow-wrap: anywhere;
  }

  .body {
    color: var(--text-muted);
  }

  .actions {
    display: flex;
    justify-content: flex-end;
    align-items: center;
    gap: 10px;
  }

  /* Узкий экран: окно почти во всю ширину, кнопки друг под другом, главная — сверху. */
  @media (max-width: 640px) {
    .box {
      padding: 20px;
      gap: 16px;
    }

    .actions {
      flex-direction: column-reverse;
      align-items: stretch;
    }

    /* Иконка — только у заголовка, текст и поля под ними во всю ширину окна,
       вровень с кнопками. */
    .head {
      display: grid;
      grid-template-columns: auto 1fr;
      align-items: center;
      gap: 12px 10px;
    }

    .head :global(svg) {
      margin-top: 0;
    }

    .text {
      display: contents;
    }

    .body {
      grid-column: 1 / -1;
    }
  }
</style>
