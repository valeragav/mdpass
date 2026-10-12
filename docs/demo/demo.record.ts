// Сценарий для docs/demo.gif. Запуск: npm run demo:gif.
// Идёт в невидимом браузере на заглушке с выдуманными данными (src/dev/mock.ts).
import { test, type Locator, type Page } from "@playwright/test";

// В записи браузера курсора нет, поэтому рисуем свой: кружок, который следует за мышью.
const CURSOR = `
  addEventListener("DOMContentLoaded", () => {
    const dot = document.createElement("div");
    dot.style.cssText = "position:fixed;z-index:99999;width:18px;height:18px;margin:-9px 0 0 -9px;border-radius:50%;" +
      "background:rgba(229,169,61,.35);border:2px solid #e5a93d;pointer-events:none;transition:transform .08s;left:-50px;top:-50px";
    document.body.appendChild(dot);
    addEventListener("mousemove", (e) => { dot.style.left = e.clientX + "px"; dot.style.top = e.clientY + "px"; }, true);
    addEventListener("mousedown", () => (dot.style.transform = "scale(.7)"), true);
    addEventListener("mouseup", () => (dot.style.transform = ""), true);
  });
`;

async function moveTo(page: Page, target: Locator) {
  const box = await target.boundingBox();
  if (!box) throw new Error("элемент не виден");
  await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2, { steps: 18 });
}

async function click(page: Page, target: Locator, pause = 500) {
  await moveTo(page, target);
  await page.waitForTimeout(150);
  await target.click();
  await page.waitForTimeout(pause);
}

async function press(page: Page, key: string, pause = 450) {
  await page.keyboard.press(key);
  await page.waitForTimeout(pause);
}

test("демо mdpass", async ({ page }) => {
  test.setTimeout(180_000);
  await page.addInitScript(CURSOR);
  await page.goto("/");
  await page.mouse.move(640, 600);
  await page.waitForTimeout(800);

  // Разблокировка.
  await page.getByLabel("Мастер-пароль").pressSequentially("password1", { delay: 70 });
  await click(page, page.getByRole("button", { name: "Разблокировать" }), 900);

  // Щелчок по разделу: справа все его записи с полями. Секрет скрыт, пока его не показали.
  const list = page.getByRole("navigation", { name: "Записи" });
  const server = list.getByRole("button", { name: "server", exact: true });
  await click(page, server, 900);
  await click(page, page.getByRole("button", { name: "Показать password" }).first(), 1000);

  // Стрелки и Ctrl+C: курсор идёт по записям и полям раздела.
  // Щелчок по кнопке поставил курсор на поле: ← возвращает его в список слева.
  await page.mouse.move(1180, 760, { steps: 10 });
  await press(page, "ArrowLeft", 600);
  await press(page, "Enter", 700);
  await press(page, "ArrowDown", 700);
  await press(page, "Control+c", 1400);
  await press(page, "ArrowLeft", 500);

  // По списку слева — тоже стрелками; → открывает поля записи, пробел показывает секрет.
  await press(page, "ArrowDown");
  await press(page, "ArrowDown");
  await press(page, "ArrowDown", 600);
  await press(page, "ArrowRight");
  await press(page, "ArrowDown");
  await press(page, "ArrowDown");
  await press(page, " ", 1100);
  await press(page, "ArrowLeft", 400);

  // Поиск.
  const search = page.getByRole("searchbox", { name: "Поиск" });
  await click(page, search, 200);
  await search.pressSequentially("почт", { delay: 90 });
  await page.waitForTimeout(900);
  await search.fill("");
  await page.waitForTimeout(400);

  // Все разделы разом.
  await click(page, page.getByRole("button", { name: "Свернуть все" }), 900);
  await click(page, page.getByRole("button", { name: "Развернуть все" }), 700);

  // Перетаскивание записи в другой раздел.
  const pochta = list.getByRole("button", { name: /почта/ });
  const site = list.getByRole("button", { name: "site", exact: true });
  await moveTo(page, pochta);
  await page.mouse.down();
  const box = await site.boundingBox();
  if (box) await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2, { steps: 30 });
  await page.waitForTimeout(400);
  await page.mouse.up();
  await page.waitForTimeout(1100);

  // Новая запись в выбранном разделе: генератор пароля и порядок полей перетаскиванием.
  await click(page, site, 400);
  await click(page, page.getByRole("button", { name: "Новая запись" }), 700);
  await page.getByLabel("Название").pressSequentially("web-03 (тест)", { delay: 60 });
  await click(page, page.getByLabel("Значение поля ssh"), 100);
  await page.getByLabel("Значение поля ssh").pressSequentially("deploy@192.0.2.7", { delay: 45 });
  await click(page, page.getByRole("button", { name: "Сгенерировать пароль" }), 800);
  const grip = page.getByRole("button", { name: "Переместить поле password" });
  const top = await page.getByRole("button", { name: "Переместить поле ssh" }).boundingBox();
  await moveTo(page, grip);
  await page.mouse.down();
  if (top) await page.mouse.move(top.x + top.width / 2, top.y + top.height / 2 - 4, { steps: 20 });
  await page.waitForTimeout(300);
  await page.mouse.up();
  await page.waitForTimeout(700);
  await click(page, page.getByRole("button", { name: "Сохранить запись" }), 1500);

  // Все горячие клавиши — в настройках.
  await click(page, page.getByRole("button", { name: "Настройки" }), 600);
  await click(page, page.getByRole("button", { name: "Горячие клавиши" }), 2200);
});
