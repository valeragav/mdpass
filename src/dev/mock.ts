// Заглушка Rust-части для просмотра интерфейса в обычном браузере (`npm run dev`).
// Подключается только в режиме разработки вне Tauri, в сборку приложения не попадает.
// Верный пароль — «password1». Файл с «broken» в имени открывается с ошибкой целостности.
import { mockIPC } from "@tauri-apps/api/mocks";

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
let settings = {
  autoLockMinutes: 5,
  clipboardClearSeconds: 30,
  lockOnMinimize: true,
  backupsEnabled: true,
  backupsToKeep: 30,
  theme: "dark",
  textSize: "m",
  accent: "#E5A93D",
};
let lastVault: string | null = `${HOME}/Documents/vault.md.age`;
let retryUntil = 0;

mockIPC(
  (cmd, args) => {
  const a = (args ?? {}) as Record<string, string>;
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
        unlocked = true;
        protection = "none";
        return protection;
      }
      if (a.passphrase !== "password1") {
        retryUntil = Date.now() + 3000;
        throw { kind: "wrongPassphrase", retryInMs: 3000 };
      }
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
      return null;
    case "add_entry": {
      // Запись «конфликт» изображает файл, который изменили вне приложения.
      if ((args as { draft: Draft }).draft.title === "конфликт") throw { kind: "changedOnDisk" };
      const id = nextId++;
      ENTRIES.push({ id, ...applyDraft((args as { draft: Draft }).draft) });
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
        at = same.length ? same[same.length - 1] + 1 : ENTRIES.length;
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
      const at = n.before ? ENTRIES.findIndex((e) => n.before!.every((part, k) => e.path[k] === part)) : -1;
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
      return null;
    case "restore_damaged_vault":
      if ((args as { passphrase: string }).passphrase !== "password1") throw { kind: "wrongPassphrase", retryInMs: 3000 };
      unlocked = true;
      return null;
    case "export_backup":
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
      if (n.protection !== "none" && (n.new ?? "").length < 8)
        throw { kind: "invalid", message: "Мастер-пароль должен быть не короче 8 символов" };
      protection = n.protection;
      if (lastVault) {
        lastVault = protection === "none" ? lastVault.replace(/\.age$/, "") : lastVault.replace(/(\.md)?$/, ".md.age").replace(".md.age.md.age", ".md.age");
      }
      return { protection, vault: { ...describe(lastVault ?? ""), encrypted: protection !== "none", exists: true } };
    }
    case "update_settings":
      settings = (args as { settings: typeof settings }).settings;
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
