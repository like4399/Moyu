import { cpSync, existsSync, mkdirSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const exe = join(root, "src-tauri", "target", "release", "moyu.exe");
const data = join(root, "data");
const out = join(root, "src-tauri", "target", "release", "portable");

if (!existsSync(exe)) {
  console.error("缺少 release 可执行文件，请先运行: npm run tauri build");
  process.exit(1);
}

rmSync(out, { recursive: true, force: true });
mkdirSync(out, { recursive: true });
cpSync(exe, join(out, "moyu.exe"));
cpSync(data, join(out, "data"), { recursive: true });
writeFileSync(
  join(out, "README.txt"),
  [
    "摸鱼大王（便携版）",
    "",
    "1. 保持 moyu.exe 与旁边的 data/ 一起拷走。",
    "2. 双击 moyu.exe 即可运行（需 Windows WebView2）。",
    "3. 待办和资料都在 data/ 里。",
    "",
  ].join("\n"),
  "utf8",
);

console.log(`便携包已生成: ${out}`);
