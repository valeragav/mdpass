import { defineConfig } from "@playwright/test";

// Запись видео для docs/demo.gif. Обычные тесты интерфейса — в playwright.config.ts в корне.
export default defineConfig({
  testDir: ".",
  testMatch: "demo.record.ts",
  outputDir: "../../target/demo",
  reporter: "list",
  use: {
    baseURL: "http://127.0.0.1:1420",
    viewport: { width: 1280, height: 800 },
    locale: "ru-RU",
    video: { mode: "on", size: { width: 1280, height: 800 } },
  },
  webServer: {
    command: "npm run dev",
    url: "http://127.0.0.1:1420",
    reuseExistingServer: true,
    timeout: 60_000,
  },
});
