<script lang="ts">
  import { untrack } from "svelte";
  import { api, errorText, type EntryView, type FieldInput } from "../lib/api";
  import { isSecretName, parsePath, parseTags, previewLines } from "../lib/entryText";
  import { app, go } from "../lib/nav.svelte";
  import { showToast } from "../lib/toast.svelte";
  import Button from "../lib/ui/Button.svelte";
  import ConfirmDialog from "../lib/ui/ConfirmDialog.svelte";
  import Icon from "../lib/ui/Icon.svelte";
  import IconButton from "../lib/ui/IconButton.svelte";
  import Logo from "../lib/ui/Logo.svelte";
  import MarkdownPreview from "../lib/ui/MarkdownPreview.svelte";
  import { deleteEntry, loadEntries, vault } from "../lib/vault.svelte";

  /// Запись, которую правим. `null` — новая запись.
  let { entry: entryProp }: { entry: EntryView | null } = $props();
  // Форма заполняется один раз. При смене записи экран создаётся заново через {#key}.
  const entry = untrack(() => entryProp);

  interface Row {
    uid: number;
    key: string;
    value: string;
    secret: boolean;
    shown: boolean;
    /// Номер поля в исходной записи, пока значение секрета не трогали.
    keepFrom: number | null;
  }

  const TEMPLATES: { id: string; label: string; fields: [string, boolean][] }[] = [
    { id: "server", label: "Сервер", fields: [["ssh", false], ["password", true]] },
    { id: "db", label: "База данных", fields: [["host", false], ["port", false], ["user", false], ["password", true]] },
    { id: "site", label: "Сайт", fields: [["url", false], ["login", false], ["password", true]] },
    { id: "token", label: "Токен", fields: [["user", false], ["token", true]] },
    { id: "empty", label: "Пустая", fields: [["", false]] },
  ];

  let nextUid = 1;
  const row = (key: string, value: string, secret: boolean, keepFrom: number | null = null): Row => ({
    uid: nextUid++,
    key,
    value,
    secret,
    shown: false,
    keepFrom,
  });

  // Поле «note» редактируется отдельным многострочным полем, как в дизайне.
  const noteIndex = entry ? entry.fields.findIndex((f) => f.key.toLowerCase() === "note" && !f.secret) : -1;

  const initial = (() => {
    if (!entry) {
      // Новая запись по умолчанию попадает в раздел выбранной записи.
      const near = vault.entries.find((e) => e.id === vault.selectedId) ?? vault.entries.at(-1);
      return { path: near?.path.join(" / ") ?? "", title: "", tags: "", note: "", template: "server" };
    }
    return {
      path: entry.path.join(" / "),
      title: entry.title,
      tags: entry.tags.join(", "),
      note: noteIndex >= 0 ? (entry.fields[noteIndex].value ?? "") : "",
      template: "",
    };
  })();

  let path = $state(initial.path);
  let title = $state(initial.title);
  let tags = $state(initial.tags);
  let note = $state(initial.note);
  let template = $state(initial.template);
  let rows = $state<Row[]>(
    entry
      ? entry.fields
          .map((f, i) => ({ f, i }))
          .filter(({ i }) => i !== noteIndex)
          .map(({ f, i }) => row(f.key, f.value ?? "", f.secret, f.secret ? i : null))
      : TEMPLATES[0].fields.map(([key, secret]) => row(key, "", secret)),
  );

  let busy = $state(false);
  let error = $state<string | null>(null);
  let askLeave = $state(false);
  let askDelete = $state(false);

  const snapshot = () =>
    JSON.stringify({
      path: parsePath(path),
      title: title.trim(),
      tags: parseTags(tags),
      note: note.trim(),
      rows: rows.map((r) => [r.key.trim(), r.keepFrom ?? r.value, r.secret]),
    });
  const start = snapshot();
  let dirty = $derived(snapshot() !== start);

  let pathChips = $derived([...new Set(vault.entries.map((e) => e.path.join(" / ")).filter(Boolean))].slice(0, 8));
  let canSave = $derived(title.trim().length > 0 && !busy);

  function pickTemplate(id: string) {
    const t = TEMPLATES.find((x) => x.id === id)!;
    template = id;
    rows = t.fields.map(([key, secret]) => row(key, "", secret));
  }

  function setKey(r: Row, key: string) {
    r.key = key;
    // Поле с «секретным» именем всегда секретное.
    if (isSecretName(key)) r.secret = true;
  }

  function setValue(r: Row, value: string) {
    r.value = value;
    r.keepFrom = null;
  }

  async function toggleShown(r: Row) {
    if (!r.shown && r.keepFrom !== null && entry) {
      try {
        r.value = await api.revealField(entry.id, r.keepFrom);
        r.keepFrom = null;
      } catch (err) {
        error = errorText(err);
        return;
      }
    }
    r.shown = !r.shown;
  }

  async function generate(r: Row) {
    r.value = await api.generatePassword();
    r.keepFrom = null;
    r.shown = true;
  }

  function draft() {
    // Поля без ключа и пустые поля из шаблона в файл не пишутся.
    const fields: FieldInput[] = rows
      .filter((r) => r.key.trim() && (r.value.trim() || r.keepFrom !== null))
      .map((r) => ({
        key: r.key.trim(),
        value: r.keepFrom === null ? r.value : null,
        secret: r.secret,
        keepFrom: r.keepFrom,
      }));
    if (note.trim()) fields.push({ key: "note", value: note, secret: false, keepFrom: null });
    return { path: parsePath(path), title: title.trim(), tags: parseTags(tags), fields };
  }

  async function save(): Promise<boolean> {
    if (!canSave) return false;
    busy = true;
    error = null;
    try {
      if (entry) {
        await api.updateEntry(entry.id, draft());
        vault.selectedId = entry.id;
      } else {
        vault.selectedId = await api.addEntry(draft());
      }
      await loadEntries();
      showToast(entry ? "Изменения сохранены" : "Запись добавлена");
      go("main");
      return true;
    } catch (err) {
      error = errorText(err);
      return false;
    } finally {
      busy = false;
    }
  }

  function leave() {
    if (dirty) askLeave = true;
    else go("main");
  }

  async function confirmDelete() {
    if (!entry) return;
    askDelete = false;
    try {
      await deleteEntry(entry.id, entry.title);
      go("main");
    } catch (err) {
      error = errorText(err);
    }
  }

  // Секрет в предпросмотре виден, только если его показали в форме.
  let lines = $derived(
    previewLines({
      path: parsePath(path),
      title: title.trim(),
      tags: parseTags(tags),
      fields: [
        ...rows
          .filter((r) => r.key.trim())
          .map((r) => ({ key: r.key.trim(), value: r.secret && !r.shown ? null : r.value || "…" })),
        ...(note.trim() ? [{ key: "note", value: note.trim() }] : []),
      ],
    }),
  );

  let fileName = $derived(app.vault?.name.replace(/\.age$/, "") ?? "vault.md");
</script>

<div class="screen">
  <header>
    <Button variant="ghost" size="sm" icon="back" onclick={leave}>Назад</Button>
    <Logo size="sm" />
    <div class="file mono">
      <Icon name="lock" size={14} />
      <span>{app.vault?.name}</span>
    </div>
  </header>

  <div class="body">
    <form
      class="form"
      onsubmit={(e) => {
        e.preventDefault();
        save();
      }}
    >
      <div class="scroll content">
        <div class="inner">
          <div class="heading">
            <h1>{entry ? "Редактирование записи" : "Новая запись"}</h1>
            {#if entry}<p class="sub">{entry.title}</p>{/if}
          </div>

          {#if !entry}
            <div class="group">
              <div class="label">Шаблон</div>
              <div class="chips">
                {#each TEMPLATES as t (t.id)}
                  <button
                    type="button"
                    class="chip"
                    class:on={template === t.id}
                    aria-pressed={template === t.id}
                    onclick={() => pickTemplate(t.id)}>{t.label}</button
                  >
                {/each}
              </div>
            </div>
          {/if}

          <div class="two">
            <div class="group">
              <label for="path">Раздел</label>
              <input id="path" class="input mono" bind:value={path} placeholder="work / server" spellcheck="false" />
            </div>
            <div class="group">
              <label for="title">Название</label>
              <!-- svelte-ignore a11y_autofocus -->
              <input
                id="title"
                class="input"
                bind:value={title}
                placeholder="Например, vps3 (стейдж)"
                autofocus={!entry}
              />
            </div>
          </div>
          {#if pathChips.length > 0}
            <div class="existing">
              <span>Существующие:</span>
              {#each pathChips as p (p)}
                <button type="button" class="path-chip mono" class:on={parsePath(path).join(" / ") === p} onclick={() => (path = p)}
                  >{p}</button
                >
              {/each}
            </div>
          {/if}

          <div class="group">
            <div class="label">Поля</div>
            <div class="fields">
              {#each rows as r (r.uid)}
                <div class="field">
                  <input
                    class="input key mono"
                    value={r.key}
                    oninput={(e) => setKey(r, e.currentTarget.value)}
                    placeholder="ключ"
                    aria-label="Ключ поля"
                    spellcheck="false"
                  />
                  <input
                    class="input mono"
                    type={r.secret && !r.shown ? "password" : "text"}
                    value={r.value}
                    oninput={(e) => setValue(r, e.currentTarget.value)}
                    placeholder={r.keepFrom !== null ? "•••••••• (без изменений)" : "значение"}
                    aria-label={`Значение поля ${r.key || "без названия"}`}
                    autocomplete="off"
                    spellcheck="false"
                  />
                  <div class="field-actions">
                    {#if r.secret}
                      <IconButton icon="generate" label="Сгенерировать пароль" onclick={() => generate(r)} />
                      <IconButton
                        icon={r.shown ? "eye-off" : "eye"}
                        label={r.shown ? "Скрыть значение" : "Показать значение"}
                        onclick={() => toggleShown(r)}
                      />
                    {/if}
                    <button
                      type="button"
                      class="lock"
                      class:on={r.secret}
                      aria-pressed={r.secret}
                      aria-label="Скрывать значение"
                      title={isSecretName(r.key) ? "Поле с таким именем всегда скрыто" : "Скрывать значение"}
                      disabled={isSecretName(r.key)}
                      onclick={() => {
                        r.secret = !r.secret;
                        r.shown = false;
                      }}
                    >
                      <Icon name={r.secret ? "lock" : "unlock"} size={18} />
                    </button>
                    <IconButton
                      icon="close"
                      label="Удалить поле"
                      onclick={() => (rows = rows.filter((x) => x.uid !== r.uid))}
                    />
                  </div>
                </div>
              {/each}
            </div>
            <div>
              <button type="button" class="add" onclick={() => rows.push(row("", "", false))}>
                <Icon name="plus" stroke={2} />
                Добавить поле
              </button>
            </div>
          </div>

          <div class="group">
            <label for="tags">Теги</label>
            <input id="tags" class="input mono" bind:value={tags} placeholder="work, server, prod" spellcheck="false" />
            <span class="note">Через запятую. В файле запишутся как [work], [server]</span>
          </div>

          <div class="group">
            <label for="note">Заметка</label>
            <textarea id="note" class="input" bind:value={note} placeholder="Необязательно" rows="3"></textarea>
          </div>
        </div>
      </div>

      <div class="bar">
        {#if error}
          <p class="error" role="alert"><Icon name="error" /><span>{error}</span></p>
        {:else}
          <span class="note">После сохранения запись попадёт в {fileName}, файл будет зашифрован заново</span>
        {/if}
        <div class="bar-actions">
          {#if entry}
            <Button variant="ghost" icon="trash" onclick={() => (askDelete = true)}>Удалить запись</Button>
          {/if}
          <Button onclick={leave}>Отмена</Button>
          <Button variant="primary" type="submit" icon="check" disabled={!canSave}>
            {busy ? "Сохраняем…" : entry ? "Сохранить" : "Сохранить запись"}
          </Button>
        </div>
      </div>
    </form>

    <aside>
      <div class="aside-head mono">Так запись {entry ? "лежит" : "ляжет"} в {fileName}</div>
      <div class="scroll aside-body">
        <MarkdownPreview {lines} />
      </div>
      <div class="aside-foot">В предпросмотре скрытые поля показаны точками</div>
    </aside>
  </div>
</div>

<ConfirmDialog open={askLeave} title="Сохранить изменения?" oncancel={() => (askLeave = false)}>
  {entry ? `Вы изменили запись «${entry.title}».` : "Вы начали новую запись."} Если выйти без сохранения, правки пропадут.
  {#snippet actions()}
    <div class="spread">
      <button type="button" class="discard" onclick={() => go("main")}>Не сохранять</button>
      <div class="bar-actions">
        <Button autofocus onclick={() => (askLeave = false)}>Отмена</Button>
        <Button
          variant="primary"
          disabled={!canSave}
          onclick={async () => {
            askLeave = false;
            await save();
          }}>Сохранить</Button
        >
      </div>
    </div>
  {/snippet}
</ConfirmDialog>

{#if entry}
  <ConfirmDialog
    open={askDelete}
    title={`Удалить «${entry.title}»?`}
    icon="trash"
    tone="danger"
    oncancel={() => (askDelete = false)}
  >
    Запись исчезнет из {fileName}.
    {#if app.settings.backupsEnabled}Перед удалением создаётся резервная копия, из неё запись можно вернуть.{/if}
    {#snippet actions()}
      <Button autofocus onclick={() => (askDelete = false)}>Отмена</Button>
      <Button variant="danger" icon="trash" onclick={confirmDelete}>Удалить</Button>
    {/snippet}
  </ConfirmDialog>
{/if}

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

  header :global(.btn) {
    height: 36px;
    color: var(--text-muted);
  }

  .file {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 14px;
    border-radius: 999px;
    border: 1px solid var(--line);
    background: var(--bg-panel);
    font-size: 12px;
    color: var(--text-subtle);
  }

  .body {
    flex: 1;
    min-height: 0;
    display: flex;
  }

  .form {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }

  .content {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: 28px 40px 24px;
  }

  .inner {
    max-width: 720px;
    display: flex;
    flex-direction: column;
    gap: 26px;
  }

  .heading {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  h1 {
    font-size: 26px;
    font-weight: 600;
    letter-spacing: -0.01em;
  }

  .sub {
    color: var(--text-subtle);
  }

  .group {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  label,
  .label {
    font-weight: 500;
  }

  .two {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 16px;
  }

  .input {
    width: 100%;
    height: var(--control-h);
    padding: 0 14px;
    border-radius: var(--radius);
    border: 1px solid var(--line-strong);
    background: var(--bg-panel);
    color: var(--text);
  }

  textarea.input {
    height: auto;
    padding: 12px 14px;
    resize: vertical;
    min-height: 84px;
  }

  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }

  .chip {
    height: 36px;
    padding: 0 16px;
    border-radius: var(--radius);
    font-weight: 500;
    border: 1px solid var(--line-strong);
    color: var(--text-2);
  }

  .chip:hover,
  .path-chip:hover,
  .add:hover {
    background: var(--bg-hover-strong);
  }

  .chip.on {
    border-color: var(--accent);
    background: color-mix(in srgb, var(--accent) 12%, transparent);
    color: var(--text-strong);
  }

  .existing {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
    margin-top: -14px;
    font-size: 12px;
    color: var(--text-subtle);
  }

  .path-chip {
    height: 28px;
    padding: 0 10px;
    border-radius: 6px;
    font-size: 12px;
    border: 1px solid var(--line-strong);
    color: var(--text-muted);
  }

  .path-chip.on {
    border-color: var(--accent);
    color: var(--accent);
  }

  .fields {
    display: flex;
    flex-direction: column;
    gap: 1px;
    background: var(--line);
    border: 1px solid var(--line);
    border-radius: 10px;
    overflow: hidden;
  }

  .field {
    display: grid;
    grid-template-columns: 150px minmax(0, 1fr) auto;
    align-items: center;
    gap: 10px;
    padding: 8px 8px 8px 12px;
    background: var(--bg-panel);
  }

  .field .input {
    height: 40px;
    padding: 0 12px;
    background: var(--bg-chrome);
    font-size: 14px;
  }

  .field .key {
    color: var(--text-muted);
    font-size: 13px;
  }

  .field-actions {
    display: flex;
    gap: 2px;
  }

  .lock {
    width: 40px;
    height: 40px;
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: var(--radius);
    color: var(--text-subtle);
  }

  .lock.on {
    color: var(--accent);
  }

  .lock:hover:not(:disabled) {
    background: var(--bg-hover-strong);
  }

  .lock:disabled {
    opacity: 1;
    cursor: default;
  }

  .add {
    height: 40px;
    display: inline-flex;
    align-items: center;
    gap: 8px;
    padding: 0 14px;
    border-radius: var(--radius);
    border: 1px dashed var(--line-dashed);
    color: var(--text-2);
    font-weight: 500;
  }

  .note {
    font-size: 12px;
    color: var(--text-subtle);
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

  .bar-actions {
    display: flex;
    gap: 10px;
  }

  .bar :global(.btn.ghost) {
    color: var(--danger-text);
  }

  .error {
    display: flex;
    gap: 8px;
    align-items: flex-start;
    color: var(--danger-text);
    font-size: 13px;
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

  .aside-body {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
  }

  .aside-foot {
    padding: 12px 20px;
    border-top: 1px solid var(--line);
    font-size: 12px;
    color: var(--text-subtle);
  }

  .spread {
    width: 100%;
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 10px;
  }

  .discard {
    height: var(--control-h);
    padding: 0 16px;
    border-radius: var(--radius);
    color: var(--danger-text);
    font-weight: 500;
  }

  .discard:hover {
    background: var(--bg-hover-strong);
  }

  @media (max-width: 1100px) {
    aside {
      display: none;
    }
  }
</style>
