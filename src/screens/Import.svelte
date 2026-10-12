<script lang="ts">
  import { onMount } from "svelte";
  import { api, errorText, type ImportFixes, type ImportIssue, type ImportPreview, type VaultFile } from "../lib/api";
  import { app, go } from "../lib/nav.svelte";
  import { plural } from "../lib/text";
  import { showToast } from "../lib/toast.svelte";
  import Button from "../lib/ui/Button.svelte";
  import ConfirmDialog from "../lib/ui/ConfirmDialog.svelte";
  import FileChip from "../lib/ui/FileChip.svelte";
  import Icon from "../lib/ui/Icon.svelte";
  import Logo from "../lib/ui/Logo.svelte";
  import { loadEntries } from "../lib/vault.svelte";

  const fromWelcome = app.importFrom === "welcome";

  let source = $state<VaultFile | null>(null);
  let fixes = $state<ImportFixes>({ legacyTags: true, duplicateNotes: true, emptySections: true, missingSpace: true });
  let preview = $state<ImportPreview | null>(null);
  let error = $state<string | null>(null);
  let busy = $state(false);
  let askDelete = $state(false);

  onMount(pick);

  async function pick() {
    const file = await api.pickMarkdown();
    if (file) {
      source = file;
      await refresh();
    }
  }

  async function refresh() {
    if (!source) return;
    error = null;
    try {
      preview = await api.analyzeImport(source.path, fixes);
    } catch (err) {
      preview = null;
      error = errorText(err);
    }
  }

  function toggle(key: keyof ImportFixes) {
    fixes[key] = !fixes[key];
    refresh();
  }

  const FIX_OF: Record<ImportIssue["kind"], keyof ImportFixes> = {
    legacyTags: "legacyTags",
    duplicateNotes: "duplicateNotes",
    emptySections: "emptySections",
    missingSpace: "missingSpace",
  };

  const quoted = (titles: string[]) => titles.map((t) => `«${t}»`).join(", ");

  function describe(issue: ImportIssue): { title: string; text: string } {
    const n = issue.titles.length;
    switch (issue.kind) {
      case "legacyTags":
        return {
          title: "Ключ teg станет tags",
          text: `Найден в ${n} ${plural(n, ["записи", "записях", "записях"])}: ${quoted(issue.titles)}. Теги разберём из квадратных скобок.`,
        };
      case "duplicateNotes":
        return {
          title: `Повторяющиеся заметки в ${quoted(issue.titles)}`,
          text: "Заметки note1, note2… совпадают. Останется одна заметка note.",
        };
      case "emptySections":
        return { title: "Пустые разделы", text: `Пропустим ${quoted(issue.titles)}: в них нет полей. Ничего не потеряется.` };
      case "missingSpace":
        return {
          title: "Нет пробела после двоеточия",
          text: `В ${quoted(issue.titles)} строки записаны слитно. Значения распознаны верно, пробел добавим.`,
        };
    }
  }

  let fixCount = $derived(preview?.issues.filter((i) => fixes[FIX_OF[i.kind]]).length ?? 0);

  type Row = { key: string; depth: number; hashes: string; label: string; entry: boolean };
  let tree = $derived.by(() => {
    const rows: Row[] = [];
    let last: string[] = [];
    (preview?.places ?? []).forEach((place, n) => {
      place.path.forEach((part, i) => {
        if (last[i] !== part) {
          rows.push({ key: `g${n}:${i}`, depth: i, hashes: "#".repeat(i + 1), label: part, entry: false });
          last = [...last.slice(0, i), part];
        }
      });
      last = place.path.slice();
      rows.push({ key: `e${n}`, depth: place.path.length, hashes: "", label: `**${place.title}**`, entry: true });
    });
    return rows;
  });

  async function runImport() {
    if (!source || !preview || busy) return;
    busy = true;
    error = null;
    try {
      const count = await api.runImport(source.path, fixes);
      await loadEntries();
      showToast(`Импортировано ${count} ${plural(count, ["запись", "записи", "записей"])}`);
      askDelete = true;
    } catch (err) {
      error = errorText(err);
    } finally {
      busy = false;
    }
  }

  async function deleteSource() {
    if (!source) return;
    try {
      await api.deleteImportSource(source.path);
      showToast("Исходный файл удалён");
    } catch (err) {
      showToast(errorText(err));
    }
    go("main");
  }

  const tiles = $derived(
    preview
      ? [
          [preview.entries, ["запись", "записи", "записей"]],
          [preview.sections, ["раздел", "раздела", "разделов"]],
          [preview.fields, ["поле", "поля", "полей"]],
          [preview.tags, ["тег", "тега", "тегов"]],
        ] as [number, [string, string, string]][]
      : [],
  );
</script>

<div class="screen">
  <header>
    <Logo size="sm" />
    {#if fromWelcome}<span class="step mono">Шаг 2 из 2</span>{/if}
  </header>

  <div class="body">
    <div class="main scroll">
      <div class="inner">
        <div class="title">
          <h1>Импорт из Markdown</h1>
          <FileChip label={source?.display ?? "Файл не выбран"} onpick={pick} pickLabel="Выбрать файл" />
        </div>

        {#if error}
          <p class="bad" role="alert"><Icon name="error" /> <span>{error}</span></p>
        {/if}

        {#if !source}
          <div class="empty">
            <p class="muted">Выберите открытый .md-файл с паролями. Он не изменится.</p>
            <Button variant="primary" icon="import" onclick={pick}>Выбрать файл</Button>
          </div>
        {:else if preview}
          <div class="tiles">
            {#each tiles as [n, words] (words[0])}
              <div class="tile">
                <div class="n mono">{n}</div>
                <div class="muted">{plural(n, words)}</div>
              </div>
            {/each}
          </div>

          {#if preview.issues.length > 0}
            <div class="group">
              <div class="group-head">
                <span class="label">Что нашли в файле</span>
                <span class="muted">Исправим автоматически: {fixCount} из {preview.issues.length}</span>
              </div>
              <div class="issues">
                {#each preview.issues as issue (issue.kind)}
                  {@const d = describe(issue)}
                  <label class="issue">
                    <input type="checkbox" checked={fixes[FIX_OF[issue.kind]]} onchange={() => toggle(FIX_OF[issue.kind])} />
                    <span class="issue-text">
                      <span class="issue-title">{d.title}</span>
                      <span class="muted">{d.text}</span>
                    </span>
                  </label>
                {/each}
              </div>
            </div>
          {:else}
            <p class="muted">Файл в порядке: исправлять ничего не нужно.</p>
          {/if}
        {/if}
      </div>
    </div>

    <aside>
      <div class="aside-head mono">Как это будет в хранилище</div>
      <div class="tree scroll">
        {#each tree as row (row.key)}
          <div class="row mono" style:padding-left="{row.depth * 16}px">
            <span class="hashes">{row.hashes}</span>
            <span class:entry={row.entry}>{row.label}</span>
          </div>
        {/each}
      </div>
    </aside>
  </div>

  <div class="bar">
    <span class="note">Исходный файл не изменится. После импорта лучше удалить его: пароли в нём лежат открытым текстом.</span>
    <div class="actions">
      <Button onclick={() => go("main")}>{fromWelcome ? "Пропустить" : "Назад"}</Button>
      <Button variant="primary" icon="import" disabled={!preview || preview.entries === 0 || busy} onclick={runImport}>
        {busy
          ? "Импортируем…"
          : preview
            ? `Импортировать ${preview.entries} ${plural(preview.entries, ["запись", "записи", "записей"])}`
            : "Импортировать"}
      </Button>
    </div>
  </div>
</div>

<ConfirmDialog open={askDelete} title="Удалить исходный файл?" icon="trash" tone="danger" oncancel={() => go("main")}>
  Записи уже в хранилище. В {source?.name} пароли лежат открытым текстом: его лучше удалить. Файл удалится сразу, без
  корзины.
  {#snippet actions()}
    <Button autofocus onclick={() => go("main")}>Оставить</Button>
    <Button variant="danger" icon="trash" onclick={deleteSource}>Удалить файл</Button>
  {/snippet}
</ConfirmDialog>

<style>
  .screen {
    height: 100%;
    display: flex;
    flex-direction: column;
  }

  header {
    height: 48px;
    flex: none;
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0 16px;
    border-bottom: 1px solid var(--line);
    background: var(--bg-chrome);
  }

  .step {
    font-size: 12px;
    color: var(--text-subtle);
  }

  .body {
    flex: 1;
    min-height: 0;
    display: flex;
  }

  .main {
    flex: 1;
    min-width: 0;
    overflow-y: auto;
    padding: 28px 40px 24px;
  }

  .inner {
    max-width: 720px;
    display: flex;
    flex-direction: column;
    gap: 24px;
  }

  .title {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  h1 {
    font-size: 26px;
    font-weight: 600;
    letter-spacing: -0.01em;
  }

  .empty {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 14px;
  }

  .tiles {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    gap: 10px;
  }

  .tile {
    padding: 14px 16px;
    border-radius: 10px;
    border: 1px solid var(--line);
    background: var(--bg-panel);
  }

  .n {
    font-size: 24px;
    font-weight: 500;
  }

  .group {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .group-head {
    display: flex;
    justify-content: space-between;
    gap: 16px;
  }

  .label {
    font-weight: 500;
  }

  .issues {
    display: flex;
    flex-direction: column;
    gap: 1px;
    background: var(--line);
    border: 1px solid var(--line);
    border-radius: 10px;
    overflow: hidden;
  }

  .issue {
    display: flex;
    gap: 14px;
    align-items: flex-start;
    padding: 14px 16px;
    background: var(--bg-panel);
    cursor: pointer;
  }

  .issue input {
    width: 18px;
    height: 18px;
    margin-top: 2px;
    accent-color: var(--accent);
  }

  .issue-text {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .issue-title {
    font-weight: 500;
  }

  .muted {
    color: var(--text-subtle);
  }

  .bad {
    display: flex;
    gap: 8px;
    color: var(--danger-text);
  }

  aside {
    width: 380px;
    flex: none;
    display: flex;
    flex-direction: column;
    min-height: 0;
    background: var(--bg-inset);
    border-left: 1px solid var(--line);
  }

  .aside-head {
    padding: 14px 20px;
    border-bottom: 1px solid var(--line);
    font-size: 12px;
    color: var(--text-subtle);
  }

  .tree {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: 14px 20px;
  }

  .row {
    display: flex;
    gap: 8px;
    font-size: 13px;
    line-height: 24px;
    color: var(--text-muted);
  }

  .hashes {
    color: var(--text-faint);
  }

  .entry {
    color: var(--accent);
  }

  .bar {
    flex: none;
    min-height: 68px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    padding: 12px 40px;
    border-top: 1px solid var(--line);
    background: var(--bg-chrome);
  }

  .note {
    font-size: 12px;
    color: var(--text-subtle);
  }

  .actions {
    display: flex;
    gap: 10px;
    flex: none;
  }

  @media (max-width: 1100px) {
    aside {
      display: none;
    }
  }
</style>
