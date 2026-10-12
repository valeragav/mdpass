/// Русская форма слова для числа: plural(3, ["секунду", "секунды", "секунд"]).
export function plural(n: number, [one, few, many]: [string, string, string]): string {
  const tens = n % 100;
  const units = n % 10;
  if (tens >= 11 && tens <= 14) return many;
  if (units === 1) return one;
  if (units >= 2 && units <= 4) return few;
  return many;
}

const MIN_MASTER_LENGTH = 8;

/// Минимальная длина мастер-пароля. Повторяет `Protection::min_passphrase_chars` в Rust.
export const minMasterLength = (protection: "strong" | "weak" | "none") => (protection === "weak" ? 4 : MIN_MASTER_LENGTH);

/// Надёжность пароля от 0 до 4, как на экране первого запуска в дизайне.
export function strength(password: string): { score: number; label: string } {
  let score = 0;
  if (password.length >= MIN_MASTER_LENGTH) score++;
  if (password.length >= 12) score++;
  if (/[A-ZА-ЯЁ]/.test(password) && /[a-zа-яё]/.test(password)) score++;
  if (/\d/.test(password) && /[^\p{L}\d]/u.test(password)) score++;
  // Длинная фраза из нескольких слов надёжна и без цифр и регистра.
  if (password.length >= 20 && password.trim().split(/\s+/).length >= 4) score = 4;
  if (!password) score = 0;
  const labels = ["", "Слабый", "Средний", "Хороший", "Надёжный"];
  return { score, label: labels[score] };
}
