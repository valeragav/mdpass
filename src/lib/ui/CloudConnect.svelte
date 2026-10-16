<script lang="ts">
  // Подключение облака своим токеном: пользователь получает его на странице облака
  // сам и вводит сюда вместе с Client ID своего приложения. mdpass никуда сам не ходит.
  import { api, errorText, type CloudProvider } from "../api";
  import Button from "./Button.svelte";

  interface Props {
    ondone: () => void;
  }

  let { ondone }: Props = $props();

  const PROVIDERS: [CloudProvider, string][] = [
    ["yandex", "Яндекс Диск"],
    ["google", "Google Drive"],
  ];
  const HINT: Record<CloudProvider, string> = {
    yandex:
      "Создайте приложение на oauth.yandex.ru с доступом к папке «Приложения/mdpass» (cloud_api:disk.app_folder), получите токен и вставьте его сюда.",
    google:
      "Создайте проект в Google Cloud Console, включите Drive API, выпустите OAuth-токен с правом drive.appdata (служебная папка приложения) и вставьте его сюда.",
  };

  let provider = $state<CloudProvider>("yandex");
  let clientId = $state("");
  let token = $state("");
  let busy = $state(false);
  let error = $state<string | null>(null);

  async function connect(event: SubmitEvent) {
    event.preventDefault();
    busy = true;
    error = null;
    try {
      await api.cloudConnect(provider, clientId, token);
      ondone();
    } catch (err) {
      error = errorText(err);
    } finally {
      busy = false;
    }
  }
</script>

<form class="connect" onsubmit={connect}>
  <div class="chips" role="group" aria-label="Облако">
    {#each PROVIDERS as [id, label] (id)}
      <button type="button" class="chip" class:on={provider === id} aria-pressed={provider === id} onclick={() => (provider = id)}>
        {label}
      </button>
    {/each}
  </div>
  <p class="hint">{HINT[provider]} Пароль от облака mdpass не видит.</p>
  {#if error}
    <p class="error">{error}</p>
  {/if}
  <label for="cloud-client-id">Client ID</label>
  <input id="cloud-client-id" class="mono" bind:value={clientId} placeholder="необязательно, только для справки" />
  <label for="cloud-token">OAuth-токен</label>
  <input id="cloud-token" class="mono" type="password" bind:value={token} placeholder="токен" required />
  <Button variant="primary" type="submit" full disabled={busy || !token.trim()}>
    {busy ? "Подключаем…" : "Подключить"}
  </Button>
</form>

<style>
  .connect {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .chips {
    display: flex;
    gap: 8px;
  }

  .chip {
    height: 32px;
    padding: 0 12px;
    border-radius: var(--radius);
    border: 1px solid var(--line);
    background: var(--bg-chrome);
    color: var(--text-muted);
    font-size: 13px;
  }

  .chip.on {
    border-color: var(--accent);
    color: var(--text-strong);
    background: color-mix(in srgb, var(--accent) 12%, transparent);
  }

  label {
    font-size: 13px;
    font-weight: 500;
  }

  input {
    height: 36px;
    padding: 0 10px;
    border-radius: var(--radius);
    border: 1px solid var(--line);
    background: var(--bg-chrome);
    color: var(--text-strong);
  }

  .hint {
    color: var(--text-muted);
    font-size: 13px;
  }

  .error {
    color: var(--danger);
  }
</style>
