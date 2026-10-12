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

let unlocked = false;
let lastVault: string | null = `${HOME}/Documents/vault.md.age`;
let retryUntil = 0;

mockIPC((cmd, args) => {
  const a = (args ?? {}) as Record<string, string>;
  switch (cmd) {
    case "status":
      return {
        unlocked,
        lastVault: lastVault ? describe(lastVault) : null,
        defaultVault: describe(`${HOME}/Documents/vault.md.age`),
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
});
