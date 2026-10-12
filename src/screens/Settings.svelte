<script lang="ts" module>
  // Вкладка запоминается, пока приложение открыто: после резервных копий
  // «Настройки» возвращают туда же.
  let lastTab: "security" | "file" | "look" = "security";

  /// Вариант в группе кнопок-переключателей.
  export interface Choice {
    key: string;
    label: string;
    on: boolean;
    pick: () => void;
  }
</script>

<script lang="ts">
  import { revealItemInDir } from "@tauri-apps/plugin-opener";
  import { api, errorText, type Settings } from "../lib/api";
  import { app, go } from "../lib/nav.svelte";
  import { showToast } from "../lib/toast.svelte";
  import Button from "../lib/ui/Button.svelte";
  import Icon from "../lib/ui/Icon.svelte";
  import Logo from "../lib/ui/Logo.svelte";
  import { PROTECTION_LABEL } from "../lib/ui/ProtectionPicker.svelte";
  import ChangeProtection from "./settings/ChangeProtection.svelte";

  type Tab = "security" | "file" | "look";
  const TABS: [Tab, string][] = [
    ["security", "Безопасность"],
    ["file", "Файл и копии"],
    ["look", "Внешний вид"],
  ];

  // Варианты взяты с экрана настроек в дизайне.
  const LOCK = [1, 5, 15, 30, 0];
  const CLIP = [15, 30, 60, 0];
  const KEEP = [10, 30, 100];
  const THEMES: [Settings["theme"], string][] = [
    ["dark", "Тёмная"],
    ["light", "Светлая"],
    ["system", "Как в системе"],
  ];
  const SIZES: [Settings["textSize"], string][] = [
    ["s", "Мелкий"],
    ["m", "Обычный"],
    ["l", "Крупный"],
  ];
  const SWATCHES: [string, string][] = [
    ["#E5A93D", "Янтарный"],
    ["#5BC2A8", "Бирюзовый"],
    ["#7AA7F0", "Синий"],
    ["#E57A7A", "Коралловый"],
  ];

  let tab = $state<Tab>(lastTab);
  $effect(() => {
    lastTab = tab;
  });
  let changingPassphrase = $state(false);

  async function update(patch: Partial<Settings>) {
    const before = app.settings;
    app.settings = { ...before, ...patch };
    try {
      app.settings = await api.updateSettings(app.settings);
    } catch (err) {
      app.settings = before;
      showToast(errorText(err));
    }
  }

  async function showInFolder() {
    if (!app.vault) return;
    try {
      await revealItemInDir(app.vault.path);
    } catch (err) {
      showToast(errorText(err));
    }
  }

  /// Варианты для группы кнопок-переключателей.
  function choices<T>(options: T[], current: T, label: (o: T) => string, pick: (o: T) => void): Choice[] {
    return options.map((o) => ({ key: String(o), label: label(o), on: o === current, pick: () => pick(o) }));
  }

  const themeName = (t: Settings["theme"]) => THEMES.find(([id]) => id === t)?.[1] ?? t;
  const sizeName = (t: Settings["textSize"]) => SIZES.find(([id]) => id === t)?.[1] ?? t;

  function openBackups() {
    app.backupsFrom = "settings";
    go("backups");
  }
</script>

{#snippet chips(items: Choice[], name: string)}
  <div class="chips" role="group" aria-label={name}>
    {#each items as item (item.key)}
      <button type="button" class="chip" class:on={item.on} aria-pressed={item.on} onclick={item.pick}>{item.label}</button>
    {/each}
  </div>
{/snippet}

{#snippet toggle(on: boolean, label: string, flip: () => void)}
  <button type="button" class="switch" class:on role="switch" aria-checked={on} aria-label={label} onclick={flip}>
    <span></span>
  </button>
{/snippet}

<div class="screen">
  <header>
    <Button variant="ghost" size="sm" icon="back" onclick={() => go("main")}>Назад</Button>
    <Logo size="sm" />
    <span class="spacer"></span>
  </header>

  <div class="body">
    <nav aria-label="Разделы настроек">
      <div class="nav-title">Настройки</div>
      {#each TABS as [id, label] (id)}
        <button type="button" class="tab" class:on={tab === id} aria-current={tab === id ? "page" : undefined} onclick={() => (tab = id)}>
          {label}
        </button>
      {/each}
    </nav>

    <div class="content scroll">
      <div class="inner">
        {#if tab === "security"}
          <h1>Безопасность</h1>
          <div class="row stacked">
            <div>
              <div class="name">Автоблокировка</div>
              <div class="hint">Через сколько минут бездействия хранилище блокируется</div>
            </div>
            {@render chips(
              choices(LOCK, app.settings.autoLockMinutes, (o) => (o === 0 ? "Никогда" : `${o} мин`), (o) => update({ autoLockMinutes: o })),
              "Автоблокировка",
            )}
          </div>
          <div class="row stacked">
            <div>
              <div class="name">Очистка буфера обмена</div>
              <div class="hint">Через сколько секунд после копирования буфер очищается</div>
            </div>
            {@render chips(
              choices(CLIP, app.settings.clipboardClearSeconds, (o) => (o === 0 ? "Не очищать" : `${o} с`), (o) => update({ clipboardClearSeconds: o })),
              "Очистка буфера обмена",
            )}
          </div>
          <div class="row">
            <div>
              <div class="name">Блокировать при сворачивании окна</div>
              <div class="hint">Хранилище закроется, как только вы свернёте окно</div>
            </div>
            {@render toggle(app.settings.lockOnMinimize, "Блокировать при сворачивании окна", () => update({ lockOnMinimize: !app.settings.lockOnMinimize }))}
          </div>
          <div class="row">
            <div>
              <div class="name">Защита файла</div>
              <div class="hint">
                {app.protection ? PROTECTION_LABEL[app.protection] : "—"}.
                {app.protection === "none" ? "Файл лежит открытым текстом" : "Мастер-пароль, сила шифрования или его отключение"}
              </div>
            </div>
            <Button size="sm" onclick={() => (changingPassphrase = true)}>
              {app.protection === "none" ? "Включить шифрование…" : "Изменить…"}
            </Button>
          </div>
        {:else if tab === "file"}
          <h1>Файл и копии</h1>
          <div class="row stacked">
            <div class="name">Файл хранилища</div>
            <div class="file-row">
              <div class="file mono"><Icon name="file" /><span>{app.vault?.display}</span></div>
              <Button size="sm" onclick={showInFolder}>Показать в папке</Button>
            </div>
          </div>
          <div class="row">
            <div>
              <div class="name">Копия перед каждой записью</div>
              <div class="hint">Зашифрованная копия создаётся до того, как файл будет перезаписан</div>
            </div>
            {@render toggle(app.settings.backupsEnabled, "Копия перед каждой записью", () => update({ backupsEnabled: !app.settings.backupsEnabled }))}
          </div>
          <div class="row stacked">
            <div>
              <div class="name">Сколько копий хранить</div>
              <div class="hint">Старые копии удаляются автоматически</div>
            </div>
            {@render chips(
              choices(KEEP, app.settings.backupsToKeep, String, (o) => update({ backupsToKeep: o })),
              "Сколько копий хранить",
            )}
          </div>
          <div>
            <Button icon="generate" onclick={openBackups}>Открыть резервные копии</Button>
          </div>
        {:else}
          <h1>Внешний вид</h1>
          <div class="row stacked">
            <div class="name">Тема</div>
            {@render chips(
              choices(THEMES.map(([t]) => t), app.settings.theme, themeName, (o) => update({ theme: o })),
              "Тема",
            )}
          </div>
          <div class="row stacked">
            <div class="name">Акцентный цвет</div>
            <div class="swatches" role="group" aria-label="Акцентный цвет">
              {#each SWATCHES as [color, label] (color)}
                <button
                  type="button"
                  class="swatch"
                  class:on={app.settings.accent.toUpperCase() === color}
                  aria-label={label}
                  aria-pressed={app.settings.accent.toUpperCase() === color}
                  title={label}
                  onclick={() => update({ accent: color })}
                >
                  <span style:background={color}></span>
                </button>
              {/each}
            </div>
          </div>
          <div class="row stacked">
            <div class="name">Размер текста в записях</div>
            {@render chips(
              choices(SIZES.map(([z]) => z), app.settings.textSize, sizeName, (o) => update({ textSize: o })),
              "Размер текста в записях",
            )}
          </div>
        {/if}
      </div>
    </div>
  </div>
</div>

<ChangeProtection open={changingPassphrase} onclose={() => (changingPassphrase = false)} />

<style>
  .screen {
    height: 100%;
    display: flex;
    flex-direction: column;
  }

  header {
    height: 48px;
    flex: none;
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0 16px;
    border-bottom: 1px solid var(--line);
    background: var(--bg-chrome);
  }

  header :global(.btn) {
    height: 36px;
  }

  .spacer {
    width: 120px;
  }

  .body {
    flex: 1;
    min-height: 0;
    display: flex;
  }

  nav {
    width: 260px;
    flex: none;
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 24px 12px;
    border-right: 1px solid var(--line);
    background: var(--bg-chrome);
  }

  .nav-title {
    padding: 0 12px 12px;
    font-family: var(--font-mono);
    font-size: 12px;
    color: var(--text-subtle);
  }

  .tab {
    height: 40px;
    padding: 0 12px;
    border-radius: var(--radius);
    color: var(--text-muted);
    font-weight: 500;
  }

  .tab:hover {
    background: var(--bg-hover);
  }

  .tab.on {
    background: color-mix(in srgb, var(--accent) 12%, transparent);
    color: var(--text-strong);
  }

  .content {
    flex: 1;
    min-width: 0;
    overflow-y: auto;
    padding: 32px 48px 40px;
  }

  .inner {
    max-width: 640px;
    display: flex;
    flex-direction: column;
    gap: 28px;
  }

  h1 {
    font-size: 24px;
    font-weight: 600;
    letter-spacing: -0.01em;
  }

  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 24px;
  }

  .row.stacked {
    flex-direction: column;
    align-items: stretch;
    gap: 12px;
  }

  .name {
    font-weight: 500;
  }

  .hint {
    margin-top: 2px;
    font-size: 13px;
    color: var(--text-subtle);
  }

  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }

  .chip {
    height: 36px;
    padding: 0 14px;
    border-radius: var(--radius);
    border: 1px solid var(--line-strong);
    color: var(--text-2);
    font-weight: 500;
  }

  .chip:hover {
    background: var(--bg-hover-strong);
  }

  .chip.on {
    border-color: var(--accent);
    background: color-mix(in srgb, var(--accent) 12%, transparent);
    color: var(--text-strong);
  }

  .switch {
    position: relative;
    width: 44px;
    height: 26px;
    flex: none;
    border-radius: 999px;
    background: var(--line-dashed);
    transition: background 0.15s;
  }

  .switch span {
    position: absolute;
    top: 3px;
    left: 3px;
    width: 20px;
    height: 20px;
    border-radius: 50%;
    background: #fff;
    transition: left 0.15s;
  }

  .switch.on {
    background: var(--accent);
  }

  .switch.on span {
    left: 21px;
  }

  .file-row {
    display: flex;
    gap: 10px;
    align-items: center;
  }

  .file {
    flex: 1;
    min-width: 0;
    height: 40px;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 0 14px;
    border-radius: var(--radius);
    border: 1px solid var(--line);
    background: var(--bg-chrome);
    font-size: 13px;
    color: var(--text-muted);
    overflow: hidden;
    white-space: nowrap;
  }

  .swatches {
    display: flex;
    gap: 12px;
  }

  .swatch {
    width: 44px;
    height: 44px;
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: 50%;
    border: 2px solid transparent;
  }

  .swatch.on {
    border-color: var(--text);
  }

  .swatch span {
    width: 30px;
    height: 30px;
    border-radius: 50%;
  }
</style>
