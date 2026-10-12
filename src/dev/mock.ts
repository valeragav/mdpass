// Заглушка Rust-части для просмотра интерфейса в обычном браузере (`npm run dev`).
// Подключается только в режиме разработки вне Tauri, в сборку приложения не попадает.
// Верный пароль — «password1». Файл с «broken» в имени открывается с ошибкой целостности.
import { mockIPC } from "@tauri-apps/api/mocks";

const HOME = "/home/demo";
const describe = (path: string) => ({
  path,
  display: path.startsWith(HOME) ? "~" + path.slice(HOME.length) : path,
  name: path.split("/").pop() ?? path,
  exists: !path.includes("missing"),
});

// Выдуманные данные: адреса из диапазонов для документации, пароли случайные.
const ENTRIES = [
  { id: 1, path: ["work", "server"], title: "alpha (инфра)", tags: ["prod"], fields: [
    { key: "ssh", value: "deploy@203.0.113.10", secret: false },
    { key: "password", value: "q7#Lm2vX9pTe", secret: true },
  ] },
  { id: 2, path: ["work", "server"], title: "beta (стейдж)", tags: ["dev"], fields: [
    { key: "ssh", value: "deploy@198.51.100.24", secret: false },
    { key: "password", value: "Zk4!rW8nYb1s", secret: true },
  ] },
  { id: 3, path: ["work", "server"], title: "БД заказов", tags: [], fields: [
    { key: "port", value: "5432", secret: false },
    { key: "user", value: "orders_app", secret: false },
    { key: "password", value: "hV3$eQ6mJ0cA", secret: true },
    { key: "note", value: "таблицу orders_tmp можно удалять", secret: false },
  ] },
  { id: 4, path: ["work", "site"], title: "example.test", tags: ["work", "prod"], fields: [
    { key: "url", value: "https://example.test/", secret: false },
    { key: "url_admin", value: "https://example.test/admin", secret: false },
    { key: "login", value: "admin", secret: false },
    { key: "password", value: "Tn5&uP2wGx7d", secret: true },
  ] },
  { id: 5, path: ["my"], title: "почта", tags: [], fields: [
    { key: "login", value: "me@example.test", secret: false },
    { key: "password", value: "fR9^kL3zVm6q", secret: true },
  ] },
];

let nextId = 100;

interface Draft {
  path: string[];
  title: string;
  tags: string[];
  fields: { key: string; value: string | null; secret: boolean; keepFrom: number | null }[];
}

function applyDraft(draft: Draft, current?: (typeof ENTRIES)[number]) {
  return {
    path: draft.path,
    title: draft.title,
    tags: draft.tags,
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
        defaultVault: describe(`${HOME}/Documents/vault.md.age`),
        settings,
      };
    case "describe_path":
      return describe(a.path);
    case "create_vault":
      unlocked = true;
      lastVault = a.path;
      return null;
    case "unlock": {
      if (Date.now() < retryUntil) throw { kind: "tooSoon", retryInMs: retryUntil - Date.now() };
      if (a.path.includes("broken")) throw { kind: "damaged", detail: "authentication tag mismatch" };
      if (a.passphrase !== "password1") {
        retryUntil = Date.now() + 3000;
        throw { kind: "wrongPassphrase", retryInMs: 3000 };
      }
      unlocked = true;
      lastVault = a.path;
      return null;
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
    case "add_entry": {
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
    case "delete_entry": {
      const i = ENTRIES.findIndex((e) => e.id === (args as { entryId: number }).entryId);
      if (i < 0) throw { kind: "entryNotFound" };
      ENTRIES.splice(i, 1);
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
    case "preview_backup": {
      const id = (args as { id: string }).id;
      if (id.includes("passphrase")) throw { kind: "otherPassphrase" };
      if (id.includes("add")) return [{ kind: "removed", title: "example.test" }, { kind: "changed", title: "БД заказов", keys: ["note"] }];
      return [{ kind: "changed", title: "БД заказов", keys: ["note"] }];
    }
    case "restore_backup":
      return null;
    case "restore_damaged_vault":
      if ((args as { passphrase: string }).passphrase !== "password1") throw { kind: "wrongPassphrase", retryInMs: 3000 };
      unlocked = true;
      return null;
    case "export_backup":
      return null;
    case "touch":
      return null;
    case "change_passphrase": {
      const n = args as { current: string; new: string };
      if (n.current !== "password1") throw { kind: "wrongPassphrase", retryInMs: 3000 };
      if (n.new.length < 8) throw { kind: "invalid", message: "Новый мастер-пароль должен быть не короче 8 символов" };
      return null;
    }
    case "update_settings":
      settings = (args as { settings: typeof settings }).settings;
      return settings;
    case "lock":
      unlocked = false;
      return null;
    case "plugin:dialog|open":
      return `${HOME}/Documents/broken.md.age`;
    case "plugin:dialog|save":
      return `${HOME}/Documents/new.md.age`;
    default:
      throw new Error(`нет заглушки для команды ${cmd}`);
  }
},
  { shouldMockEvents: true },
);
