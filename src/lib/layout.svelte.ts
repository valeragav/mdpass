// Раскладка под ширину окна. На узком экране (телефон) список записей и запись
// не помещаются рядом: показывается что-то одно, запись открывается отдельным экраном.

/// Ширина, до которой экран считается узким.
const NARROW = "(max-width: 640px)";

export const layout = $state({
  narrow: false,
  /// На узком экране открыта запись или раздел, а не список.
  detail: false,
});

if (typeof window !== "undefined" && window.matchMedia) {
  const query = window.matchMedia(NARROW);
  layout.narrow = query.matches;
  query.addEventListener("change", (event) => {
    layout.narrow = event.matches;
    if (!event.matches) layout.detail = false;
  });
}

/// Открыть запись или раздел отдельным экраном (на широком экране ничего не меняет).
export function openDetail() {
  if (layout.narrow) layout.detail = true;
}

/// Вернуться к списку.
export function closeDetail() {
  layout.detail = false;
}
