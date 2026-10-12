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
