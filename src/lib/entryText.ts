// Правила формата, которые нужны интерфейсу. Повторяют `crates/mdpass-core/src/format.rs`:
// если правило меняется там, его нужно поменять и здесь.

const SECRET_WORDS = ["password", "passwd", "token", "secret"];
const SECRET_NAMES = ["pass", "pwd", "pin", "otp", "totp", "api_key", "apikey", "private_key"];

/// Секретно ли поле по имени ключа. У таких полей признак секрета не снимается.
export function isSecretName(key: string): boolean {
  const k = key.trim().toLowerCase();
  return SECRET_WORDS.some((w) => k.includes(w)) || SECRET_NAMES.includes(k);
}

export interface PreviewLine {
  prefix: string;
  key: string;
  value: string;
  tone: "muted" | "title" | "plain" | "placeholder";
}

export interface PreviewEntry {
  path: string[];
  title: string;
  tags: string[];
  /// `value: null` — значение скрыто и показывается точками.
  fields: { key: string; value: string | null }[];
}

/// Строки записи в том виде, в каком она лежит в файле.
export function previewLines(entry: PreviewEntry): PreviewLine[] {
  const out: PreviewLine[] = [];
  entry.path.forEach((part, i) => out.push({ prefix: "#".repeat(i + 1) + " ", key: "", value: part, tone: "muted" }));
  // Запись в файле — строка с названием жирным: **название**.
  out.push({
    prefix: "",
    key: "",
    value: `**${entry.title || "название"}**`,
    tone: entry.title ? "title" : "placeholder",
  });

  const tagsLine = entry.tags.length ? entry.tags.map((t) => `[${t}]`).join(", ") : null;
  // Теги пишутся перед заметкой, если она есть.
  const noteAt = entry.fields.findIndex((f) => f.key.toLowerCase() === "note");
  entry.fields.forEach((field, i) => {
    if (tagsLine && i === noteAt) out.push({ prefix: "", key: "tags:", value: " " + tagsLine, tone: "plain" });
    const value = field.value ?? "••••••••";
    const [first, ...rest] = value.split("\n");
    out.push({ prefix: "", key: field.key + ":", value: first ? " " + first : "", tone: "plain" });
    for (const line of rest) out.push({ prefix: "  ", key: "", value: line, tone: "plain" });
  });
  if (tagsLine && noteAt === -1) out.push({ prefix: "", key: "tags:", value: " " + tagsLine, tone: "plain" });
  return out;
}

/// «work / server» → ["work", "server"].
export const parsePath = (text: string) =>
  text
    .split("/")
    .map((p) => p.trim())
    .filter(Boolean);

/// «work, prod» → ["work", "prod"].
export const parseTags = (text: string) =>
  text
    .split(",")
    .map((t) => t.trim().replace(/^\[|\]$/g, "").trim())
    .filter(Boolean);
