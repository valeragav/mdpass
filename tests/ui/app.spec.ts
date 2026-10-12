import { expect, test, type Page } from "@playwright/test";

// В заглушке верный пароль — «password1», в хранилище пять выдуманных записей.

async function unlock(page: Page) {
  await page.goto("/");
  await page.getByLabel("Мастер-пароль").fill("password1");
  await page.getByRole("button", { name: "Разблокировать" }).click();
  await expect(page.getByRole("heading", { name: "alpha (инфра)" })).toBeVisible();
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
  await expect(sidebar(page).getByRole("button", { name: /alpha/ })).toHaveCount(0);
  await expect(page.getByText("1 из 5 записей")).toBeVisible();

  await page.getByRole("searchbox", { name: "Поиск" }).fill("");
  await page.getByRole("button", { name: "[dev] 1" }).click();
  await expect(page.getByText("1 из 5 записей")).toBeVisible();
});

test("раздел сворачивается и разворачивается", async ({ page }) => {
  await unlock(page);
  const server = sidebar(page).getByRole("button", { name: "server", exact: true });
  await server.click();
  await expect(server).toHaveAttribute("aria-expanded", "false");
  await expect(sidebar(page).getByRole("button", { name: /beta/ })).toHaveCount(0);
  await server.click();
  await expect(sidebar(page).getByRole("button", { name: /beta/ })).toBeVisible();
});

test("стрелки ходят по списку и полям, Ctrl+C копирует поле под курсором", async ({ page }) => {
  await unlock(page);
  await page.keyboard.press("ArrowDown");
  await expect(page.getByRole("heading", { name: "beta (стейдж)" })).toBeVisible();

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
  await page.keyboard.press("ArrowRight");
  await expect(server).toHaveAttribute("aria-expanded", "true");
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
  await expect(page.getByText("work / site")).toBeVisible();
});

test("выход из редактора с правками спрашивает подтверждение", async ({ page }) => {
  await unlock(page);
  await page.getByRole("button", { name: "Редактировать" }).click();
  await page.getByLabel("Название").fill("alpha (новая)");
  await page.keyboard.press("Escape");
  const dialog = page.getByRole("dialog", { name: "Сохранить изменения?" });
  await expect(dialog).toBeVisible();
  await dialog.getByRole("button", { name: "Сохранить" }).click();
  await expect(page.getByRole("heading", { name: "alpha (новая)" })).toBeVisible();
});

test("удаление записи спрашивает подтверждение", async ({ page }) => {
  await unlock(page);
  await page.getByRole("button", { name: "Удалить запись" }).click();
  const dialog = page.getByRole("dialog");
  await expect(dialog).toContainText("Удалить «alpha (инфра)»?");
  await dialog.getByRole("button", { name: "Удалить" }).click();
  await expect(page.getByRole("status")).toContainText("удалена");
  await expect(sidebar(page).getByRole("button", { name: /alpha/ })).toHaveCount(0);
});

test("запись перетаскивается в другой раздел", async ({ page }) => {
  await unlock(page);
  const entry = sidebar(page).getByRole("button", { name: /почта/ });
  const target = sidebar(page).getByRole("button", { name: "site", exact: true });
  await entry.dragTo(target);
  await expect(page.getByRole("status")).toContainText("Перемещено");
  await expect(sidebar(page).getByRole("button", { name: "my", exact: true })).toHaveCount(0);
});

test("горячие клавиши: Ctrl+N открывает новую запись, Esc возвращает", async ({ page }) => {
  await unlock(page);
  await page.keyboard.press("Control+n");
  await expect(page.getByRole("heading", { name: "Новая запись" })).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(page.getByRole("heading", { name: "alpha (инфра)" })).toBeVisible();
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
