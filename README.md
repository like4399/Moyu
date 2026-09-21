# 摸鱼大王

本地桌面记事：今日待办和资料库。数据是 Markdown / Office 文件，拷走 `data` 文件夹就能换电脑。

## 技术栈

- Tauri 2 + Rust（本地文件读写）
- TypeScript + HTML/CSS + Vite（界面）
- 存储：`data/**/*.md` 与 Word/Excel/PPT/PDF 等

## 开发运行

需要 Node.js、Rust，以及 Windows WebView2。

```bash
npm install
npm run tauri dev
```

开发时数据在项目里的 `data/`。可用环境变量 `MOYU_DATA` 指定别的目录。

## 打包与带走

```bash
npm run tauri build
npm run package:portable
```

产物：

- 可执行文件：`src-tauri/target/release/moyu.exe`
- 便携目录：`src-tauri/target/release/portable/`（exe + `data/` 样例）

把 `portable` 整个文件夹拷到另一台电脑，双击 `moyu.exe` 即可。打包后的程序会在 exe 旁边读写 `data/`。

## 测试

```bash
npm test
npm run test:rust
npm run build
```

## 数据约定

- `data/todos/YYYY-MM-DD.md`：某一天的待办，`- [ ]` / `- [x]`。未完成在上，已完成在下
- `data/notes/<分类>/`：资料文件柜。文件夹里的普通文件都会列出，单击用系统程序打开，应用内不编辑
- `data/.meta/index.md`：自动生成的目录，不用手改
- `data/.meta/order.md`：分类和文件的拖动顺序，不用手改。新出现的项会排到已有顺序后面
