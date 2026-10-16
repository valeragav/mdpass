<script lang="ts">
  import { onMount } from "svelte";
  import { errorText } from "../lib/api";
  import { app, go } from "../lib/nav.svelte";
  import { plural } from "../lib/text";
  import AppShell from "../lib/ui/AppShell.svelte";
  import Button from "../lib/ui/Button.svelte";
  import ConfirmDialog from "../lib/ui/ConfirmDialog.svelte";
  import Icon from "../lib/ui/Icon.svelte";
  import { deleteEntry, loadEntries, lockVault, matches, vault } from "../lib/vault.svelte";
  import EntryDetails from "./main/EntryDetails.svelte";
  import SectionView from "./main/SectionView.svelte";
  import Sidebar from "./main/Sidebar.svelte";
  import { closeDetail, layout } from "../lib/layout.svelte";
  import IconButton from "../lib/ui/IconButton.svelte";
  import { showToast } from "../lib/toast.svelte";
  import { toggleFavorite } from "../lib/vault.svelte";
  import MobileList from "./mobile/MobileList.svelte";

  let query = $state("");
  let tag = $state("");
  let error = $state<string | null>(null);

  onMount(async () => {
    try {
      await loadEntries();
    } catch (err) {
      error = errorText(err);
    }
  });

  let visible = $derived(vault.entries.filter((e) => matches(e, query, tag)));
  let selected = $derived(vault.entries.find((e) => e.id === vault.selectedId) ?? null);

  const entriesWord = (n: number) => `${n} ${plural(n, ["запись", "записи", "записей"])}`;
  let status = $derived(
    visible.length === vault.entries.length
      ? entriesWord(vault.entries.length)
      : `${visible.length} из ${entriesWord(vault.entries.length)}`,
  );

  let askDelete = $state(false);

  // Ctrl+F — поиск, Ctrl+N — новая запись, Ctrl+L — заблокировать.
  function onKey(event: KeyboardEvent) {
    if (layout.narrow && layout.detail && event.key === "Escape" && !askDelete) {
      closeDetail();
      return;
    }
    if (!(event.ctrlKey || event.metaKey) || event.altKey || askDelete) return;
    const key = event.key.toLowerCase();
    if (key === "f" || key === "а") {
      event.preventDefault();
      document.querySelector<HTMLInputElement>('input[type="search"]')?.focus();
    } else if (key === "n" || key === "т") {
      event.preventDefault();
      go("new-entry");
    } else if (key === "l" || key === "д") {
      event.preventDefault();
      lockVault();
    }
  }

  async function confirmDelete() {
    askDelete = false;
    if (!selected) return;
    try {
      await deleteEntry(selected.id, selected.title);
    } catch (err) {
      error = errorText(err);
    }
  }
</script>

<svelte:window onkeydown={onKey} />

{#snippet content()}

  {#if error}
    <div class="center">
      <p class="error" role="alert"><Icon name="error" /> {error}</p>
    </div>
  {:else if vault.entries.length === 0}
    <div class="center">
      <div class="empty">
        <Icon name="document" size={72} stroke={1.1} color="var(--accent)" />
        <div class="empty-text">
          <h1>Хранилище пусто</h1>
          <p>Добавьте первую запись или импортируйте существующий Markdown-файл</p>
        </div>
        <div class="empty-actions">
          <Button variant="primary" icon="plus" onclick={() => go("new-entry")}>Создать запись</Button>
          <Button
            icon="import"
            onclick={() => {
              app.importFrom = "main";
              go("import");
            }}>Импортировать .md</Button
          >
        </div>
        <div class="sample">
          <div class="sample-head mono">Так выглядит запись в файле {app.vault?.name.replace(/\.age$/, "")}</div>
          <div class="sample-body mono">
            <div><span class="k">**название**</span></div>
            <div><span class="k">ssh:</span> user@host</div>
            <div><span class="k">password:</span> <span class="muted">••••••••</span></div>
            <div><span class="k">tags:</span> [work], [prod]</div>
          </div>
        </div>
      </div>
    </div>
  {:else if vault.selectedSection}
    <!-- Щелчок по разделу: все записи этого раздела без подразделов. -->
    <SectionView path={vault.selectedSection} {visible} />
  {:else if selected}
    {#key selected.id}
      <EntryDetails entry={selected} ondelete={() => (askDelete = true)} compact={layout.narrow} />
    {/key}
  {:else}
    <div class="center"><p class="muted">Выберите запись слева</p></div>
  {/if}
{/snippet}

{#if !layout.narrow}
  <AppShell {status}>
    <Sidebar {visible} bind:query bind:tag />
    {@render content()}
  </AppShell>
{:else if vault.entries.length === 0 || error !== null}
  <AppShell {status}>
    {@render content()}
  </AppShell>
{:else if layout.detail}
  <!-- Телефон: запись на весь экран, сверху «назад» и кнопки записи. -->
  <div class="pane">
    <div class="bar">
      <IconButton icon="back" label="Назад к записям" onclick={closeDetail} />
      <span class="spacer"></span>
      {#if selected && !vault.selectedSection}
        <IconButton
          icon="star"
          label={selected.favorite ? "Убрать из избранного" : "Добавить в избранное"}
          pressed={selected.favorite}
          filled={selected.favorite}
          onclick={() => selected && toggleFavorite(selected).catch((err) => showToast(errorText(err)))}
        />
        <IconButton icon="edit" label="Редактировать" onclick={() => go("edit-entry")} />
      {/if}
    </div>
    {@render content()}
  </div>
{:else}
  <MobileList {visible} bind:query bind:tag />
{/if}

{#if selected}
  <ConfirmDialog
    open={askDelete}
    title={`Удалить «${selected.title}»?`}
    icon="trash"
    tone="danger"
    oncancel={() => (askDelete = false)}
  >
    Запись исчезнет из {app.vault?.name.replace(/\.age$/, "")}.
    {#if app.settings.backupsEnabled}Перед удалением создаётся резервная копия, из неё запись можно вернуть.{/if}
    {#snippet actions()}
      <Button autofocus onclick={() => (askDelete = false)}>Отмена</Button>
      <Button variant="danger" icon="trash" onclick={confirmDelete}>Удалить</Button>
    {/snippet}
  </ConfirmDialog>
{/if}

<style>
  /* Узкий экран: запись или раздел на весь экран со строкой «назад» сверху. */
  .pane {
    height: 100%;
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }

  .pane > :global(*:not(.bar)) {
    flex: 1;
    min-height: 0;
  }

  .bar {
    flex: none;
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 6px 8px;
  }

  .spacer {
    flex: 1;
  }

  .center {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 40px;
  }

  .error {
    display: flex;
    gap: 8px;
    color: var(--danger-text);
  }

  .empty {
    width: 100%;
    max-width: 520px;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 24px;
    text-align: center;
  }

  .empty-text {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  h1 {
    font-size: 26px;
    font-weight: 600;
    letter-spacing: -0.01em;
  }

  .empty-text p {
    color: var(--text-muted);
  }

  .empty-actions {
    display: flex;
    flex-wrap: wrap;
    justify-content: center;
    gap: 10px;
  }

  .sample {
    width: 100%;
    text-align: left;
    border: 1px solid var(--line);
    border-radius: 10px;
    background: var(--bg-inset);
    overflow: hidden;
  }

  .sample-head {
    padding: 10px 18px;
    border-bottom: 1px solid var(--line);
    font-size: 12px;
    color: var(--text-subtle);
  }

  .sample-body {
    padding: 14px 18px;
    font-size: 13px;
    line-height: 22px;
  }

  .k {
    color: var(--accent);
  }

  .muted {
    color: var(--text-muted);
  }
</style>
