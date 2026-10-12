// Короткое уведомление внизу окна: «Скопировано», «Запись сохранена».
export const toast = $state({ text: "" });

let timer: ReturnType<typeof setTimeout> | undefined;

export function showToast(text: string) {
  toast.text = text;
  clearTimeout(timer);
  timer = setTimeout(() => (toast.text = ""), 2400);
}
