<script lang="ts">
  // Окно выбора версии, когда запись изменили на двух устройствах. В файле уже лежат
  // обе версии: выбор сворачивает их в одну или оставляет как есть. Значения секретов
  // приходят только по «Показать пароли», как и в карточке записи.
  import { api, errorText, type ConflictChoice, type ConflictRow, type ConflictView } from "../api";
  import { showToast } from "../toast.svelte";
  import { loadEntries, pending } from "../vault.svelte";
  import Button from "./Button.svelte";
  import ConfirmDialog from "./ConfirmDialog.svelte";
  import { IS_MOBILE } from "../platform";

  const PART: Record<string, string> = { title: "Название", section: "Раздел", tags: "Теги", extra: "Строки без ключа" };

  let conflict = $derived<ConflictView | undefined>(pending.conflicts[0]);
  let busy = $state(false);
  /// Показанные значения секретов: ключ `${сторона}:${ключ поля}`.
  let revealed = $state<Record<string, string>>({});

  // Новый конфликт — секреты снова скрыты.
  $effect(() => {
    void conflict?.entryId;
    void conflict?.title;
    revealed = {};
  });

  let other = $derived(conflict?.theirDevice ?? "Другое устройство");
  /// Как назвать это устройство: телефон или компьютер.
  const here = IS_MOBILE ? "Этот телефон" : "Этот компьютер";
  let hasSecrets = $derived(conflict?.rows.some((r) => r.secret) ?? false);
  let shown = $derived(Object.keys(revealed).length > 0);

  const time = (ms: number | null) =>
    ms === null
      ? "—"
      : new Date(ms).toLocaleString("ru-RU", { day: "numeric", month: "long", hour: "2-digit", minute: "2-digit" });

  function cell(row: ConflictRow, side: "mine" | "theirs"): string {
    const field = side === "mine" ? row.mineField : row.theirsField;
    if (!row.part && field === null) return "нет поля";
    if (row.secret) return revealed[`${side}:${row.key}`] ?? "••••••••";
    const value = side === "mine" ? row.mine : row.theirs;
    return value === null || value === "" ? "—" : value;
  }

  async function toggleSecrets() {
    if (!conflict) return;
    if (shown) {
      revealed = {};
      return;
    }
    const next: Record<string, string> = {};
    try {
      for (const row of conflict.rows.filter((r) => r.secret)) {
        if (row.mineField !== null && conflict.copyId !== null) {
          next[`mine:${row.key}`] = await api.revealField(conflict.copyId, row.mineField);
        }
        if (row.theirsField !== null) {
          next[`theirs:${row.key}`] = await api.revealField(conflict.entryId, row.theirsField);
        }
      }
      revealed = next;
    } catch (err) {
      showToast(errorText(err));
    }
  }

  async function choose(choice: ConflictChoice, done: string) {
    busy = true;
    try {
      await api.resolveConflict(0, choice);
      showToast(done);
    } catch (err) {
      showToast(errorText(err));
    } finally {
      revealed = {};
      busy = false;
      await loadEntries();
    }
  }

  // Закрыть окно, не выбирая: обе версии остаются в файле, окно спросит снова
  // после следующего перечитывания записей.
  function later() {
    pending.conflicts = [];
  }

  let titles = $derived(
    conflict?.kind === "deletedHere"
      ? "Удалённую запись изменили на другом устройстве"
      : conflict?.kind === "deletedThere"
        ? "Запись удалили на другом устройстве"
        : "Запись изменили на двух устройствах",
  );
</script>

<ConfirmDialog open={conflict !== undefined} title={titles} oncancel={later} wide={conflict?.kind === "edited"}>
  {#if conflict}
    {#if pending.conflicts.length > 1}
      <p class="count">1 из {pending.conflicts.length}</p>
    {/if}
    {#if conflict.kind === "edited"}
      <p>
        Пока вы правили здесь, ту же запись изменили {conflict.theirDevice ? `на устройстве «${conflict.theirDevice}»` : "на другом устройстве"}.
        Выберите, какую версию оставить. Остальные изменения уже объединены.
      </p>
      <p class="where">
        {#if conflict.path.length > 0}{conflict.path.join(" / ")} · {/if}<strong>{conflict.title}</strong>
      </p>
      <table>
        <thead>
          <tr>
            <th scope="col">Поле</th>
            <th scope="col" class="mine">{here}</th>
            <th scope="col" class="theirs">{other}</th>
          </tr>
        </thead>
        <tbody>
          {#each conflict.rows as row (row.key)}
            <tr>
              <th scope="row" class:mono={!row.part}>{row.part ? PART[row.key] : row.key}</th>
              <td class:mono={!row.part} class:masked={row.secret && !shown} data-label={here}>{cell(row, "mine")}</td>
              <td class:mono={!row.part} class:masked={row.secret && !shown} data-label={other}>{cell(row, "theirs")}</td>
            </tr>
          {/each}
          {#if conflict.myAtMs !== null || conflict.theirAtMs !== null}
            <tr class="when">
              <th scope="row">Изменено</th>
              <td data-label={here}>{time(conflict.myAtMs)}</td>
              <td data-label={other}>{time(conflict.theirAtMs)}</td>
            </tr>
          {/if}
        </tbody>
      </table>
      {#if hasSecrets}
        <Button variant="ghost" size="sm" icon="eye" onclick={toggleSecrets}>{shown ? "Скрыть пароли" : "Показать пароли"}</Button>
      {/if}
    {:else if conflict.kind === "deletedHere"}
      <p>
        Вы удалили запись «{conflict.title}», а {conflict.theirDevice ? `на устройстве «${conflict.theirDevice}»` : "на другом устройстве"}
        её в это время изменили. Чтобы правка не пропала, запись вернулась.
      </p>
    {:else}
      <p>
        {conflict.theirDevice ? `На устройстве «${conflict.theirDevice}»` : "На другом устройстве"} запись «{conflict.title}» удалили,
        а вы её в это время изменили. Чтобы правка не пропала, запись осталась.
      </p>
    {/if}
  {/if}
  {#snippet actions()}
    {#if conflict?.kind === "edited"}
      <Button disabled={busy} onclick={() => choose("both", "Оставлены обе версии")}>Оставить обе</Button>
      <Button disabled={busy} onclick={() => choose("theirs", "Оставлена версия с другого устройства")}>
        Версию с другого устройства
      </Button>
      <Button variant="primary" autofocus disabled={busy} onclick={() => choose("mine", "Оставлена ваша версия")}>
        Мою версию
      </Button>
    {:else if conflict?.kind === "deletedHere"}
      <Button disabled={busy} onclick={() => choose("theirs", "Запись оставлена")}>Оставить</Button>
      <Button variant="primary" autofocus disabled={busy} onclick={() => choose("mine", "Запись удалена")}>Удалить</Button>
    {:else}
      <Button disabled={busy} onclick={() => choose("theirs", "Запись удалена")}>Удалить</Button>
      <Button variant="primary" autofocus disabled={busy} onclick={() => choose("mine", "Запись оставлена")}>Оставить</Button>
    {/if}
  {/snippet}
</ConfirmDialog>

<style>
  .count {
    color: var(--text-muted);
    font-size: 13px;
  }

  .where {
    margin-top: 12px;
    color: var(--text-muted);
    font-size: 13px;
  }

  .where strong {
    color: var(--text);
    font-weight: 600;
  }

  table {
    width: 100%;
    margin: 10px 0 6px;
    border-collapse: collapse;
    border: 1px solid var(--line);
    border-radius: var(--radius);
    table-layout: fixed;
    font-size: 13px;
  }

  th,
  td {
    padding: 10px 12px;
    border-top: 1px solid var(--line);
    text-align: left;
    vertical-align: top;
    overflow-wrap: anywhere;
    white-space: pre-line;
  }

  thead th {
    border-top: 0;
    background: var(--bg-chrome);
    font-size: 12px;
    font-weight: 600;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--text-subtle);
  }

  thead th:first-child {
    width: 120px;
  }

  thead .mine {
    color: var(--accent);
  }

  thead .theirs {
    color: var(--info);
  }

  tbody th {
    font-weight: 400;
    color: var(--text-muted);
  }

  .masked {
    letter-spacing: 0.16em;
  }

  .when th,
  .when td {
    background: var(--bg-chrome);
    font-size: 12px;
    color: var(--text-muted);
  }

  /* Узкий экран: вместо колонок — карточка на каждое поле, подписи у значений. */
  @media (max-width: 640px) {
    table,
    tbody,
    tr,
    th,
    td {
      display: block;
    }

    thead {
      display: none;
    }

    tbody th {
      padding-bottom: 4px;
      font-weight: 500;
      color: var(--text);
    }

    td {
      border-top: 0;
      padding-top: 4px;
    }

    td::before {
      content: attr(data-label);
      display: block;
      margin-bottom: 2px;
      font-family: var(--font-sans);
      font-size: 11px;
      letter-spacing: 0;
      color: var(--text-subtle);
    }
  }
</style>
