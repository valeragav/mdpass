// Переводит записанное видео в docs/demo.gif. Нужен ffmpeg в PATH.
import { execFileSync } from "node:child_process";
import { readdirSync, statSync } from "node:fs";
import { join } from "node:path";

const out = "target/demo";
const videos = [];
for (const dir of readdirSync(out)) {
  const path = join(out, dir);
  if (!statSync(path).isDirectory()) continue;
  for (const file of readdirSync(path)) if (file.endsWith(".webm")) videos.push(join(path, file));
}
if (videos.length === 0) throw new Error("нет записи: сначала запустите сценарий");

// Палитра строится по самому ролику: цвета темы без полос и пятен.
const filters = "fps=12,scale=960:-1:flags=lanczos,split[a][b];[a]palettegen=max_colors=128:stats_mode=diff[p];[b][p]paletteuse=dither=bayer:bayer_scale=4:diff_mode=rectangle";
execFileSync("ffmpeg", ["-y", "-loglevel", "error", "-i", videos[0], "-vf", filters, "-loop", "0", "docs/demo.gif"], {
  stdio: "inherit",
});
const size = (statSync("docs/demo.gif").size / 1024 / 1024).toFixed(1);
console.log(`docs/demo.gif: ${size} МБ`);
