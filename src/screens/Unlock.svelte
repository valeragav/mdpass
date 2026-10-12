<script lang="ts">
  import { go } from "../lib/nav.svelte";
  import Button from "../lib/ui/Button.svelte";
  import Icon from "../lib/ui/Icon.svelte";
  import Logo from "../lib/ui/Logo.svelte";
  import PasswordInput from "../lib/ui/PasswordInput.svelte";

  let password = $state("");

  function submit(event: SubmitEvent) {
    event.preventDefault();
    // Проверка пароля появится на этапе 3, пока экран только показывает тему.
    go("main");
  }
</script>

<div class="screen">
  <Logo size="lg" vertical />

  <form class="card" onsubmit={submit}>
    <div class="file mono">
      <Icon name="file" />
      <span>vault.md.age</span>
    </div>

    <div class="field">
      <label for="master">Мастер-пароль</label>
      <PasswordInput id="master" bind:value={password} placeholder="Введите мастер-пароль" autofocus />
    </div>

    <Button variant="primary" type="submit" icon="unlock" full>Разблокировать</Button>
    <Button variant="ghost" size="sm" full onclick={() => go("welcome")}>Создать новое хранилище</Button>
  </form>

  <p class="hint">Файл расшифровывается только в памяти и блокируется автоматически через 5 минут бездействия</p>
</div>

<style>
  .screen {
    height: 100%;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 28px;
    padding: 24px;
  }

  .card {
    width: 100%;
    max-width: 400px;
    display: flex;
    flex-direction: column;
    gap: 20px;
    padding: 28px;
    border-radius: var(--radius-card);
    background: var(--bg-panel);
    border: 1px solid var(--line);
  }

  .file {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 10px 14px;
    border-radius: var(--radius);
    background: var(--bg-chrome);
    border: 1px solid var(--line);
    font-size: 13px;
    color: var(--text-muted);
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  label {
    font-weight: 500;
  }

  .hint {
    max-width: 400px;
    text-align: center;
    font-size: 12px;
    color: var(--text-subtle);
  }
</style>
