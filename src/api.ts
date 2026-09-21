import * as mock from "./mock";
import type { NoteDoc, NoteSummary, TodoItem } from "./types";

function inTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<T>(command, args);
}

export async function loadDay(date: string): Promise<TodoItem[]> {
  if (!inTauri()) return mock.loadDay(date);
  return call("load_day", { date });
}

export async function saveDay(date: string, items: TodoItem[]): Promise<void> {
  if (!inTauri()) return mock.saveDay(date, items);
  return call("save_day", { date, items });
}

export async function carryUnfinished(today: string): Promise<number> {
  if (!inTauri()) return mock.carryUnfinished(today);
  return call("carry_unfinished", { today });
}

export async function listCategories(): Promise<string[]> {
  if (!inTauri()) return mock.listCategories();
  return call("list_categories");
}

export async function listNotes(category: string | null, query: string | null): Promise<NoteSummary[]> {
  if (!inTauri()) return mock.listNotes(category, query);
  return call("list_notes", { category, query });
}

export async function readNote(category: string, fileName: string): Promise<NoteDoc> {
  if (!inTauri()) return mock.readNote(category, fileName);
  return call("read_note", { category, fileName });
}

export async function saveNote(input: {
  category: string;
  title: string;
  body: string;
  previousFileName: string | null;
}): Promise<NoteDoc> {
  if (!inTauri()) return mock.saveNote(input);
  return call("save_note", {
    category: input.category,
    title: input.title,
    body: input.body,
    previousFileName: input.previousFileName,
  });
}

export async function deleteNote(category: string, fileName: string): Promise<void> {
  if (!inTauri()) return mock.deleteNote(category, fileName);
  return call("delete_note", { category, fileName });
}

export async function createCategory(name: string): Promise<void> {
  if (!inTauri()) return mock.createCategory(name);
  return call("create_category", { name });
}

export async function deleteCategory(name: string): Promise<void> {
  if (!inTauri()) return mock.deleteCategory(name);
  return call("delete_category", { name });
}

export async function renameCategory(from: string, to: string): Promise<void> {
  if (!inTauri()) return mock.renameCategory(from, to);
  return call("rename_category", { from, to });
}

export async function reorderCategories(names: string[]): Promise<void> {
  if (!inTauri()) return mock.reorderCategories(names);
  return call("reorder_categories", { names });
}

export async function reorderFiles(category: string, names: string[]): Promise<void> {
  if (!inTauri()) return mock.reorderFiles(category, names);
  return call("reorder_files", { category, names });
}

export async function dataDirectory(): Promise<string> {
  if (!inTauri()) return mock.dataDirectory();
  return call("data_directory");
}

export async function openDataDirectory(): Promise<void> {
  if (!inTauri()) throw new Error("当前是浏览器预览，没有本地目录");
  return call("open_data_directory");
}

export async function openCategoryDirectory(category: string): Promise<void> {
  if (!inTauri()) return mock.openCategoryDirectory(category);
  return call("open_category_directory", { category });
}

export async function saveExport(start: string, end: string, content: string): Promise<boolean> {
  if (!inTauri()) {
    const blob = new Blob([content], { type: "text/markdown;charset=utf-8" });
    const url = URL.createObjectURL(blob);
    const link = document.createElement("a");
    link.href = url;
    link.download = `待办-${start}-${end}.md`;
    link.click();
    URL.revokeObjectURL(url);
    return true;
  }
  const { save } = await import("@tauri-apps/plugin-dialog");
  const path = await save({
    title: "导出待办",
    defaultPath: `待办-${start}-${end}.md`,
    filters: [{ name: "Markdown", extensions: ["md"] }],
  });
  if (!path) return false;
  await call("write_export", { destination: path, content });
  return true;
}

export async function importFile(category: string, sourcePath: string): Promise<NoteSummary> {
  if (!inTauri()) return mock.importFile(category, sourcePath);
  return call("import_file", { category, sourcePath });
}

export async function absolutePath(category: string, fileName: string): Promise<string> {
  if (!inTauri()) return mock.absolutePath(category, fileName);
  return call("absolute_path", { category, fileName });
}

export async function pickAndImport(category: string): Promise<NoteSummary[]> {
  if (!inTauri()) return mock.pickAndImport(category);
  const { open } = await import("@tauri-apps/plugin-dialog");
  const selected = await open({
    multiple: true,
    title: "导入到资料库",
  });
  if (!selected) return [];
  const paths = Array.isArray(selected) ? selected : [selected];
  const imported: NoteSummary[] = [];
  for (const path of paths) {
    imported.push(await importFile(category, path));
  }
  return imported;
}

export async function openWithSystem(category: string, fileName: string): Promise<void> {
  if (!inTauri()) return mock.openWithSystem(category, fileName);
  return call("open_stored_file", { category, fileName });
}

export async function revealFile(category: string, fileName: string): Promise<void> {
  if (!inTauri()) return mock.revealFile(category, fileName);
  return call("reveal_stored_file", { category, fileName });
}
