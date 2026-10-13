import { defineConfig, devices } from "@playwright/test";
// @ts-expect-error type error without @types/node package
import process from "node:process";

// Тесты интерфейса в невидимом браузере. Команды Rust заменяет заглушка src/dev/mock.ts,
// поэтому здесь проверяются экраны и сценарии, а не шифрование и файлы: их проверяют
// тесты Rust. Что видно только в окне приложения — в docs/manual-check.md.
export default defineConfig({
  testDir: "tests/ui",
  fullyParallel: true,
  retries: process.env.CI ? 1 : 0,
  reporter: process.env.CI ? "github" : "list",
  use: {
    baseURL: "http://127.0.0.1:1420",
    viewport: { width: 1280, height: 800 },
    locale: "ru-RU",
    trace: "retain-on-failure",
  },
  projects: [{ name: "chromium", use: { ...devices["Desktop Chrome"], viewport: { width: 1280, height: 800 } } }],
  webServer: {
    command: "npm run dev",
    url: "http://127.0.0.1:1420",
    reuseExistingServer: !process.env.CI,
    timeout: 60_000,
  },
});
