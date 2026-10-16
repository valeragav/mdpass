// Синхронизация с облаком: что знает интерфейс. Сама синхронизация идёт в Rust,
// в фоне; сюда приходит только её состояние.
import { api, type CloudView, type SyncStatus } from "./api";

export const cloud = $state<{ view: CloudView | null }>({ view: null });

export async function refreshCloud() {
  try {
    cloud.view = await api.cloudStatus();
  } catch {
    cloud.view = null;
  }
}

export function setSyncStatus(status: SyncStatus) {
  if (cloud.view) cloud.view.status = status;
}

/// Состояние синхронизации для пользователя.
export function statusText(status: SyncStatus): string {
  switch (status.kind) {
    case "idle":
      return "Ждёт синхронизации";
    case "syncing":
      return "Синхронизация…";
    case "synced": {
      const time = new Date(status.atMs).toLocaleTimeString("ru-RU", { hour: "2-digit", minute: "2-digit" });
      return `Синхронизировано в ${time}`;
    }
    case "offline":
      return "Нет связи с облаком. Изменения сохранены на компьютере и уйдут, когда связь появится";
    case "needsLogin":
      return "Вход в облако истёк: выйдите и подключите его заново";
    case "failed":
      return status.message;
  }
}
