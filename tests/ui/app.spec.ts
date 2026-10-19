import { expect, test, type Page } from "@playwright/test";

// В заглушке верный пароль — «password1», в хранилище пять выдуманных записей.

async function unlock(page: Page) {
  await page.goto("/");
  await page.getByLabel("Мастер-пароль").fill("password1");
  await page.getByRole("button", { name: "Разблокировать" }).click();
  await expect(page.getByRole("heading", { name: "web-01 (прод)" })).toBeVisible();
}

const sidebar = (page: Page) => page.getByRole("navigation", { name: "Записи" });

test("неверный пароль показывает паузу перед следующей попыткой", async ({ page }) => {
  await page.goto("/");
  await page.getByLabel("Мастер-пароль").fill("не тот");
  await page.getByRole("button", { name: "Разблокировать" }).click();
  await expect(page.getByRole("alert")).toContainText("Неверный мастер-пароль");
  await expect(page.getByRole("alert")).toContainText("Следующая попытка через");
  await expect(page.getByRole("button", { name: "Разблокировать" })).toBeDisabled();
});

test("секрет скрыт, пока его не показали", async ({ page }) => {
  await unlock(page);
  await expect(page.getByText("q7#Lm2vX9pTe")).toHaveCount(0);
  await page.getByRole("button", { name: "Показать password" }).click();
  await expect(page.getByText("q7#Lm2vX9pTe")).toBeVisible();
  await page.getByRole("button", { name: "Скрыть password" }).click();
  await expect(page.getByText("q7#Lm2vX9pTe")).toHaveCount(0);
});

test("копирование ssh сообщает о команде и очистке буфера", async ({ page }) => {
  await unlock(page);
  await page.getByRole("button", { name: "Скопировать ssh" }).click();
  await expect(page.getByRole("status")).toContainText("Скопирована команда ssh");
  await expect(page.getByRole("status")).toContainText("буфер очистится через 30 с");
});

test("поиск и фильтр по тегу сужают список", async ({ page }) => {
  await unlock(page);
  await page.getByRole("searchbox", { name: "Поиск" }).fill("почта");
  await expect(sidebar(page).getByRole("button", { name: /почта/ })).toBeVisible();
  await expect(sidebar(page).getByRole("button", { name: /web-01/ })).toHaveCount(0);
  await expect(page.getByText("1 из 5 записей")).toBeVisible();

  await page.getByRole("searchbox", { name: "Поиск" }).fill("");
  await page.getByRole("button", { name: "[dev] 1" }).click();
  await expect(page.getByText("1 из 5 записей")).toBeVisible();
});

test("раздел сворачивается и разворачивается", async ({ page }) => {
  await unlock(page);
  const server = sidebar(page).getByRole("button", { name: "server", exact: true });
  // Первый щелчок выбирает раздел, второй сворачивает, третий раскрывает.
  await server.click();
  await expect(server).toHaveAttribute("aria-expanded", "true");
  await server.click();
  await expect(server).toHaveAttribute("aria-expanded", "false");
  await expect(sidebar(page).getByRole("button", { name: /web-02/ })).toHaveCount(0);
  await server.click();
  await expect(sidebar(page).getByRole("button", { name: /web-02/ })).toBeVisible();
  // Стрелка у раздела сворачивает сразу.
  await server.locator(".chevron").click();
  await expect(server).toHaveAttribute("aria-expanded", "false");

  // Все разделы разом.
  await page.getByRole("button", { name: "Свернуть все" }).click();
  for (const name of ["work", "my"]) {
    await expect(sidebar(page).getByRole("button", { name, exact: true })).toHaveAttribute("aria-expanded", "false");
  }
  await expect(sidebar(page).getByRole("button", { name: /почта/ })).toHaveCount(0);
  await page.getByRole("button", { name: "Развернуть все" }).click();
  await expect(sidebar(page).getByRole("button", { name: /почта/ })).toBeVisible();
  await expect(server).toHaveAttribute("aria-expanded", "true");
});

test("щелчок по разделу показывает все его записи без подразделов", async ({ page }) => {
  await unlock(page);
  await sidebar(page).getByRole("button", { name: "server", exact: true }).click();
  await expect(page.getByRole("heading", { level: 1, name: "server" })).toBeVisible();
  await expect(page.getByText(/в разделе/)).toHaveCount(0);
  for (const title of ["web-01 (прод)", "web-02 (тест)", "БД каталога"]) {
    await expect(page.getByRole("region", { name: title })).toBeVisible();
  }
  await expect(page.getByRole("region", { name: "example.test" })).toHaveCount(0);

  // Курсор идёт по названиям записей раздела и их полям.
  await page.keyboard.press("Enter");
  await expect(page.locator(".title.cursor")).toHaveText("web-01 (прод)");
  await page.keyboard.press("ArrowDown");
  await expect(page.locator(".field.cursor")).toContainText("deploy@203.0.113.10");
  for (let i = 0; i < 3; i++) await page.keyboard.press("ArrowDown");
  await expect(page.locator(".field.cursor")).toContainText("deploy@198.51.100.24");
  await page.keyboard.press("Control+c");
  await expect(page.getByRole("status")).toContainText("Скопирована команда ssh");

  await page.getByRole("region", { name: "web-02 (тест)" }).getByRole("button", { name: "web-02 (тест)" }).click();
  await expect(page.getByRole("heading", { level: 1, name: "web-02 (тест)" })).toBeVisible();

  // В разделе только подразделы: по ним тоже ходят стрелки, Enter или → открывает.
  await sidebar(page).getByRole("button", { name: "work", exact: true }).click();
  await expect(page.getByText("Прямо в этом разделе записей нет, только подразделы")).toBeVisible();
  await page.keyboard.press("Enter");
  await expect(page.locator(".sub.cursor")).toContainText("server");
  await expect(page.locator(".sub.cursor")).toContainText("3 записи");
  await page.keyboard.press("ArrowDown");
  await expect(page.locator(".sub.cursor")).toContainText("site");
  await page.keyboard.press("Enter");
  await expect(page.getByRole("heading", { level: 1, name: "site" })).toBeVisible();
  await expect(page.locator(".title.cursor")).toHaveText("example.test");
  await page.keyboard.press("ArrowRight");
  await expect(page.getByRole("heading", { level: 1, name: "example.test" })).toBeVisible();
  await expect(page.locator(".field.cursor")).toContainText("https://example.test/");
});

test("стрелки ходят по списку и полям, Ctrl+C копирует поле под курсором", async ({ page }) => {
  await unlock(page);
  await page.keyboard.press("ArrowDown");
  await expect(page.getByRole("heading", { name: "web-02 (тест)" })).toBeVisible();

  await page.keyboard.press("ArrowRight");
  const cursor = page.locator(".field.cursor");
  await expect(cursor).toContainText("ssh");
  await page.keyboard.press("ArrowDown");
  await expect(cursor).toContainText("password");
  await page.keyboard.press(" ");
  await expect(page.getByText("Zk4!rW8nYb1s")).toBeVisible();
  await page.keyboard.press("Control+c");
  await expect(page.getByRole("status")).toContainText("Секрет скопирован");

  await page.keyboard.press("ArrowLeft");
  await expect(cursor).toHaveCount(0);
  await page.keyboard.press("ArrowLeft");
  const server = sidebar(page).getByRole("button", { name: "server", exact: true });
  await expect(server).toContainClass("on");
  await page.keyboard.press("ArrowLeft");
  await expect(server).toHaveAttribute("aria-expanded", "false");
  // → на разделе раскрывает его и переводит курсор в его список справа.
  await page.keyboard.press("ArrowRight");
  await expect(server).toHaveAttribute("aria-expanded", "true");
  await expect(page.locator(".title.cursor")).toHaveText("web-01 (прод)");
  await page.keyboard.press("ArrowDown");
  await expect(page.locator(".field.cursor")).toContainText("deploy@203.0.113.10");
  await page.keyboard.press("ArrowLeft");

  // С первой строки ↑ уходит в поиск, ↓ из поиска — обратно в список.
  // Первая строка — избранное.
  await page.keyboard.press("Home");
  await page.keyboard.press("ArrowUp");
  await expect(page.getByRole("searchbox", { name: "Поиск" })).toBeFocused();
  await page.keyboard.press("ArrowDown");
  await expect(page.getByRole("searchbox", { name: "Поиск" })).not.toBeFocused();
  const favorites = page.getByRole("navigation", { name: "Избранное" });
  await expect(favorites.getByRole("button", { name: /web-01/ })).toContainClass("on");
});

test("ширина списка слева меняется и запоминается", async ({ page }) => {
  await unlock(page);
  const aside = page.locator("aside");
  const resizer = page.getByRole("separator", { name: "Ширина списка записей" });
  expect((await aside.boundingBox())!.width).toBe(320);

  // Мышью: край на 100 пикселей правее.
  const box = (await resizer.boundingBox())!;
  await page.mouse.move(box.x + box.width / 2, box.y + 200);
  await page.mouse.down();
  await page.mouse.move(box.x + box.width / 2 + 100, box.y + 200, { steps: 6 });
  await page.mouse.up();
  expect((await aside.boundingBox())!.width).toBe(420);

  // Ширина сохраняется: после настроек панель такая же.
  await page.getByRole("button", { name: "Настройки" }).click();
  await page.getByRole("button", { name: "Назад" }).click();
  expect((await aside.boundingBox())!.width).toBe(420);

  // С клавиатуры: ← уже на 16, стрелки не уходят в список записей.
  await resizer.focus();
  await page.keyboard.press("ArrowLeft");
  await expect(resizer).toHaveAttribute("aria-valuenow", "404");
  await expect(page.getByRole("heading", { name: "web-01 (прод)" })).toBeVisible();
  // Уже самой узкой не бывает.
  await page.keyboard.press("Home");
  await expect(resizer).toHaveAttribute("aria-valuenow", "220");

  await resizer.dblclick();
  await expect(resizer).toHaveAttribute("aria-valuenow", "320");
});

test("стрелки доходят до «Свернуть все» между избранным и разделами", async ({ page }) => {
  await unlock(page);
  const work = sidebar(page).getByRole("button", { name: "work", exact: true });
  const folders = sidebar(page).getByRole("button", { name: /Свернуть все|Развернуть все/ });

  // Home — первая избранная запись, через две строки — кнопка.
  await page.keyboard.press("Home");
  await page.keyboard.press("ArrowDown");
  await page.keyboard.press("ArrowDown");
  await expect(folders).toContainClass("on");
  await expect(work).not.toContainClass("on");

  await page.keyboard.press("ArrowLeft");
  await expect(folders).toHaveText(/Развернуть все/);
  await expect(work).toHaveAttribute("aria-expanded", "false");
  await page.keyboard.press("ArrowRight");
  await expect(folders).toHaveText(/Свернуть все/);
  await expect(work).toHaveAttribute("aria-expanded", "true");
  await page.keyboard.press("Enter");
  await expect(folders).toHaveText(/Развернуть все/);
  await page.keyboard.press("Enter");
  await expect(folders).toHaveText(/Свернуть все/);

  // ↓ — к первому разделу, ↑ — обратно на кнопку и дальше в избранное.
  await page.keyboard.press("ArrowDown");
  await expect(work).toContainClass("on");
  await page.keyboard.press("ArrowUp");
  await expect(folders).toContainClass("on");
  await page.keyboard.press("ArrowUp");
  await expect(page.getByRole("navigation", { name: "Избранное" }).getByRole("button", { name: /почта/ })).toContainClass(
    "on",
  );
});

test("стрелки ходят по избранному и раскрывают длинный список", async ({ page }) => {
  await unlock(page);
  const favorites = page.getByRole("navigation", { name: "Избранное" });
  for (const title of [/web-02/, /БД каталога/, /example\.test/]) {
    await sidebar(page).getByRole("button", { name: title }).click();
    await page.getByRole("button", { name: "Добавить в избранное" }).click();
  }
  await page.getByRole("button", { name: "Новая запись" }).click();
  await page.getByLabel("Название").fill("шестая");
  await page.getByRole("button", { name: "В избранном" }).click();
  await page.getByRole("button", { name: "Сохранить запись" }).click();
  await expect(page.getByRole("status")).toContainText("Запись добавлена");

  // Home — на заголовок избранного, → раскрывает список, ← сворачивает.
  await page.keyboard.press("Home");
  const head = favorites.getByRole("button", { name: /Избранное/ });
  await expect(head).toContainClass("on");
  await page.keyboard.press("ArrowRight");
  await expect(head).toHaveAttribute("aria-expanded", "true");
  await page.keyboard.press("ArrowLeft");
  await expect(head).toHaveAttribute("aria-expanded", "false");
  await page.keyboard.press("Enter");
  await expect(head).toHaveAttribute("aria-expanded", "true");

  // ↓ идёт по избранному, курсор подсвечен только там, а не в разделе.
  await page.keyboard.press("ArrowDown");
  await expect(page.getByRole("heading", { name: "web-01 (прод)" })).toBeVisible();
  await expect(favorites.getByRole("button", { name: /web-01/ })).toContainClass("on");
  await expect(sidebar(page).getByRole("button", { name: /web-01/ })).not.toContainClass("on");
  for (let i = 0; i < 5; i++) await page.keyboard.press("ArrowDown");
  await expect(page.getByRole("heading", { name: "шестая" })).toBeVisible();
  await expect(favorites.getByRole("button", { name: /шестая/ })).toContainClass("on");

  // ← из избранного — к заголовку, следующая ↓ после избранного — в разделы.
  await page.keyboard.press("ArrowLeft");
  await expect(head).toContainClass("on");
  // End — на последнюю запись разделов: курсор уходит из избранного.
  await page.keyboard.press("End");
  await expect(sidebar(page).locator(".entry.on")).toHaveCount(1);
  await expect(favorites.locator(".on")).toHaveCount(0);

  // Свёрнутое избранное: после пятой записи ↓ уходит к «Свернуть все», шестая пропускается.
  await page.keyboard.press("Home");
  await page.keyboard.press("ArrowLeft");
  await expect(head).toHaveAttribute("aria-expanded", "false");
  for (let i = 0; i < 5; i++) await page.keyboard.press("ArrowDown");
  await expect(favorites.locator(".entry.on")).toHaveCount(1);
  await page.keyboard.press("ArrowDown");
  await expect(sidebar(page).getByRole("button", { name: /Свернуть все/ })).toContainClass("on");
  await expect(favorites.getByRole("button", { name: /шестая/ })).not.toContainClass("on");

  // Шестая, выбранная мышью: ↑ ведёт на пятую, ↓ — к «Свернуть все».
  await favorites.getByRole("button", { name: /шестая/ }).click();
  await page.keyboard.press("ArrowUp");
  await expect(page.getByRole("heading", { name: "почта" })).toBeVisible();
  await favorites.getByRole("button", { name: /шестая/ }).click();
  await page.keyboard.press("ArrowDown");
  await expect(sidebar(page).getByRole("button", { name: /Свернуть все/ })).toContainClass("on");
});

test("избранное стоит на месте, пока прокручивается список", async ({ page }) => {
  await page.setViewportSize({ width: 1280, height: 420 });
  await unlock(page);
  const favorites = page.getByRole("navigation", { name: "Избранное" });
  const top = (await favorites.boundingBox())!.y;
  await page.locator("aside .list").evaluate((el) => (el.scrollTop = el.scrollHeight));
  await expect(sidebar(page).getByRole("button", { name: "work", exact: true })).not.toBeInViewport();
  expect((await favorites.boundingBox())!.y).toBe(top);
  await expect(favorites).toBeInViewport();
});

test("«Свернуть все» висит над списком: строки не занимает и не уезжает", async ({ page }) => {
  await page.setViewportSize({ width: 1280, height: 420 });
  await unlock(page);
  const folders = page.getByRole("button", { name: "Свернуть все" });
  const list = page.locator("aside .list");
  // Первый раздел начинается у верха списка, как будто кнопки нет.
  const listTop = (await list.boundingBox())!.y;
  const work = sidebar(page).getByRole("button", { name: "work", exact: true });
  expect((await work.boundingBox())!.y - listTop).toBeLessThan(12);

  const y = (await folders.boundingBox())!.y;
  await list.evaluate((el) => (el.scrollTop = el.scrollHeight));
  await expect(work).not.toBeInViewport();
  expect((await folders.boundingBox())!.y).toBe(y);
  await expect(folders).toBeInViewport();
  await folders.click();
  await expect(page.getByRole("button", { name: "Развернуть все" })).toBeVisible();
});

test("правые края избранного и «Свернуть все» на одной линии", async ({ page }) => {
  await unlock(page);
  const hint = (await page.locator(".favorites .hint").first().boundingBox())!;
  const folders = page.locator(".folders");
  const box = (await folders.boundingBox())!;
  const padding = await folders.evaluate((el) => parseFloat(getComputedStyle(el).paddingRight));
  expect(Math.abs(hint.x + hint.width - (box.x + box.width - padding))).toBeLessThan(1);
});

test("новая запись подставляет выбранный раздел и сохраняется", async ({ page }) => {
  await unlock(page);
  await sidebar(page).getByRole("button", { name: "site", exact: true }).click();
  await page.getByRole("button", { name: "Новая запись" }).click();
  await expect(page.getByLabel("Раздел")).toHaveValue("work / site");

  await page.getByLabel("Название").fill("gamma");
  await page.getByLabel("Значение поля ssh").fill("deploy@192.0.2.1");
  await page.getByRole("button", { name: "Сгенерировать пароль" }).click();
  await page.getByRole("button", { name: "Сохранить запись" }).click();

  await expect(page.getByRole("status")).toContainText("Запись добавлена");
  await expect(page.getByRole("heading", { name: "gamma" })).toBeVisible();
  const crumb = page.getByRole("navigation", { name: "Путь" });
  await expect(crumb).toHaveText("work / site");
  // Щелчок по части пути открывает этот раздел.
  await crumb.getByRole("button", { name: "work" }).click();
  await expect(page.getByRole("heading", { name: "work" })).toBeVisible();
});

test("в редакторе под тегами подсказываются существующие", async ({ page }) => {
  await unlock(page);
  await page.getByRole("button", { name: "Новая запись" }).click();
  const tags = page.getByLabel("Теги");
  const hints = page.locator(".tag-hints button");
  await expect(hints).toHaveText(["prod", "dev", "work"]);

  // Набранный кусок сужает список, щелчок дописывает тег целиком.
  await tags.fill("pr");
  await expect(hints).toHaveText(["prod"]);
  await hints.first().click();
  await expect(tags).toHaveValue("prod, ");
  await expect(tags).toBeFocused();
  await expect(hints).toHaveText(["dev", "work"]);

  await hints.filter({ hasText: "work" }).click();
  await expect(tags).toHaveValue("prod, work, ");
  await expect(hints).toHaveText(["dev"]);
});

test("избранные записи показываются над разделами", async ({ page }) => {
  await unlock(page);
  const favorites = page.getByRole("navigation", { name: "Избранное" });
  await expect(favorites.getByRole("button")).toHaveText([/web-01/, /почта/]);

  await favorites.getByRole("button", { name: /почта/ }).click();
  await expect(page.getByRole("heading", { name: "почта" })).toBeVisible();

  // Звёздочка в карточке убирает запись из избранного и добавляет обратно.
  await page.getByRole("button", { name: "Убрать из избранного" }).click();
  await expect(favorites.getByRole("button")).toHaveText([/web-01/]);
  await page.getByRole("button", { name: "Добавить в избранное" }).click();
  await expect(favorites.getByRole("button")).toHaveText([/web-01/, /почта/]);

  // В виде Markdown видна строка признака.
  await page.getByRole("tab", { name: "Markdown" }).click();
  await expect(page.getByText("favorite:")).toBeVisible();

  // Во время поиска избранное спрятано: найденное и так в списке.
  await page.getByRole("searchbox", { name: "Поиск" }).fill("web");
  await expect(favorites).toHaveCount(0);
});

test("длинное избранное прокручивается: видно 5 строк, раскрытое — 10", async ({ page }) => {
  await unlock(page);
  const favorites = page.getByRole("navigation", { name: "Избранное" });
  for (const title of [/web-02/, /БД каталога/, /example\.test/]) {
    await sidebar(page).getByRole("button", { name: title }).click();
    await page.getByRole("button", { name: "Добавить в избранное" }).click();
  }
  // Пять избранных: стрелки у заголовка нет.
  await expect(favorites.getByRole("button", { name: /Избранное/ })).toHaveCount(0);

  for (let i = 1; i <= 7; i++) {
    await page.getByRole("button", { name: "Новая запись" }).click();
    await page.getByLabel("Название").fill(`новая-${i}`);
    await page.getByRole("button", { name: "В избранном" }).click();
    await page.getByRole("button", { name: "Сохранить запись" }).click();
    await expect(page.getByRole("status")).toContainText("Запись добавлена");
  }

  // Двенадцать избранных, все в списке; в заголовке нет счётчика.
  const head = favorites.getByRole("button", { name: /Избранное/ });
  await expect(head).toHaveText("Избранное");
  await expect(head).toHaveAttribute("aria-expanded", "false");
  const box = favorites.locator(".fav-scroll");
  await expect(box.getByRole("button")).toHaveCount(12);
  expect((await box.boundingBox())!.height).toBe(150);

  // Дальние записи — через прокрутку области избранного.
  const last = box.getByRole("button", { name: /новая-7/ });
  await expect(last).not.toBeInViewport();
  await last.scrollIntoViewIfNeeded();
  await expect(last).toBeInViewport();

  await head.click();
  await expect(head).toHaveAttribute("aria-expanded", "true");
  expect((await box.boundingBox())!.height).toBe(300);
  await head.click();
  expect((await box.boundingBox())!.height).toBe(150);
});

test("избранное переставляется перетаскиванием и Alt+стрелками", async ({ page }) => {
  await unlock(page);
  const favorites = page.getByRole("navigation", { name: "Избранное" });
  const rows = favorites.getByRole("button");
  await expect(rows).toHaveText([/web-01/, /почта/]);

  // «почта» — на верхнюю половину «web-01».
  const from = (await favorites.getByRole("button", { name: /почта/ }).boundingBox())!;
  const to = (await favorites.getByRole("button", { name: /web-01/ }).boundingBox())!;
  await page.mouse.move(from.x + from.width / 2, from.y + from.height / 2);
  await page.mouse.down();
  await page.mouse.move(to.x + to.width / 2, to.y + 4, { steps: 8 });
  await page.mouse.up();
  await expect(page.getByRole("status")).toContainText("Порядок избранного изменён");
  await expect(rows).toHaveText([/почта/, /web-01/]);

  // Номер места виден в файле.
  await favorites.getByRole("button", { name: /почта/ }).click();
  await page.getByRole("tab", { name: "Markdown" }).click();
  await expect(page.locator("main").getByText("favorite:")).toBeVisible();
  await expect(page.locator("main")).toContainText("favorite: 1");

  // Alt+↓ возвращает её вниз, курсор остаётся на ней.
  await page.keyboard.press("Alt+ArrowDown");
  await expect(rows).toHaveText([/web-01/, /почта/]);
  await expect(favorites.getByRole("button", { name: /почта/ })).toContainClass("on");
  await expect(page.locator("main")).toContainText("favorite: 2");
});

test("в редакторе запись добавляется в избранное", async ({ page }) => {
  await unlock(page);
  await sidebar(page).getByRole("button", { name: /БД каталога/ }).click();
  await page.getByRole("button", { name: "Редактировать" }).click();
  const star = page.getByRole("button", { name: "В избранном" });
  await expect(star).toHaveAttribute("aria-pressed", "false");
  await star.click();
  await page.getByRole("button", { name: "Сохранить", exact: true }).click();
  await expect(page.getByRole("status")).toContainText("Изменения сохранены");
  await expect(page.getByRole("navigation", { name: "Избранное" }).getByRole("button")).toHaveText([
    /web-01/,
    /БД каталога/,
    /почта/,
  ]);
});

test("много тегов не вытесняют список записей", async ({ page }) => {
  await unlock(page);
  await page.getByRole("button", { name: "Новая запись" }).click();
  await page.getByLabel("Название").fill("много тегов");
  await page.getByLabel("Теги").fill(Array.from({ length: 15 }, (_, i) => `tag-${i + 1}`).join(", "));
  await page.getByRole("button", { name: "Сохранить запись" }).click();
  await expect(page.getByRole("status")).toContainText("Запись добавлена");

  const tags = page.locator("aside .tags");
  const more = page.getByRole("button", { name: /^Все теги \(\d+\)$/ });
  await expect(more).toBeVisible();
  expect((await tags.boundingBox())!.height).toBeLessThanOrEqual(62);
  await expect(sidebar(page).getByRole("button", { name: /web-01/ })).toBeInViewport();

  await more.click();
  expect((await tags.boundingBox())!.height).toBeGreaterThan(62);
  await page.getByRole("button", { name: "Свернуть теги" }).click();
  await expect(more).toBeVisible();
});

test("в редакторе порядок полей меняется перетаскиванием и Alt+стрелками", async ({ page }) => {
  await unlock(page);
  await sidebar(page).getByRole("button", { name: /БД каталога/ }).click();
  await page.getByRole("button", { name: "Редактировать" }).click();
  const keys = page.getByLabel("Ключ поля");
  await expect(keys).toHaveCount(3); // note редактируется отдельно
  await expect(keys.nth(0)).toHaveValue("port");

  // Alt+↓ в поле ввода переносит поле ниже, фокус остаётся в нём.
  await keys.nth(0).focus();
  await page.keyboard.press("Alt+ArrowDown");
  await expect(keys.nth(0)).toHaveValue("user");
  await expect(keys.nth(1)).toHaveValue("port");
  await expect(keys.nth(1)).toBeFocused();

  // Перетаскивание за ручку: password — наверх.
  const grip = page.getByRole("button", { name: "Переместить поле password" });
  const top = (await page.getByRole("button", { name: "Переместить поле user" }).boundingBox())!;
  const box = (await grip.boundingBox())!;
  await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
  await page.mouse.down();
  await page.mouse.move(top.x + top.width / 2, top.y + top.height / 2 - 4, { steps: 8 });
  await page.mouse.up();
  await expect(keys.nth(0)).toHaveValue("password");
  await expect(keys.nth(1)).toHaveValue("user");
  await expect(keys.nth(2)).toHaveValue("port");

  await page.getByRole("button", { name: "Сохранить", exact: true }).click();
  await expect(page.getByRole("status")).toContainText("Изменения сохранены");
  await expect(page.locator("main .key")).toHaveText(["password", "user", "port"]);
});

test("выход из редактора с правками спрашивает подтверждение", async ({ page }) => {
  await unlock(page);
  await page.getByRole("button", { name: "Редактировать" }).click();
  await page.getByLabel("Название").fill("web-01 (новая)");
  await page.keyboard.press("Escape");
  const dialog = page.getByRole("dialog", { name: "Сохранить изменения?" });
  await expect(dialog).toBeVisible();
  await dialog.getByRole("button", { name: "Сохранить" }).click();
  await expect(page.getByRole("heading", { name: "web-01 (новая)" })).toBeVisible();
});

test("удаление записи спрашивает подтверждение", async ({ page }) => {
  await unlock(page);
  await page.getByRole("button", { name: "Удалить запись" }).click();
  const dialog = page.getByRole("dialog");
  await expect(dialog).toContainText("Удалить «web-01 (прод)»?");
  await dialog.getByRole("button", { name: "Удалить" }).click();
  await expect(page.getByRole("status")).toContainText("удалена");
  await expect(sidebar(page).getByRole("button", { name: /web-01/ })).toHaveCount(0);
});

test("запись перетаскивается в другой раздел", async ({ page }) => {
  await unlock(page);
  const entry = sidebar(page).getByRole("button", { name: /почта/ });
  const target = sidebar(page).getByRole("button", { name: "site", exact: true });
  await entry.dragTo(target);
  await expect(page.getByRole("status")).toContainText("Перемещено");
  await expect(sidebar(page).getByRole("button", { name: "my", exact: true })).toHaveCount(0);
});

test("запись и раздел, перенесённые внутрь раздела, не раздваивают его", async ({ page }) => {
  await unlock(page);
  const work = sidebar(page).getByRole("button", { name: "work", exact: true });
  const box = (await work.boundingBox())!;
  // Запись в work, где своих записей нет, только подразделы.
  await sidebar(page)
    .getByRole("button", { name: /example\.test/ })
    .dragTo(work, { targetPosition: { x: box.width / 2, y: box.height - 4 } });
  await expect(page.getByRole("status")).toContainText("Перемещено");
  await expect(work).toHaveCount(1);

  // В разделе сначала его записи, под ними подразделы. Стрелки идут в том же порядке.
  await work.click();
  const card = page.getByRole("region", { name: "example.test" });
  const subs = page.getByRole("navigation", { name: "Подразделы" });
  expect((await card.boundingBox())!.y).toBeLessThan((await subs.boundingBox())!.y);
  await page.keyboard.press("ArrowRight");
  await expect(card.getByRole("button", { name: "example.test" })).toHaveClass(/cursor/);
  await work.click();

  // Раздел my внутрь work: встаёт последним подразделом, work по-прежнему один.
  const box2 = (await work.boundingBox())!;
  await sidebar(page)
    .getByRole("button", { name: "my", exact: true })
    .dragTo(work, { targetPosition: { x: box2.width / 2, y: box2.height - 4 } });
  await expect(page.getByRole("status").last()).toContainText("Перемещено");
  await expect(work).toHaveCount(1);
  await expect(sidebar(page).getByRole("button", { name: "my", exact: true })).toHaveCount(1);
});

test("запись над разделом верхнего уровня уходит на верхний уровень", async ({ page }) => {
  await unlock(page);
  const entry = sidebar(page).getByRole("button", { name: /почта/ });
  const work = sidebar(page).getByRole("button", { name: "work", exact: true });
  const box = (await work.boundingBox())!;
  await entry.hover();
  await page.mouse.down();
  await page.mouse.move(box.x + box.width / 2, box.y + 10, { steps: 5 });
  await page.mouse.move(box.x + box.width / 2, box.y + 3, { steps: 2 });
  await page.mouse.up();
  await expect(page.getByRole("status")).toContainText("Перемещено");
  // Раздел my опустел и исчез, запись стоит над разделами.
  await expect(sidebar(page).getByRole("button", { name: "my", exact: true })).toHaveCount(0);
  expect((await entry.boundingBox())!.y).toBeLessThan((await work.boundingBox())!.y);
  await expect(page.getByText("Перетащите сюда")).toHaveCount(0);
});

test("пальцем запись перетаскивается после долгого нажатия, а простое движение — прокрутка", async ({ page }) => {
  await unlock(page);
  const entry = sidebar(page).getByRole("button", { name: /почта/ });
  const target = sidebar(page).getByRole("button", { name: "site", exact: true });
  const from = (await entry.boundingBox())!;
  const to = (await target.boundingBox())!;
  const at = (box: { x: number; y: number; width: number; height: number }) => ({
    pointerType: "touch",
    isPrimary: true,
    button: 0,
    bubbles: true,
    clientX: box.x + box.width / 2,
    clientY: box.y + box.height / 2,
  });
  const hint = page.locator("aside.dragging");

  // Сразу повели пальцем — перетаскивание не начинается.
  await entry.dispatchEvent("pointerdown", at(from));
  await entry.dispatchEvent("pointermove", at(to));
  await expect(hint).toHaveCount(0);
  await entry.dispatchEvent("pointerup", at(to));

  // Подержали — тянется.
  await entry.dispatchEvent("pointerdown", at(from));
  await page.waitForTimeout(500);
  await expect(hint).toBeVisible();
  await target.dispatchEvent("pointermove", at(to));
  await target.dispatchEvent("pointerup", at(to));
  await expect(page.getByRole("status")).toContainText("Перемещено");
});

test("горячие клавиши: Ctrl+N открывает новую запись, Esc возвращает", async ({ page }) => {
  await unlock(page);
  await page.keyboard.press("Control+n");
  await expect(page.getByRole("heading", { name: "Новая запись" })).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(page.getByRole("heading", { name: "web-01 (прод)" })).toBeVisible();
});

test("горячие клавиши: Ctrl+Shift+E сворачивает и разворачивает все разделы", async ({ page }) => {
  await unlock(page);
  const work = sidebar(page).getByRole("button", { name: "work", exact: true });
  await page.keyboard.press("Control+Shift+e");
  await expect(work).toHaveAttribute("aria-expanded", "false");
  await expect(page.getByRole("button", { name: "Развернуть все" })).toBeVisible();
  await page.keyboard.press("Control+Shift+e");
  await expect(work).toHaveAttribute("aria-expanded", "true");

  // Работает и из поля поиска, но пока идёт поиск — ничего не делает, как и кнопка.
  const search = page.getByRole("searchbox", { name: "Поиск" });
  await search.fill("web");
  await page.keyboard.press("Control+Shift+e");
  await expect(work).toHaveAttribute("aria-expanded", "true");
  await search.fill("");
  await search.focus();
  await page.keyboard.press("Control+Shift+e");
  await expect(work).toHaveAttribute("aria-expanded", "false");
});

test("хранилище без шифрования создаётся без пароля", async ({ page }) => {
  await page.goto("/");
  await page.getByRole("button", { name: "Создать новое хранилище" }).click();
  await page.getByRole("radio", { name: /Без шифрования/ }).click();
  await expect(page.getByLabel("Мастер-пароль")).toHaveCount(0);
  await expect(page.getByText("~/Documents/vault.md", { exact: true })).toBeVisible();
  await page.getByRole("button", { name: "Создать хранилище" }).click();
  await expect(page.getByRole("banner")).toContainText("без шифрования");
});

test("слабому шифрованию хватает пароля из 4 символов", async ({ page }) => {
  await page.goto("/");
  await page.getByRole("button", { name: "Создать новое хранилище" }).click();
  await page.getByRole("radio", { name: /Слабое/ }).click();
  await page.getByLabel("Мастер-пароль").fill("abcd");
  await page.getByLabel("Повторите пароль").fill("abcd");
  await expect(page.getByRole("button", { name: "Создать хранилище" })).toBeEnabled();

  await page.getByRole("radio", { name: /Надёжное/ }).click();
  await expect(page.getByRole("button", { name: "Создать хранилище" })).toBeDisabled();
});

test("в настройках есть все горячие клавиши", async ({ page }) => {
  await unlock(page);
  await page.getByRole("button", { name: "Настройки" }).click();
  await page.getByRole("button", { name: "Горячие клавиши" }).click();
  await expect(page.getByRole("heading", { level: 1, name: "Горячие клавиши" })).toBeVisible();
  for (const group of ["Главное окно", "Список записей слева", "Запись и список раздела справа", "Редактор записи"]) {
    await expect(page.getByRole("region", { name: group })).toBeVisible();
  }
  const copy = page.getByRole("region", { name: "Запись и список раздела справа" }).locator(".key-row", { hasText: "Скопировать поле" });
  await expect(copy.locator("kbd")).toHaveText(["Ctrl", "C", "Enter"]);
  const favorites = page.getByRole("region", { name: "Список записей слева" }).locator(".key-row", { hasText: "На заголовке «Избранное»" });
  await expect(favorites.locator("kbd")).toHaveText(["→", "←", "Enter"]);
});

test("настройки: защита файла и резервные копии", async ({ page }) => {
  await unlock(page);
  await page.getByRole("button", { name: "Настройки" }).click();
  await page.getByRole("button", { name: "Изменить…" }).click();
  const dialog = page.getByRole("dialog", { name: "Защита файла" });
  await expect(dialog.getByLabel("Текущий мастер-пароль")).toBeVisible();
  await dialog.getByRole("button", { name: "Отмена" }).click();

  await page.getByRole("button", { name: "Файл и копии" }).click();
  await expect(page.getByText("backups")).toBeVisible();
  await page.getByRole("button", { name: "Открыть резервные копии" }).click();
  await expect(page.getByRole("heading", { name: "Резервные копии" })).toBeVisible();
  await expect(page.getByText("Что изменится при восстановлении")).toBeVisible();
});

test("в резервной копии видно прежний пароль, запись возвращается отдельно", async ({ page }) => {
  await unlock(page);
  await page.getByRole("button", { name: "Настройки" }).click();
  await page.getByRole("button", { name: "Файл и копии" }).click();
  await page.getByRole("button", { name: "Открыть резервные копии" }).click();

  const web01 = page.locator(".change", { hasText: "web-01 (прод)" });
  const password = web01.getByLabel("Прежнее значение password");
  await expect(password).toContainText("••••••••");
  await expect(page.getByText("Old#Pass-2025")).toHaveCount(0);
  await web01.getByRole("button", { name: "Показать прежний password" }).click();
  await expect(password).toContainText("Old#Pass-2025");

  // Обычное поле показывается сразу.
  const db = page.locator(".change", { hasText: "БД каталога" });
  await expect(db.getByLabel("Прежнее значение note")).toContainText("таблицу catalog_tmp пока не трогать");

  await web01.getByRole("button", { name: "Скопировать прежний password" }).click();
  await expect(page.getByRole("status")).toContainText("Скопировано прежнее значение password");

  // «Вернуть запись» меняет только её: изменение «БД каталога» остаётся.
  await web01.getByRole("button", { name: "Вернуть запись" }).click();
  await expect(page.getByRole("status")).toContainText("Запись «web-01 (прод)» восстановлена из копии");
  await expect(web01).toHaveCount(0);
  await expect(db).toBeVisible();

  await page.getByRole("button", { name: "Настройки" }).click();
  await page.getByRole("button", { name: "Назад" }).click();
  await sidebar(page).getByRole("button", { name: /web-01/ }).click();
  await page.getByRole("button", { name: "Показать password" }).click();
  await expect(page.getByText("Old#Pass-2025")).toBeVisible();
});

test("из копии возвращается удалённая запись", async ({ page }) => {
  await unlock(page);
  await page.getByRole("button", { name: "Настройки" }).click();
  await page.getByRole("button", { name: "Файл и копии" }).click();
  await page.getByRole("button", { name: "Открыть резервные копии" }).click();
  await page.getByRole("button", { name: /Перед удалением записи/ }).click();

  const router = page.locator(".change", { hasText: "роутер" });
  await expect(router).toContainText("вернётся");
  await router.getByRole("button", { name: "Вернуть запись" }).click();
  await expect(page.getByRole("status")).toContainText("Запись «роутер» восстановлена из копии");
  await expect(router).toHaveCount(0);
});

// Найдено проверкой: показанный секрет был привязан к номеру поля и переезжал на другое поле.
test("после правки записи показанный пароль снова скрыт", async ({ page }) => {
  await unlock(page);
  await sidebar(page).getByRole("button", { name: /БД каталога/ }).click();
  await page.getByRole("button", { name: "Показать password" }).click();
  await expect(page.getByText("hV3$eQ6mJ0cA")).toBeVisible();

  await page.getByRole("button", { name: "Редактировать" }).click();
  const keys = page.getByLabel("Ключ поля");
  await keys.nth(2).focus();
  await page.keyboard.press("Alt+ArrowUp");
  await page.keyboard.press("Alt+ArrowUp");
  await expect(keys.nth(0)).toHaveValue("password");
  await page.getByRole("button", { name: "Сохранить", exact: true }).click();
  await expect(page.getByRole("status")).toContainText("Изменения сохранены");
  await expect(page.locator("main .key")).toHaveText(["password", "port", "user"]);
  await expect(page.getByText("hV3$eQ6mJ0cA")).toHaveCount(0);
});

// Найдено проверкой: после «Перечитать» записи нумеруются заново, и секрет и выбор
// переезжали на другую запись.
test("после перечитывания файла выбрана та же запись, секрет скрыт", async ({ page }) => {
  await unlock(page);
  await sidebar(page).getByRole("button", { name: /web-02/ }).click();
  await page.getByRole("button", { name: "Удалить запись" }).click();
  await page.getByRole("dialog").getByRole("button", { name: "Удалить" }).click();
  await expect(page.getByRole("status")).toContainText("удалена");

  await sidebar(page).getByRole("button", { name: /почта/ }).click();
  await page.getByRole("button", { name: "Показать password" }).click();
  await expect(page.getByText("fR9^kL3zVm6q")).toBeVisible();

  // Запись «конфликт» в заглушке изображает файл, изменённый вне приложения.
  await page.getByRole("button", { name: "Новая запись" }).click();
  await page.getByLabel("Название").fill("конфликт");
  await page.getByRole("button", { name: "Сохранить запись" }).click();
  await page.getByRole("dialog", { name: "Файл изменён вне приложения" }).getByRole("button", { name: "Перечитать файл" }).click();
  await expect(page.getByRole("status")).toContainText("Файл перечитан");
  await page.keyboard.press("Escape");
  await page.getByRole("button", { name: "Не сохранять" }).click();

  await expect(page.getByRole("heading", { name: "почта" })).toBeVisible();
  await expect(page.getByText("fR9^kL3zVm6q")).toHaveCount(0);
});

test("Enter на кнопке, до которой дошли Tab'ом, нажимает её", async ({ page }) => {
  await unlock(page);
  await page.getByRole("button", { name: "Новая запись" }).focus();
  await page.keyboard.press("Enter");
  await expect(page.getByRole("heading", { name: "Новая запись" })).toBeVisible();
});

test("многострочный ключ правится целиком, показ пароля — не правка", async ({ page }) => {
  await page.goto("/");
  await page.getByRole("button", { name: /work\.md\.age/ }).click();
  await page.getByLabel("Мастер-пароль").fill("password1");
  await page.getByRole("button", { name: "Разблокировать" }).click();
  await sidebar(page).getByRole("button", { name: /deploy-ключ/ }).click();
  await page.getByRole("button", { name: "Редактировать" }).click();

  await page.getByRole("button", { name: "Показать значение" }).click();
  const value = page.getByLabel("Значение поля private_key");
  await expect(value).toHaveValue("-----BEGIN KEY-----\nAAAAexample\nBBBBexample\n-----END KEY-----");
  expect(await value.evaluate((el) => el.tagName)).toBe("TEXTAREA");

  // Только показали: выход без вопроса «Сохранить изменения?».
  await page.keyboard.press("Escape");
  await expect(page.getByRole("heading", { name: "deploy-ключ" })).toBeVisible();
});

test("быстрые нажатия на краю списка не теряются", async ({ page }) => {
  await unlock(page);
  const resizer = page.getByRole("separator", { name: "Ширина списка записей" });
  await resizer.focus();
  for (let i = 0; i < 3; i++) await page.keyboard.press("ArrowLeft");
  await expect(resizer).toHaveAttribute("aria-valuenow", "272");
  await page.waitForTimeout(600);
  await expect(resizer).toHaveAttribute("aria-valuenow", "272");
  await page.getByRole("button", { name: "Настройки" }).click();
  await page.getByRole("button", { name: "Назад" }).click();
  expect((await page.locator("aside").boundingBox())!.width).toBe(272);
});

test("при конфликте выбирают версию записи", async ({ page }) => {
  await unlock(page);
  // Запись «две версии» в заглушке изображает слияние с конфликтом.
  await page.getByRole("button", { name: "Новая запись" }).click();
  await page.getByLabel("Название").fill("две версии");
  await page.getByRole("button", { name: "Сохранить запись" }).click();
  const dialog = page.getByRole("dialog", { name: "Запись изменили на двух устройствах" });
  await expect(dialog).toBeVisible();
  await expect(dialog.getByRole("columnheader", { name: "Ноутбук" })).toBeVisible();
  await expect(dialog.getByText("Lk7#vQ2pZx")).toHaveCount(0);
  await dialog.getByRole("button", { name: "Показать пароли" }).click();
  await expect(dialog.getByText("Lk7#vQ2pZx")).toBeVisible();
  await dialog.getByRole("button", { name: "Мою версию" }).click();
  await expect(dialog).toBeHidden();
  await expect(sidebar(page).getByRole("button", { name: /копия с этого устройства/ })).toHaveCount(0);
});

test("при конфликте можно оставить обе версии", async ({ page }) => {
  await unlock(page);
  await page.getByRole("button", { name: "Новая запись" }).click();
  await page.getByLabel("Название").fill("две версии");
  await page.getByRole("button", { name: "Сохранить запись" }).click();
  const dialog = page.getByRole("dialog", { name: "Запись изменили на двух устройствах" });
  await dialog.getByRole("button", { name: "Оставить обе" }).click();
  await expect(dialog).toBeHidden();
  await expect(sidebar(page).getByRole("button", { name: /две версии \(копия с этого устройства\)/ })).toBeVisible();
});

test("файл, изменённый вне приложения, предлагают перечитать", async ({ page }) => {
  await unlock(page);
  await page.getByRole("button", { name: "Новая запись" }).click();
  await page.getByLabel("Название").fill("конфликт");
  await page.getByRole("button", { name: "Сохранить запись" }).click();
  const dialog = page.getByRole("dialog", { name: "Файл изменён вне приложения" });
  await expect(dialog).toBeVisible();
  await dialog.getByRole("button", { name: "Перечитать файл" }).click();
  await expect(page.getByRole("status")).toContainText("Файл перечитан");
});

test("Яндекс Диск подключается токеном, и хранилище начинает синхронизироваться", async ({ page }) => {
  await unlock(page);
  await page.getByRole("button", { name: "Настройки" }).click();
  await page.getByRole("button", { name: "Облако" }).click();
  await page.getByLabel("OAuth-токен").fill("y0_test-token");
  await page.getByRole("button", { name: "Подключить", exact: true }).click();
  await expect(page.getByRole("status")).toContainText("Яндекс Диск подключён");

  await page.getByRole("button", { name: "Синхронизировать", exact: true }).click();
  await expect(page.getByText("Синхронизировано в 12:30")).toBeVisible();
  await page.getByRole("button", { name: "Отключить" }).click();
  await expect(page.getByRole("button", { name: "Синхронизировать", exact: true })).toBeVisible();
  await page.getByRole("button", { name: "Выйти" }).click();
  await expect(page.getByLabel("OAuth-токен")).toBeVisible();
});

test("на узком экране текст и поля диалога во всю ширину, вровень с кнопками", async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await page.goto("/");
  await page.getByRole("button", { name: "Создать новое хранилище" }).click();
  await page.getByRole("button", { name: "Открыть из облака" }).click();
  const connect = page.getByRole("dialog", { name: "Подключить облако" });
  const field = (await connect.getByLabel("OAuth-токен").boundingBox())!;
  const cancel = (await connect.getByRole("button", { name: "Отмена" }).boundingBox())!;
  expect(field.x).toBe(cancel.x);
  expect(field.width).toBe(cancel.width);
});

test("на новом компьютере хранилище открывается из облака", async ({ page }) => {
  await page.goto("/");
  await page.getByRole("button", { name: "Создать новое хранилище" }).click();
  await page.getByRole("button", { name: "Открыть из облака" }).click();

  const connect = page.getByRole("dialog", { name: "Подключить облако" });
  await connect.getByLabel("OAuth-токен").fill("y0_test-token");
  await connect.getByRole("button", { name: "Подключить", exact: true }).click();

  const pick = page.getByRole("dialog", { name: "Открыть из облака" });
  await pick.getByRole("button", { name: "vault.md.age" }).click();
  await expect(page.getByLabel("Мастер-пароль")).toBeVisible();
  await expect(page.getByRole("button", { name: /new\.md\.age/ })).toBeVisible();
  await expect(page.getByText("Файл не найден")).toHaveCount(0);
});

test("Google Drive выбирается переключателем и подключается своим токеном", async ({ page }) => {
  await unlock(page);
  await page.getByRole("button", { name: "Настройки" }).click();
  await page.getByRole("button", { name: "Облако" }).click();
  await page.getByRole("button", { name: "Google Drive" }).click();
  await page.getByLabel("OAuth-токен").fill("ya29.test-token");
  await page.getByRole("button", { name: "Подключить", exact: true }).click();
  await expect(page.getByRole("status")).toContainText("Google Drive подключён");
});

test("на узком экране запись открывается отдельно и возвращается к списку", async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  // Общий unlock ждёт карточку записи, а на узком экране сначала список.
  await page.goto("/");
  await page.getByLabel("Мастер-пароль").fill("password1");
  await page.getByRole("button", { name: "Разблокировать" }).click();
  await expect(page.getByRole("button", { name: "Новая запись" })).toBeVisible();
  // Шапка списка: логотип, замок и шестерёнка; счётчика записей и нижней строки нет.
  await expect(page.getByText("mdpass", { exact: true })).toBeVisible();
  await expect(page.getByText(/\d+ запис/)).toHaveCount(0);
  await expect(page.getByRole("button", { name: "Заблокировать" })).toBeVisible();
  await expect(page.getByRole("button", { name: "Настройки" })).toBeVisible();
  await expect(page.getByRole("contentinfo")).toHaveCount(0);

  await sidebar(page).getByRole("button", { name: /почта/ }).click();
  await expect(page.getByRole("heading", { name: "почта" })).toBeVisible();
  await expect(page.getByRole("button", { name: "Новая запись" })).toHaveCount(0);

  // Раздел в списке открывается отдельным экраном, как справа на компьютере.
  await page.getByRole("button", { name: "Назад к записям" }).click();
  await sidebar(page).getByRole("button", { name: "site", exact: true }).click();
  await expect(page.getByRole("heading", { name: "site" })).toBeVisible();
  await expect(page.getByRole("button", { name: /example\.test/ })).toBeVisible();
  await page.getByRole("button", { name: "Назад к записям" }).click();
  await sidebar(page).getByRole("button", { name: /почта/ }).click();

  // Путь над записью ведёт в раздел и на телефоне, из раздела можно вернуться в запись.
  await page.getByRole("navigation", { name: "Путь" }).getByRole("button", { name: "my" }).click();
  await expect(page.getByRole("heading", { name: "my" })).toBeVisible();
  await page.getByRole("button", { name: "почта", exact: true }).click();
  await expect(page.getByRole("heading", { name: "почта" })).toBeVisible();

  await page.getByRole("button", { name: "Назад к записям" }).click();
  await expect(page.getByRole("button", { name: "Новая запись" })).toBeVisible();
  await expect(page.getByRole("heading", { name: "почта" })).toHaveCount(0);

  // Настройки на телефоне — списком разделов, «Назад» из раздела ведёт к списку.
  await page.getByRole("button", { name: "Настройки" }).click();
  await expect(page.getByRole("button", { name: "Горячие клавиши" })).toHaveCount(0);
  await page.getByRole("button", { name: "Безопасность" }).click();
  await expect(page.getByRole("heading", { name: "Безопасность" })).toBeVisible();
  await page.getByRole("button", { name: "Назад" }).click();
  await expect(page.getByRole("button", { name: "Облако" })).toBeVisible();
  await page.getByRole("button", { name: "Назад" }).click();

  await page.getByRole("button", { name: "Новая запись" }).click();
  await expect(page.getByRole("heading", { name: "Новая запись" })).toBeVisible();
});

test("на узком экране системная кнопка «Назад» возвращает, а не закрывает приложение", async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await page.goto("/");
  await page.getByLabel("Мастер-пароль").fill("password1");
  await page.getByRole("button", { name: "Разблокировать" }).click();
  await expect(page.getByRole("button", { name: "Новая запись" })).toBeVisible();
  // На списке возвращаться некуда: лишней записи в истории нет, «Назад» закроет приложение.
  await expect.poll(() => page.evaluate(() => history.state)).toBeNull();

  // Запись → раздел по пути → «Назад» → список.
  await sidebar(page).getByRole("button", { name: /почта/ }).click();
  await expect(page.getByRole("heading", { name: "почта" })).toBeVisible();
  await page.goBack();
  await expect(page.getByRole("button", { name: "Новая запись" })).toBeVisible();
  await expect.poll(() => page.evaluate(() => history.state)).toBeNull();

  // Настройки: из раздела к списку разделов, потом к записям.
  await page.getByRole("button", { name: "Настройки" }).click();
  await page.getByRole("button", { name: "Безопасность" }).click();
  await expect(page.getByRole("heading", { name: "Безопасность" })).toBeVisible();
  await page.goBack();
  await expect(page.getByRole("button", { name: "Облако" })).toBeVisible();
  await page.goBack();
  await expect(page.getByRole("button", { name: "Новая запись" })).toBeVisible();

  // В редакторе с правками «Назад» спрашивает, а второй «Назад» закрывает вопрос.
  await page.getByRole("button", { name: "Новая запись" }).click();
  await page.getByLabel("Название").fill("черновик");
  await page.goBack();
  const ask = page.getByRole("dialog");
  await expect(ask).toBeVisible();
  await page.goBack();
  await expect(ask).toHaveCount(0);
  await expect(page.getByRole("heading", { name: "Новая запись" })).toBeVisible();
});

test("на узком экране избранное сворачивается до заголовка", async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await page.goto("/");
  await page.getByLabel("Мастер-пароль").fill("password1");
  await page.getByRole("button", { name: "Разблокировать" }).click();
  const favorites = page.getByRole("navigation", { name: "Избранное" });
  const head = favorites.getByRole("button", { name: "Избранное" });
  await expect(head).toHaveAttribute("aria-expanded", "true");
  await expect(favorites.getByRole("button", { name: /почта/ })).toBeVisible();

  await head.click();
  await expect(head).toHaveAttribute("aria-expanded", "false");
  await expect(favorites.getByRole("button", { name: /почта/ })).toHaveCount(0);
  // Сами записи остаются в разделах.
  await expect(sidebar(page).getByRole("button", { name: /почта/ })).toBeVisible();

  await head.click();
  await expect(favorites.getByRole("button", { name: /почта/ })).toBeVisible();
});

test("на узком экране видна одна строка тегов, остальные — по «Все теги»", async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await page.goto("/");
  await page.getByLabel("Мастер-пароль").fill("password1");
  await page.getByRole("button", { name: "Разблокировать" }).click();
  await page.getByRole("button", { name: "Новая запись" }).click();
  await page.getByLabel("Название").fill("много тегов");
  await page.getByLabel("Теги").fill(Array.from({ length: 15 }, (_, i) => `tag-${i + 1}`).join(", "));
  await page.getByRole("button", { name: "Сохранить запись" }).click();
  await expect(page.getByRole("status")).toContainText("Запись добавлена");
  if (await page.getByRole("button", { name: "Назад к записям" }).isVisible())
    await page.getByRole("button", { name: "Назад к записям" }).click();

  const tags = page.locator("aside .tags");
  const chip = tags.locator(".chip").first();
  // Высота области — одна строка чипов.
  await expect.poll(async () => (await tags.boundingBox())!.height).toBe((await chip.boundingBox())!.height);
  const more = page.getByRole("button", { name: /^Все теги \(\d+\)$/ });
  await more.click();
  await expect.poll(async () => (await tags.boundingBox())!.height).toBeGreaterThan(60);
  await page.getByRole("button", { name: "Свернуть теги" }).click();
  await expect(more).toBeVisible();
});

test("на низком экране форма создания хранилища прокручивается до самого верха", async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 600 });
  await page.goto("/");
  await page.getByRole("button", { name: "Создать новое хранилище" }).click();
  const create = page.getByRole("button", { name: "Создать хранилище" });
  await expect(create).toBeVisible();
  // Прокрутка в самый верх показывает начало формы, а не обрезанный текст.
  const screen = page.locator(".screen");
  await screen.evaluate((el) => (el.scrollTop = 0));
  const top = (await page.locator(".screen > *").first().boundingBox())!;
  expect(top.y).toBeGreaterThanOrEqual(0);
  // А вниз — до кнопки.
  await create.scrollIntoViewIfNeeded();
  await expect(create).toBeInViewport();
});

test("на узком экране «Файл и копии» и резервные копии помещаются в ширину экрана", async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await page.goto("/");
  await page.getByLabel("Мастер-пароль").fill("password1");
  await page.getByRole("button", { name: "Разблокировать" }).click();
  await page.getByRole("button", { name: "Настройки" }).click();
  await page.getByRole("button", { name: "Файл и копии" }).click();
  const wide = () => page.evaluate(() => document.documentElement.scrollWidth);
  // Длинный путь переносится, кнопки не уезжают за край.
  await expect(page.getByRole("button", { name: "Переименовать…" })).toBeInViewport({ ratio: 1 });
  expect(await wide()).toBeLessThanOrEqual(390);

  // Копии: сначала список, копия — отдельным экраном, «Копии» возвращает к списку.
  await page.getByRole("button", { name: "Открыть резервные копии" }).click();
  const item = page.getByRole("button", { name: /12 окт., 11:42/ });
  await expect(item).toBeVisible();
  await expect(page.getByRole("button", { name: "Восстановить эту копию" })).toBeHidden();
  await item.click();
  await expect(page.getByRole("button", { name: "Восстановить эту копию" })).toBeVisible();
  await expect(item).toBeHidden();
  expect(await wide()).toBeLessThanOrEqual(390);
  await page.getByRole("button", { name: "Копии" }).click();
  await expect(item).toBeVisible();
  await page.getByRole("button", { name: "Настройки" }).click();
  await expect(page.getByRole("button", { name: "Файл и копии" })).toBeVisible();
});

test("на узком экране всё левее стрелки раздела сворачивает его", async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await page.goto("/");
  await page.getByLabel("Мастер-пароль").fill("password1");
  await page.getByRole("button", { name: "Разблокировать" }).click();
  const server = sidebar(page).getByRole("button", { name: "server", exact: true });
  const entry = sidebar(page).getByRole("button", { name: /web-02/ });
  await expect(entry).toBeVisible();
  // В отступ слева от стрелки: раздел свернулся, экран раздела не открылся.
  await server.click({ position: { x: 4, y: 10 } });
  await expect(entry).toBeHidden();
  await expect(page.getByRole("heading", { name: "server" })).toHaveCount(0);
  await server.click({ position: { x: 4, y: 10 } });
  await expect(entry).toBeVisible();
  // По названию — открывается раздел.
  await server.getByText("server").click();
  await expect(page.getByRole("heading", { name: "server" })).toBeVisible();
});

test("на узком экране у раздела узкие поля, а пустой путь не занимает строку", async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await page.goto("/");
  await page.getByLabel("Мастер-пароль").fill("password1");
  await page.getByRole("button", { name: "Разблокировать" }).click();
  await sidebar(page).getByRole("button", { name: "work", exact: true }).getByText("work").click();
  const title = page.getByRole("heading", { name: "work" });
  await expect(title).toBeVisible();
  expect((await title.boundingBox())!.x).toBeLessThanOrEqual(16);
  await expect(page.getByRole("navigation", { name: "Путь" })).toHaveCount(0);
  // У подраздела путь есть.
  await page.getByRole("button", { name: /server/ }).click();
  await expect(page.getByRole("navigation", { name: "Путь" })).toHaveText("work");
});

test("на узком экране скрытое избранное остаётся скрытым после перезапуска", async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  const open = async () => {
    await page.goto("/");
    await page.getByLabel("Мастер-пароль").fill("password1");
    await page.getByRole("button", { name: "Разблокировать" }).click();
  };
  await open();
  const head = page.getByRole("navigation", { name: "Избранное" }).getByRole("button", { name: "Избранное" });
  await head.click();
  await expect(head).toHaveAttribute("aria-expanded", "false");
  await open();
  await expect(head).toHaveAttribute("aria-expanded", "false");
  await head.click();
  await open();
  await expect(head).toHaveAttribute("aria-expanded", "true");
});

test("на узком экране «Новая запись» видна, даже когда список длиннее экрана", async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 480 });
  await page.goto("/");
  await page.getByLabel("Мастер-пароль").fill("password1");
  await page.getByRole("button", { name: "Разблокировать" }).click();
  await expect(page.getByText("mdpass", { exact: true })).toBeVisible();
  // Страница не прокручивается целиком: кнопка остаётся внизу окна.
  await expect(page.getByRole("button", { name: "Новая запись" })).toBeInViewport();
  expect(await page.evaluate(() => document.documentElement.scrollHeight)).toBeLessThanOrEqual(480);
});
