/// Приложение запущено на телефоне. На узком окне компьютера — `false`: там важна
/// ширина (`layout.narrow`), а не устройство.
export const IS_MOBILE = typeof navigator !== "undefined" && /Android|iPhone|iPad/i.test(navigator.userAgent);
