<script lang="ts">
  import { onMount } from "svelte";
  import { errorText } from "../lib/api";
  import { app, go } from "../lib/nav.svelte";
  import { plural } from "../lib/text";
  import AppShell from "../lib/ui/AppShell.svelte";
  import Button from "../lib/ui/Button.svelte";
  import ConfirmDialog from "../lib/ui/ConfirmDialog.svelte";
  import Icon from "../lib/ui/Icon.svelte";
  import { deleteEntry, loadEntries, matches, vault } from "../lib/vault.svelte";
  import EntryDetails from "./main/EntryDetails.svelte";
  import Sidebar from "./main/Sidebar.svelte";

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

<AppShell {status}>
  <Sidebar {visible} bind:query bind:tag />

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
          <Button icon="import" onclick={() => go("import")}>Импортировать .md</Button>
        </div>
        <div class="sample">
          <div class="sample-head mono">Так выглядит запись в файле {app.vault?.name.replace(/\.age$/, "")}</div>
          <div class="sample-body mono">
            <div><span class="p">### </span><span class="k">название</span></div>
            <div><span class="k">ssh:</span> user@host</div>
            <div><span class="k">password:</span> <span class="muted">••••••••</span></div>
            <div><span class="k">tags:</span> [work], [prod]</div>
          </div>
        </div>
      </div>
    </div>
  {:else if selected}
    {#key selected.id}
      <EntryDetails entry={selected} ondelete={() => (askDelete = true)} />
    {/key}
  {:else}
    <div class="center"><p class="muted">Выберите запись слева</p></div>
  {/if}
</AppShell>

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

  .p {
    color: var(--text-faint);
  }

  .k {
    color: var(--accent);
  }

  .muted {
    color: var(--text-muted);
  }
</style>
