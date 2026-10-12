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
    case "plugin:opener|reveal_item_in_dir":
      return null;
    case "analyze_import": {
      const fixes = (args as { fixes: Record<string, boolean> }).fixes;
      const places = [
        { path: ["work", "server"], title: "vps1 (инфра)" },
        { path: ["work", "server"], title: "vps2 (бизнес)" },
        { path: ["work", "site"], title: "leading" },
        { path: ["my"], title: "почта" },
      ];
      if (!fixes.emptySections) places.push({ path: ["my"], title: "заметки" });
      return {
        entries: places.length,
        sections: 4,
        fields: 9,
        tags: 3,
        issues: [
          { kind: "legacyTags", titles: ["leading"] },
          { kind: "duplicateNotes", titles: ["БД анкеты"] },
          { kind: "emptySections", titles: ["заметки"] },
          { kind: "missingSpace", titles: ["vps2 (бизнес)"] },
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
