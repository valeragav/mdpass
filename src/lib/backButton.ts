// Системная кнопка «Назад» на телефоне. WebView на Android сначала идёт назад по истории
// страницы и только когда идти некуда, закрывает приложение. Пока на экране есть куда
// вернуться, в истории лежит одна лишняя запись: «Назад» снимает её, а приложение
// закрывает диалог или нажимает кнопку «назад» экрана.

/// Открытый диалог (последний сверху) или кнопка «назад» на экране.
function target(): HTMLElement | null {
  const dialogs = document.querySelectorAll<HTMLDialogElement>("dialog[open]");
  return dialogs[dialogs.length - 1] ?? document.querySelector<HTMLElement>("[data-back]");
}

export function handleBackButton() {
  let armed = false;
  // Возвращаемся на корень сами: следующий popstate не нажатие «Назад».
  let unwinding = false;

  function sync() {
    if (unwinding) return;
    const want = target() !== null;
    if (want && !armed) {
      history.pushState({ back: true }, "");
      armed = true;
    } else if (!want && armed) {
      unwinding = true;
      history.back();
    }
  }

  window.addEventListener("popstate", () => {
    armed = false;
    if (unwinding) {
      unwinding = false;
    } else {
      const el = target();
      // Диалог закрывается так же, как по Esc: обработчик cancel у каждого свой.
      if (el instanceof HTMLDialogElement) el.dispatchEvent(new Event("cancel", { cancelable: true }));
      else el?.click();
    }
    sync();
  });

  // Экраны и диалоги меняются не только по нажатию, поэтому следим за самой страницей.
  new MutationObserver(sync).observe(document.body, {
    subtree: true,
    childList: true,
    attributes: true,
    attributeFilter: ["open", "data-back"],
  });
  sync();
}
