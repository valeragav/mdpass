// Управление стрелками на главном экране. Курсор стоит либо в списке слева, либо в полях
// открытой записи. Списком и полями управляют свои обработчики; кто обработал клавишу,
// отменяет её действие по умолчанию, и второй обработчик её уже не трогает.

interface KeyNav {
  zone: "list" | "fields";
  /// Номер поля под курсором в порядке показа, а не в порядке файла.
  field: number;
}

export const keyNav = $state<KeyNav>({ zone: "list", field: 0 });

/// Клавиша для навигации: не обработана, не набирается в поле ввода и не уходит в диалог.
export function isNavKey(event: KeyboardEvent): boolean {
  if (event.defaultPrevented || event.altKey) return false;
  const el = event.target as HTMLElement | null;
  if (el?.closest("input, textarea, select, [contenteditable]")) return false;
  // Enter и пробел на кнопке, до которой дошли Tab'ом, нажимают её саму. Строки
  // списка и поля записи (`data-nav`, `data-cursor`) — часть навигации, их это не касается.
  const activates = event.key === "Enter" || event.key === " ";
  if (activates && el?.closest("button, a[href], [role=switch], [role=tab]") && !el.closest("[data-nav], [data-cursor]")) {
    return false;
  }
  return document.querySelector("dialog[open]") === null;
}

/// Прокрутить к строке, которую подсветили с клавиатуры, когда интерфейс её перерисует.
export function scrollToCursor(find: () => Element | null | undefined) {
  requestAnimationFrame(() => find()?.scrollIntoView({ block: "nearest" }));
}
