import type { NoteDoc, NoteSummary, TodoItem } from "./types";

const KEY = "moyu.preview.v2";

type DB = {
  days: Record<string, TodoItem[]>;
  later: TodoItem[];
  categories: string[];
  notes: NoteDoc[];
};

function blank(): DB {
  return {
    days: {},
    later: [],
    categories: [],
    notes: [],
  };
}

function load(): DB {
  const raw = localStorage.getItem(KEY);
  if (!raw) return blank();
  try {
    return JSON.parse(raw) as DB;
  } catch {
    return blank();
  }
}

function save(db: DB) {
  localStorage.setItem(KEY, JSON.stringify(db));
}

function cleanItems(items: TodoItem[]): TodoItem[] {
  return items
    .map((item) => ({ done: item.done, text: item.text.replace(/[\r\n]/g, " ").trim() }))
    .filter((item) => item.text.length > 0 && item.text.length <= 200);
}

function asSummary(note: NoteDoc): NoteSummary {
  return {
    category: note.category,
    title: note.title,
    fileName: note.fileName,
    extension: note.extension,
    kind: note.kind,
  };
}

export async function loadDay(date: string): Promise<TodoItem[]> {
  return load().days[date] ?? [];
}

export async function saveDay(date: string, items: TodoItem[]): Promise<void> {
  const db = load();
  const cleaned = cleanItems(items);
  if (cleaned.length === 0) delete db.days[date];
  else db.days[date] = cleaned;
  save(db);
}

export async function loadLater(): Promise<TodoItem[]> {
  return load().later;
}

export async function saveLater(items: TodoItem[]): Promise<void> {
  const db = load();
  db.later = cleanItems(items);
  save(db);
}

export async function carryUnfinished(today: string): Promise<number> {
  const db = load();
  const dates = Object.keys(db.days)
    .filter((date) => date < today)
    .sort();
  const moved: TodoItem[] = [];
  for (const date of dates) {
    const items = db.days[date] ?? [];
    const done = items.filter((item) => item.done);
    const open = items.filter((item) => !item.done);
    if (open.length === 0) continue;
    moved.push(...open);
    if (done.length === 0) delete db.days[date];
    else db.days[date] = done;
  }
  if (moved.length > 0) {
    db.days[today] = [...moved, ...(db.days[today] ?? [])];
    save(db);
  }
  return moved.length;
}

export async function listCategories(): Promise<string[]> {
  return load().categories;
}

export async function listNotes(category: string | null, query: string | null): Promise<NoteSummary[]> {
  const q = (query ?? "").trim().toLowerCase();
  return load()
    .notes.filter((note) => (category ? note.category === category : true))
    .filter((note) => {
      if (!q) return true;
      return note.title.toLowerCase().includes(q) || note.fileName.toLowerCase().includes(q);
    })
    .map(asSummary);
}

export async function readNote(category: string, fileName: string): Promise<NoteDoc> {
  const note = load().notes.find((item) => item.category === category && item.fileName === fileName);
  if (!note) throw new Error("资料不存在");
  return note;
}

export async function saveNote(input: {
  category: string;
  title: string;
  body: string;
  previousFileName: string | null;
}): Promise<NoteDoc> {
  const title = input.title.trim();
  if (!title) throw new Error("名称不能为空");
  const db = load();
  if (!db.categories.includes(input.category)) throw new Error("分类不存在");
  const previous = input.previousFileName?.trim() || null;
  const previousNote = previous
    ? db.notes.find((note) => note.category === input.category && note.fileName === previous)
    : null;
  if (previousNote && previousNote.kind !== "text") throw new Error("Office 文件请用系统程序编辑");
  const extension = previousNote?.extension ?? "md";
  const fileName = `${title}.${extension}`;
  const conflict = db.notes.find((note) => note.category === input.category && note.fileName === fileName);
  if (conflict && conflict.fileName !== previous) throw new Error("已有同名资料");
  db.notes = db.notes.filter(
    (note) => !(note.category === input.category && (note.fileName === fileName || note.fileName === previous)),
  );
  const note: NoteDoc = {
    category: input.category,
    title,
    fileName,
    extension,
    kind: "text",
    body: input.body.replace(/\r\n/g, "\n").replace(/\n$/, ""),
    sizeLabel: `${Math.max(1, input.body.length)} B`,
  };
  db.notes.push(note);
  save(db);
  return note;
}

export async function deleteNote(category: string, fileName: string): Promise<void> {
  const db = load();
  db.notes = db.notes.filter((note) => !(note.category === category && note.fileName === fileName));
  save(db);
}

export async function createCategory(name: string): Promise<void> {
  const title = name.trim();
  if (!title) throw new Error("名称不能为空");
  const db = load();
  if (db.categories.includes(title)) throw new Error("已有同名分类");
  db.categories.push(title);
  save(db);
}

export async function deleteCategory(name: string): Promise<void> {
  const db = load();
  db.categories = db.categories.filter((item) => item !== name);
  db.notes = db.notes.filter((note) => note.category !== name);
  save(db);
}

export async function renameCategory(from: string, to: string): Promise<void> {
  const next = to.trim();
  if (!next) throw new Error("名称不能为空");
  const db = load();
  if (!db.categories.includes(from)) throw new Error("分类不存在");
  if (from === next) return;
  if (db.categories.includes(next)) throw new Error("已有同名分类");
  db.categories = db.categories.map((item) => (item === from ? next : item));
  for (const note of db.notes) {
    if (note.category === from) note.category = next;
  }
  save(db);
}

export async function dataDirectory(): Promise<string> {
  return "浏览器预览，内容只留在本页";
}

export async function importFile(category: string, sourcePath: string): Promise<NoteSummary> {
  const base = sourcePath.split(/[/\\]/).pop() || "未命名.docx";
  const extension = (base.includes(".") ? base.split(".").pop() : "docx")!.toLowerCase();
  const title = base.replace(/\.[^.]+$/, "") || "未命名";
  const db = load();
  if (!db.categories.includes(category)) throw new Error("分类不存在");
  let fileName = `${title}.${extension}`;
  let index = 2;
  while (db.notes.some((note) => note.category === category && note.fileName === fileName)) {
    fileName = `${title} ${index}.${extension}`;
    index += 1;
  }
  const note: NoteDoc = {
    category,
    title: fileName.replace(/\.[^.]+$/, ""),
    fileName,
    extension,
    kind: extension === "md" || extension === "txt" ? "text" : "file",
    body: "",
    sizeLabel: "1.0 KB",
  };
  db.notes.push(note);
  save(db);
  return asSummary(note);
}

export async function absolutePath(category: string, fileName: string): Promise<string> {
  return `preview://${category}/${fileName}`;
}

export async function pickAndImport(category: string): Promise<NoteSummary[]> {
  return [await importFile(category, "演示表格.xlsx")];
}

export async function openWithSystem(category: string, fileName: string): Promise<void> {
  void category;
  void fileName;
}

export async function revealFile(category: string, fileName: string): Promise<void> {
  void category;
  void fileName;
}

export async function openCategoryDirectory(category: string): Promise<void> {
  void category;
}
