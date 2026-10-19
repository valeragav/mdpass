// Заглушка Rust-части для просмотра интерфейса в обычном браузере (`npm run dev`).
// Подключается только в режиме разработки вне Tauri, в сборку приложения не попадает.
// Верный пароль — «password1». Файл с «broken» в имени открывается с ошибкой целостности.
// «work.md.age» из недавних открывается с большим набором записей.
import { mockIPC } from "@tauri-apps/api/mocks";
import { isSecretName } from "../lib/entryText";
import { minMasterLength } from "../lib/text";

const HOME = "/home/demo";
const describe = (path: string) => ({
  path,
  display: path.startsWith(HOME) ? "~" + path.slice(HOME.length) : path,
  name: path.split("/").pop() ?? path,
  exists: !path.includes("missing") && !path.includes("new"),
  encrypted: path.includes("missing") || path.includes("new") ? null : path.endsWith(".age"),
});
let protection: "strong" | "weak" | "none" = "strong";

// Выдуманные данные: адреса из диапазонов для документации, пароли случайные.
const ENTRIES = [
  { id: 1, path: ["work", "server"], title: "web-01 (прод)", tags: ["prod"], favorite: true, favoriteOrder: null as number | null, fields: [
    { key: "ssh", value: "deploy@203.0.113.10", secret: false },
    { key: "password", value: "q7#Lm2vX9pTe", secret: true },
  ] },
  { id: 2, path: ["work", "server"], title: "web-02 (тест)", tags: ["dev"], favorite: false, favoriteOrder: null as number | null, fields: [
    { key: "ssh", value: "deploy@198.51.100.24", secret: false },
    { key: "password", value: "Zk4!rW8nYb1s", secret: true },
  ] },
  { id: 3, path: ["work", "server"], title: "БД каталога", tags: [], favorite: false, favoriteOrder: null as number | null, fields: [
    { key: "port", value: "5432", secret: false },
    { key: "user", value: "catalog_app", secret: false },
    { key: "password", value: "hV3$eQ6mJ0cA", secret: true },
    { key: "note", value: "таблицу catalog_tmp можно удалять", secret: false },
  ] },
  { id: 4, path: ["work", "site"], title: "example.test", tags: ["work", "prod"], favorite: false, favoriteOrder: null as number | null, fields: [
    { key: "url", value: "https://example.test/", secret: false },
    { key: "url_admin", value: "https://example.test/admin", secret: false },
    { key: "login", value: "admin", secret: false },
    { key: "password", value: "Tn5&uP2wGx7d", secret: true },
  ] },
  { id: 5, path: ["my"], title: "почта", tags: [], favorite: true, favoriteOrder: null as number | null, fields: [
    { key: "login", value: "me@example.test", secret: false },
    { key: "password", value: "fR9^kL3zVm6q", secret: true },
  ] },
];

let nextId = 100;

// Большое хранилище для ручной проверки: «work.md.age» в недавних, пароль тот же.
// Тесты открывают vault.md.age, поэтому эти данные их не задевают.
const BIG_VAULT = "work.md.age";

function bigVault(): MockEntry[] {
  // Пароли выдуманные, но каждый раз одни и те же.
  let seed = 7;
  const chars = "abcdefghijkmnpqrstuvwxyzABCDEFGHJKLMNPQRSTUVWXYZ23456789!#$%&*";
  const secret = () =>
    Array.from({ length: 14 }, () => chars[(seed = (seed * 48271) % 2147483647) % chars.length]).join("");
  let id = 1000;
  const out: MockEntry[] = [];
  const add = (path: string, title: string, tags: string[], fields: [string, string][], favorite: number | boolean = false) =>
    out.push({
      id: id++,
      path: path ? path.split(" / ") : [],
      title,
      tags,
      favorite: favorite !== false,
      favoriteOrder: typeof favorite === "number" ? favorite : null,
      fields: fields.map(([key, value]) => ({ key, value: value === "*" ? secret() : value, secret: isSecretName(key) })),
    });

  add("", "Wi-Fi дома", ["home"], [["ssid", "home-5g"], ["password", "*"]], 3);
  add("", "Сейф в шкафу", [], [["pin", "*"], ["note", "код меняли в марте"]]);

  for (const [n, ip] of [[1, "203.0.113.11"], [2, "203.0.113.12"], [3, "203.0.113.13"], [4, "203.0.113.14"]] as const) {
    add("work / servers / prod", `web-0${n} (прод)`, ["prod", "web"], [["ssh", `deploy@${ip}`], ["password", "*"]], n === 1 ? 1 : false);
  }
  add("work / servers / prod", "deploy-ключ", ["ssh"], [["user", "deploy"], ["private_key", "-----BEGIN KEY-----\nAAAAexample\nBBBBexample\n-----END KEY-----"]]);
  add("work / servers / prod", "balancer-01", ["prod", "net"], [["ssh", "root@203.0.113.2"], ["password", "*"], ["note", "перезапуск только в окно обслуживания\nпосле — проверить health-check"]]);
  add("work / servers / prod", "очередь задач (RabbitMQ)", ["prod"], [["url", "https://mq.example.test/"], ["login", "ops"], ["password", "*"]], true);
  for (const n of [1, 2]) {
    add("work / servers / stage", `stage-0${n}`, ["stage", "web"], [["ssh", `deploy@198.51.100.${30 + n}`], ["password", "*"]]);
  }
  add("work / servers / stage", "stage-db с очень длинным названием, чтобы проверить обрезку строки", ["stage", "db"], [["host", "198.51.100.40"], ["port", "5432"], ["password", "*"]]);
  add("work / databases", "PostgreSQL каталог", ["prod", "db"], [["host", "db.example.test"], ["port", "5432"], ["user", "catalog_app"], ["password", "*"]], 2);
  add("work / databases", "PostgreSQL отчёты (реплика)", ["prod", "db", "readonly"], [["host", "replica.example.test"], ["port", "5433"], ["user", "reports_ro"], ["password", "*"]]);
  add("work / databases", "Redis кеш", ["prod", "db"], [["host", "cache.example.test"], ["port", "6379"], ["token", "*"]]);
  add("work / databases", "ClickHouse аналитика", ["db", "analytics"], [["url", "https://ch.example.test:8443/"], ["user", "analyst"], ["password", "*"], ["note", "лимит запроса — 60 секунд"]]);
  add("work / cloud / aws", "AWS root", ["cloud", "2fa"], [["login", "root@example.test"], ["password", "*"], ["totp", "*"]], true);
  add("work / cloud / aws", "AWS ci-deployer", ["cloud", "ci"], [["access_key", "AKIAEXAMPLE0000DEMO"], ["secret_key", "*"]]);
  add("work / cloud / aws / s3", "бакет бэкапов", ["cloud", "backup"], [["bucket", "backups-example-test"], ["region", "eu-central-1"]]);
  add("work / cloud / gcp", "GCP проект analytics", ["cloud", "analytics"], [["project", "analytics-example"], ["token", "*"]]);
  add("work / ci", "GitHub Actions — деплой", ["ci"], [["url", "https://github.example.test/org/app"], ["token", "*"]], true);
  add("work / ci", "Docker registry", ["ci"], [["url", "https://registry.example.test/"], ["login", "ci-bot"], ["password", "*"]]);
  add("work / ci", "SonarQube", ["ci"], [["url", "https://sonar.example.test/"], ["token", "*"]]);
  add("work / сайты", "example.test — админка", ["web", "prod"], [["url", "https://example.test/admin"], ["login", "admin"], ["password", "*"]], true);
  add("work / сайты", "блог (WordPress)", ["web"], [["url", "https://blog.example.test/wp-admin"], ["login", "editor"], ["password", "*"]]);
  add("work / сайты", "лендинг акции", ["web", "stage"], [["url", "https://promo.example.test/"], ["login", "marketing"], ["password", "*"]]);
  add("work / почта", "служебный ящик", ["mail"], [["login", "noreply@example.test"], ["password", "*"], ["smtp", "smtp.example.test:587"]]);
  add("work / почта", "ящик поддержки", ["mail"], [["login", "support@example.test"], ["password", "*"]]);

  add("clients / Acme", "Acme — VPN", ["vpn", "client"], [["server", "vpn.acme.example"], ["login", "contractor-7"], ["password", "*"]], true);
  add("clients / Acme", "Acme — Jira", ["client"], [["url", "https://acme.example/jira"], ["login", "contractor-7"], ["password", "*"]]);
  add("clients / Acme / стенды", "Acme stage", ["client", "stage"], [["ssh", "user@192.0.2.20"], ["password", "*"]]);
  add("clients / Globex", "Globex — портал", ["client"], [["url", "https://portal.globex.example/"], ["login", "me@example.test"], ["password", "*"], ["note", "доступ до конца квартала"]]);
  add("clients / Globex", "Globex — FTP", ["client", "legacy"], [["host", "ftp.globex.example"], ["login", "upload"], ["password", "*"]]);

  add("my", "почта", ["mail"], [["login", "me@example.test"], ["password", "*"]], true);
  add("my", "вторая почта", ["mail"], [["login", "me.backup@example.test"], ["password", "*"]]);
  add("my / банки", "Банк — онлайн", ["bank", "2fa"], [["login", "me1985"], ["password", "*"], ["note", "кодовое слово — в бумажной записной книжке"]], true);
  add("my / банки", "Банк — карта", ["bank"], [["pin", "*"]]);
  add("my / банки", "брокерский счёт", ["bank", "2fa"], [["login", "me1985"], ["password", "*"], ["otp", "*"]]);
  add("my / подписки", "музыка", ["subscription"], [["login", "me@example.test"], ["password", "*"]]);
  add("my / подписки", "кино", ["subscription"], [["login", "me@example.test"], ["password", "*"]], true);
  add("my / подписки", "облачное хранилище", ["subscription", "backup"], [["login", "me@example.test"], ["password", "*"]]);
  add("my / дом", "роутер", ["home", "net"], [["url", "http://192.0.2.1/"], ["login", "admin"], ["password", "*"]], true);
  add("my / дом", "умная колонка", ["home"], [["login", "me@example.test"], ["password", "*"]]);
  add("my / дом / дача", "камера у ворот", ["home"], [["url", "http://192.0.2.50/"], ["login", "viewer"], ["password", "*"]]);
  add("my / документы", "Госуслуги", ["gov", "2fa"], [["login", "+7 900 000-00-00"], ["password", "*"]], true);
  add("my / игры", "Steam", ["games"], [["login", "player_one"], ["password", "*"]]);
  return out;
}

// Записи каждого хранилища заглушки; ENTRIES — записи открытого.
const vaults: Record<string, MockEntry[]> = {};
let openVaultName = "vault.md.age";

function openVault(path: string) {
  const name = path.split("/").pop() ?? path;
  // Остальные файлы заглушки показывают одни и те же записи, как и раньше.
  if (name === openVaultName || (name !== BIG_VAULT && openVaultName !== BIG_VAULT)) return;
  vaults[openVaultName] = ENTRIES.slice();
  const next = vaults[name] ?? (name === BIG_VAULT ? bigVault() : vaults["vault.md.age"]);
  ENTRIES.splice(0, ENTRIES.length, ...next);
  openVaultName = name;
}

// Резервные копии заглушки: какими записи были в момент копии.
type MockEntry = (typeof ENTRIES)[number];
const samePlace = (a: MockEntry, b: MockEntry) => a.path.join("/") === b.path.join("/") && a.title === b.title;
const BACKUPS: Record<string, MockEntry[]> = (() => {
  const copy = () => structuredClone(ENTRIES);
  const withOldValues = (entries: MockEntry[]) => {
    entries[0].fields[1].value = "Old#Pass-2025";
    entries[2].fields[3].value = "таблицу catalog_tmp пока не трогать";
    return entries;
  };
  return {
    "20261012T084203123Z-update.age": withOldValues(copy()),
    "20261012T071500000Z-add.age": withOldValues(copy()).filter((e) => e.title !== "example.test"),
    "20261011T150300000Z-delete.age": [
      ...copy(),
      { id: 900, path: ["my"], title: "роутер", tags: [], favorite: false, favoriteOrder: null, fields: [
        { key: "login", value: "admin", secret: false },
        { key: "password", value: "R0uter!pass", secret: true },
      ] },
    ],
  };
})();

function backupDiff(backup: MockEntry[]) {
  const out: unknown[] = [];
  for (const entry of ENTRIES) {
    const old = backup.find((b) => samePlace(b, entry));
    if (!old) {
      out.push({ kind: "removed", path: entry.path, title: entry.title });
      continue;
    }
    const value = (e: MockEntry, key: string) => e.fields.find((f) => f.key === key)?.value;
    const keys = [...new Set([...old.fields, ...entry.fields].map((f) => f.key))].filter(
      (key) => value(old, key) !== value(entry, key),
    );
    const fields = keys.map((key) => {
      const f = old.fields.find((x) => x.key === key);
      return f
        ? { key, secret: f.secret, before: f.secret ? null : f.value, missing: false }
        : { key, secret: false, before: null, missing: true };
    });
    if (old.tags.join() !== entry.tags.join()) {
      fields.push({ key: "tags", secret: false, before: old.tags.map((t) => `[${t}]`).join(", "), missing: false });
    }
    if (fields.length) out.push({ kind: "changed", path: entry.path, title: entry.title, fields });
  }
  for (const old of backup) {
    if (!ENTRIES.some((e) => samePlace(e, old))) out.push({ kind: "added", path: old.path, title: old.title });
  }
  return out;
}

function backupEntry(args: unknown): MockEntry {
  const n = args as { id: string; path: string[]; title: string };
  const entry = BACKUPS[n.id]?.find((e) => e.path.join("/") === n.path.join("/") && e.title === n.title);
  if (!entry) throw { kind: "entryNotFound" };
  return entry;
}

interface Draft {
  path: string[];
  title: string;
  tags: string[];
  favorite: boolean;
  fields: { key: string; value: string | null; secret: boolean; keepFrom: number | null }[];
}

function applyDraft(draft: Draft, current?: (typeof ENTRIES)[number]) {
  return {
    path: draft.path,
    title: draft.title,
    tags: draft.tags,
    favorite: draft.favorite,
    // Форма знает только «в избранном или нет»: номер остаётся прежним.
    favoriteOrder: draft.favorite ? (current?.favoriteOrder ?? null) : null,
    fields: draft.fields.map((f) => ({
      key: f.key,
      value: f.value ?? (f.keepFrom !== null ? (current?.fields[f.keepFrom]?.value ?? "") : ""),
      secret: f.secret,
    })),
  };
}

let unlocked = false;

// Команды, которым нужно открытое хранилище. После блокировки Rust отвечает «locked» —
// заглушка тоже, чтобы тесты ловили обращения к записям после блокировки.
const NEEDS_VAULT = new Set([
  "entries", "reveal_field", "copy_field", "add_entry", "update_entry", "delete_entry", "set_favorite",
  "order_favorites", "move_entry", "move_section", "reload_vault", "preview_backup", "reveal_backup_field",
  "copy_backup_field", "restore_backup_entry", "restore_backup",
]);

/// Rust раздаёт номера записей заново, когда перечитывает файл: заглушка тоже.
function renumber() {
  ENTRIES.forEach((entry, i) => (entry.id = i + 1));
}
let settings = {
  autoLockMinutes: 5,
  clipboardClearSeconds: 30,
  lockOnMinimize: true,
  backupsEnabled: true,
  backupsToKeep: 30,
  theme: "dark",
  textSize: "m",
  accent: "#E5A93D",
  sidebarWidth: 320,
};
// Конфликт слияния в заглушке: обе версии записи уже в списке, выбор их сворачивает.
interface MockConflict {
  title: string;
  copyTitle: string;
  path: string[];
}
let conflicts: MockConflict[] = [];
// Облако в заглушке: подключается сразу, синхронизация — сразу.
const cloudState = { provider: null as "yandex" | "google" | null, clientId: null as string | null, linked: false };
let lastVault: string | null = `${HOME}/Documents/vault.md.age`;
let retryUntil = 0;

mockIPC(
  (cmd, args) => {
  const a = (args ?? {}) as Record<string, string>;
  if (NEEDS_VAULT.has(cmd) && !unlocked) throw { kind: "locked" };
  switch (cmd) {
    case "status":
      return {
        unlocked,
        lastVault: lastVault ? describe(lastVault) : null,
        recentVaults: [lastVault, `${HOME}/Documents/work.md.age`, `${HOME}/Desktop/old.md`]
          .filter((p): p is string => !!p)
          .map(describe),
        defaultVault: describe(`${HOME}/Documents/vault.md.age`),
        protection: unlocked ? protection : null,
        settings,
      };
    case "describe_path":
    case "adopt_file":
      return describe(a.path);
    case "create_vault":
      unlocked = true;
      lastVault = a.path;
      protection = a.protection as typeof protection;
      return protection;
    case "unlock": {
      if (Date.now() < retryUntil) throw { kind: "tooSoon", retryInMs: retryUntil - Date.now() };
      if (a.path.includes("broken")) throw { kind: "damaged", detail: "authentication tag mismatch" };
      if (!a.path.endsWith(".age")) {
        openVault(a.path);
        unlocked = true;
        protection = "none";
        return protection;
      }
      if (a.passphrase !== "password1") {
        retryUntil = Date.now() + 3000;
        throw { kind: "wrongPassphrase", retryInMs: 3000 };
      }
      openVault(a.path);
      unlocked = true;
      lastVault = a.path;
      protection = "strong";
      return protection;
    }
    case "entries":
      return ENTRIES.map((e) => ({ ...e, fields: e.fields.map((f) => ({ ...f, value: f.secret ? null : f.value })) }));
    case "reveal_field":
    case "copy_field": {
      const n = args as { entryId: number; field: number };
      const value = ENTRIES.find((e) => e.id === n.entryId)?.fields[n.field]?.value;
      if (value === undefined) throw { kind: "entryNotFound" };
      return cmd === "reveal_field" ? value : null;
    }
    case "rename_vault": {
      const name = (args as { name: string }).name.replace(/(\.md)?(\.age)?$/, "");
      lastVault = `${HOME}/Documents/${name}${protection === "none" ? ".md" : ".md.age"}`;
      return describe(lastVault);
    }
    case "reload_vault":
      renumber();
      return null;
    case "cloud_status":
      return {
        provider: cloudState.provider,
        connected: cloudState.provider !== null,
        clientId: cloudState.clientId,
        linked: cloudState.linked,
        encrypted: protection !== "none",
        fileName: cloudState.linked ? (lastVault?.split("/").pop() ?? null) : null,
        status: cloudState.linked ? { kind: "synced", atMs: Date.parse("2026-10-16T12:30:00") } : { kind: "idle" },
      };
    case "cloud_connect": {
      const { provider, clientId, token } = args as { provider: "yandex" | "google"; clientId: string; token: string };
      if (!token.trim()) throw { kind: "invalid", message: "Введите токен" };
      cloudState.provider = provider;
      cloudState.clientId = clientId.trim() || null;
      return null;
    }
    case "cloud_logout":
      cloudState.provider = null;
      cloudState.clientId = null;
      cloudState.linked = false;
      return null;
    case "cloud_link":
      cloudState.linked = true;
      return null;
    case "cloud_unlink":
      cloudState.linked = false;
      return null;
    case "cloud_sync_now":
    case "cloud_hold":
      return null;
    case "cloud_list":
      if (cloudState.provider === null) throw { kind: "cloud", message: "Сначала войдите в облако" };
      return ["vault.md.age"];
    case "cloud_download": {
      const path = (args as { path: string }).path;
      lastVault = path;
      // Файл только что скачан: он есть и зашифрован.
      return { ...describe(path), exists: true, encrypted: true };
    }
    case "conflicts":
      return conflicts.flatMap((c) => {
        const original = ENTRIES.find((e) => e.title === c.title);
        const copy = ENTRIES.find((e) => e.title === c.copyTitle);
        if (!original || !copy) return [];
        return [
          {
            kind: "edited",
            path: c.path,
            title: c.title,
            entryId: original.id,
            copyId: copy.id,
            rows: [
              { key: "password", part: false, secret: true, mine: null, theirs: null, mineField: 1, theirsField: 1 },
              {
                key: "url",
                part: false,
                secret: false,
                mine: copy.fields[0].value,
                theirs: original.fields[0].value,
                mineField: 0,
                theirsField: 0,
              },
            ],
            theirDevice: "Ноутбук",
            theirAtMs: Date.parse("2026-10-16T12:38:00"),
            myAtMs: Date.parse("2026-10-16T12:41:00"),
          },
        ];
      });
    case "resolve_conflict": {
      const { index, choice } = args as { index: number; choice: "mine" | "theirs" | "both" };
      const c = conflicts[index];
      if (!c) throw { kind: "entryNotFound" };
      conflicts.splice(index, 1);
      if (choice === "both") return null;
      const original = ENTRIES.find((e) => e.title === c.title);
      const copyAt = ENTRIES.findIndex((e) => e.title === c.copyTitle);
      if (!original || copyAt < 0) throw { kind: "entryNotFound" };
      if (choice === "mine") original.fields = ENTRIES[copyAt].fields;
      ENTRIES.splice(copyAt, 1);
      return null;
    }
    case "add_entry": {
      // Запись «конфликт» изображает файл, который изменили вне приложения так, что
      // слить не вышло; «две версии» — слияние, где запись изменили с двух сторон.
      const title = (args as { draft: Draft }).draft.title;
      if (title === "конфликт") throw { kind: "changedOnDisk" };
      const id = nextId++;
      const draft = (args as { draft: Draft }).draft;
      if (title === "две версии") {
        // Выдуманные значения: url и пароль на «Ноутбуке» и здесь разошлись.
        const version = (url: string, password: string, copyTitle?: string) => ({
          ...applyDraft(draft),
          title: copyTitle ?? title,
          fields: [
            { key: "url", value: url, secret: false },
            { key: "password", value: password, secret: true },
          ],
        });
        ENTRIES.push({ id, ...version("https://web02.example.test", "Lk7#vQ2pZx") });
        const copyTitle = `${title} (копия с этого устройства)`;
        ENTRIES.push({ id: nextId++, ...version("https://web01.example.test", "mD4!rT8wNe", copyTitle) });
        conflicts = [{ title, copyTitle, path: draft.path }];
        return id;
      }
      ENTRIES.push({ id, ...applyDraft(draft) });
      return id;
    }
    case "update_entry": {
      const n = args as { entryId: number; draft: Draft };
      const i = ENTRIES.findIndex((e) => e.id === n.entryId);
      if (i < 0) throw { kind: "entryNotFound" };
      ENTRIES[i] = { id: n.entryId, ...applyDraft(n.draft, ENTRIES[i]) };
      return null;
    }
    case "set_favorite": {
      const n = args as { entryId: number; favorite: boolean };
      const entry = ENTRIES.find((e) => e.id === n.entryId);
      if (!entry) throw { kind: "entryNotFound" };
      if (entry.favorite !== n.favorite) {
        const numbers = ENTRIES.map((e) => e.favoriteOrder ?? 0);
        entry.favoriteOrder = n.favorite && Math.max(...numbers) > 0 ? Math.max(...numbers) + 1 : null;
      }
      entry.favorite = n.favorite;
      return null;
    }
    case "order_favorites": {
      (args as { entryIds: number[] }).entryIds.forEach((id, i) => {
        const entry = ENTRIES.find((e) => e.id === id);
        if (!entry) throw { kind: "entryNotFound" };
        entry.favoriteOrder = i + 1;
      });
      return null;
    }
    case "delete_entry": {
      const i = ENTRIES.findIndex((e) => e.id === (args as { entryId: number }).entryId);
      if (i < 0) throw { kind: "entryNotFound" };
      ENTRIES.splice(i, 1);
      return null;
    }
    case "move_entry": {
      const n = args as { entryId: number; path: string[]; beforeId: number | null };
      const i = ENTRIES.findIndex((e) => e.id === n.entryId);
      const [entry] = ENTRIES.splice(i, 1);
      entry.path = n.path;
      let at = n.beforeId === null ? -1 : ENTRIES.findIndex((e) => e.id === n.beforeId);
      if (at < 0) {
        const same = ENTRIES.map((e, k) => (e.path.join("/") === n.path.join("/") ? k : -1)).filter((k) => k >= 0);
        // Свои записи раздела в файле стоят сразу под заголовком, до подразделов.
        // У верхнего уровня это начало файла, до первого заголовка.
        const sub = ENTRIES.findIndex((e) => n.path.every((part, k) => e.path[k] === part));
        at = same.length ? same[same.length - 1] + 1 : sub >= 0 ? sub : ENTRIES.length;
      }
      ENTRIES.splice(at, 0, entry);
      return null;
    }
    case "move_section": {
      const n = args as { path: string[]; parent: string[]; before: string[] | null };
      const inside = (p: string[]) => n.path.every((part, k) => p[k] === part);
      const block = ENTRIES.filter((e) => inside(e.path));
      for (const e of block) {
        ENTRIES.splice(ENTRIES.indexOf(e), 1);
        e.path = [...n.parent, ...e.path.slice(n.path.length - 1)];
      }
      const under = (p: string[]) => (e: { path: string[] }) => p.every((part, k) => e.path[k] === part);
      // Без «перед кем» раздел встаёт последним в родителе, а не в конце файла.
      const at = n.before
        ? ENTRIES.findIndex(under(n.before))
        : ENTRIES.reduce((last, e, k) => (under(n.parent)(e) ? k + 1 : last), 0);
      ENTRIES.splice(at < 0 ? ENTRIES.length : at, 0, ...block);
      return null;
    }
    case "generate_password":
      return Array.from(crypto.getRandomValues(new Uint8Array(20)), (b) => "abcdefghijkmnpqrstuvwxyzABCDEFGHJKLMNPQRSTUVWXYZ23456789!@#$%&*-_"[b % 65]).join("");
    case "list_backups":
      return [
        { id: "20261012T084203123Z-update.age", createdMs: Date.UTC(2026, 9, 12, 8, 42), kind: "update" },
        { id: "20261012T071500000Z-add.age", createdMs: Date.UTC(2026, 9, 12, 7, 15), kind: "add" },
        { id: "20261011T150300000Z-delete.age", createdMs: Date.UTC(2026, 9, 11, 15, 3), kind: "delete" },
        { id: "20261010T090000000Z-passphrase.age", createdMs: Date.UTC(2026, 9, 10, 9, 0), kind: "passphrase" },
      ];
    case "backups_location":
      return "C:/Users/demo/AppData/Local/io.github.valeragav.mdpass/backups/vault.md.age-1a2b3c4d";
    case "preview_backup": {
      const id = (args as { id: string }).id;
      if (id.includes("passphrase")) throw { kind: "otherPassphrase" };
      return backupDiff(BACKUPS[id] ?? []);
    }
    case "reveal_backup_field": {
      const key = (args as { key: string }).key;
      const field = backupEntry(args).fields.find((f) => f.key === key);
      if (!field) throw { kind: "entryNotFound" };
      return field.value;
    }
    case "copy_backup_field":
      backupEntry(args);
      return null;
    case "restore_backup_entry": {
      const old = structuredClone(backupEntry(args));
      const at = ENTRIES.findIndex((e) => samePlace(e, old));
      if (at >= 0) {
        ENTRIES[at] = { ...old, id: ENTRIES[at].id };
        return ENTRIES[at].id;
      }
      const id = nextId++;
      ENTRIES.push({ ...old, id });
      return id;
    }
    case "restore_backup":
      renumber();
      return null;
    case "restore_damaged_vault":
      if ((args as { passphrase: string }).passphrase !== "password1") throw { kind: "wrongPassphrase", retryInMs: 3000 };
      unlocked = true;
      return null;
    case "export_backup":
      return true;
    case "close_backup":
      return null;
    case "plugin:opener|reveal_item_in_dir":
      return null;
    case "analyze_import": {
      const fixes = (args as { fixes: Record<string, boolean> }).fixes;
      const places = [
        { path: ["work", "server"], title: "node-a (кластер)" },
        { path: ["work", "server"], title: "node-b (резерв)" },
        { path: ["work", "site"], title: "landing" },
        { path: ["my"], title: "почта" },
      ];
      if (!fixes.emptySections) places.push({ path: ["my"], title: "заметки" });
      return {
        entries: places.length,
        sections: 4,
        fields: 9,
        tags: 3,
        issues: [
          { kind: "legacyTags", titles: ["landing"] },
          { kind: "duplicateNotes", titles: ["БД отзывов"] },
          { kind: "emptySections", titles: ["заметки"] },
          { kind: "missingSpace", titles: ["node-b (резерв)"] },
        ],
        places,
      };
    }
    case "run_import":
      return 4;
    case "delete_import_source":
      return null;
    case "touch":
      return null;
    case "change_protection": {
      const n = args as { current: string | null; new: string | null; protection: typeof protection };
      if (protection !== "none" && n.current !== "password1") throw { kind: "wrongPassphrase", retryInMs: 3000 };
      const min = minMasterLength(n.protection);
      if (n.protection !== "none" && (n.new ?? "").length < min)
        throw { kind: "invalid", message: `Мастер-пароль должен быть не короче ${min} символов` };
      protection = n.protection;
      if (lastVault) {
        lastVault = protection === "none" ? lastVault.replace(/\.age$/, "") : lastVault.replace(/(\.md)?$/, ".md.age").replace(".md.age.md.age", ".md.age");
      }
      return { protection, vault: { ...describe(lastVault ?? ""), encrypted: protection !== "none", exists: true } };
    }
    case "update_settings":
      settings = (args as { settings: typeof settings }).settings;
      settings.sidebarWidth = Math.min(640, Math.max(220, settings.sidebarWidth));
      return settings;
    case "lock":
      unlocked = false;
      return null;
    case "plugin:dialog|open": {
      const filters = (args as { options?: { filters?: { extensions: string[] }[] } }).options?.filters ?? [];
      const markdown = filters.some((f) => f.extensions.includes("md"));
      return markdown ? `${HOME}/Documents/passwords.md` : `${HOME}/Documents/broken.md.age`;
    }
    case "plugin:dialog|save":
      return `${HOME}/Documents/new.md.age`;
    default:
      throw new Error(`нет заглушки для команды ${cmd}`);
  }
},
  { shouldMockEvents: true },
);
