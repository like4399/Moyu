import "./styles.css";
import * as api from "./api";
import { addDays, eachDay, isValidISODate, MAX_RANGE_DAYS, normalizeRange, prettyDate, rangeTitle, todayISO } from "./dates";
import { formatTodoExport } from "./export";
import type { NoteSummary, TodoItem } from "./types";

type ListName = "day" | "later";
type ViewName = "todos" | "notes";
const $ = <T extends HTMLElement>(selector: string) => document.querySelector(selector) as T;

const navTodos = $("#nav-todos");
const navNotes = $("#nav-notes");
const viewTodos = $("#view-todos");
const viewNotes = $("#view-notes");
const dateKicker = $("#date-kicker");
const dateHeading = $("#date-heading");
const dateStart = $<HTMLInputElement>("#date-start");
const dateEnd = $<HTMLInputElement>("#date-end");
const dateStartChip = $("#date-start-chip");
const dateEndChip = $("#date-end-chip");
const dateStartText = $("#date-start-text");
const dateEndText = $("#date-end-text");
const datePrev = $("#date-prev");
const dateToday = $<HTMLButtonElement>("#date-today");
const exportTodos = $("#export-todos");
const dateNext = $("#date-next");
const dayList = $<HTMLUListElement>("#day-list");
const laterList = $<HTMLUListElement>("#later-list");
const dayForm = $<HTMLFormElement>("#day-form");
const laterForm = $<HTMLFormElement>("#later-form");
const dayInput = $<HTMLInputElement>("#day-input");
const laterInput = $<HTMLInputElement>("#later-input");
const categoryForm = $<HTMLFormElement>("#category-form");
const categoryList = $("#category-list");
const noteList = $("#note-list");
const categoryInput = $<HTMLInputElement>("#category-input");
const categoryCount = $("#category-count");
const searchInput = $<HTMLInputElement>("#search-input");
const importFilesBtn = $<HTMLButtonElement>("#import-files");
const fileEmpty = $("#file-empty");
const toast = $("#toast");
const modal = $("#modal");
const modalText = $("#modal-text");
const modalOk = $<HTMLButtonElement>("#modal-ok");
const modalCancel = $<HTMLButtonElement>("#modal-cancel");

type DayBucket = { date: string; items: TodoItem[] };

let rangeStart = todayISO();
let rangeEnd = todayISO();
let dayBuckets: DayBucket[] = [];
let laterItems: TodoItem[] = [];
let categories: string[] = [];
let notes: NoteSummary[] = [];
let activeCategory = "";
let loadedFileName = "";
let query = "";
let toastTimer = 0;
let searchTimer = 0;
let editingCategory = "";

function errorText(error: unknown): string {
  if (typeof error === "string" && error.trim()) return error;
  if (error instanceof Error && error.message.trim()) return error.message;
  return "操作没有完成";
}

function showError(error: unknown) {
  toast.hidden = false;
  toast.textContent = errorText(error);
  window.clearTimeout(toastTimer);
  toastTimer = window.setTimeout(() => {
    toast.hidden = true;
  }, 3200);
}

function showToast(message: string) {
  toast.hidden = false;
  toast.textContent = message;
  window.clearTimeout(toastTimer);
  toastTimer = window.setTimeout(() => {
    toast.hidden = true;
  }, 2200);
}

function confirmAction(text: string): Promise<boolean> {
  modalText.textContent = text;
  modal.hidden = false;
  modalOk.focus();
  return new Promise((resolve) => {
    const finish = (value: boolean) => {
      modal.hidden = true;
      modalOk.removeEventListener("click", onOk);
      modalCancel.removeEventListener("click", onCancel);
      document.removeEventListener("keydown", onKey);
      resolve(value);
    };
    const onOk = () => finish(true);
    const onCancel = () => finish(false);
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") finish(false);
    };
    modalOk.addEventListener("click", onOk);
    modalCancel.addEventListener("click", onCancel);
    document.addEventListener("keydown", onKey);
  });
}

function bucket(date: string): DayBucket | undefined {
  return dayBuckets.find((item) => item.date === date);
}

function endBucket(): DayBucket {
  return bucket(rangeEnd) ?? dayBuckets[dayBuckets.length - 1] ?? { date: rangeEnd, items: [] };
}

async function persistDay(date: string) {
  const current = bucket(date);
  if (!current) return;
  try {
    await api.saveDay(date, current.items);
    if (date < todayISO()) await refreshAfterCarry();
  } catch (error) {
    showError(error);
    current.items = await api.loadDay(date);
    renderTodos();
  }
}

async function refreshAfterCarry() {
  await setRange(rangeStart, rangeEnd);
}

async function persistLater() {
  try {
    await api.saveLater(laterItems);
  } catch (error) {
    showError(error);
    laterItems = await api.loadLater();
    renderTodos();
  }
}

function renderTodoList(list: HTMLUListElement, items: TodoItem[], which: ListName, date = "") {
  list.replaceChildren();
  if (items.length === 0 && which === "day" && !date) {
    const empty = document.createElement("li");
    empty.className = "empty";
    empty.textContent = "这一天还没有待办";
    list.append(empty);
    return;
  }
  if (items.length === 0 && which === "later") {
    const empty = document.createElement("li");
    empty.className = "empty";
    empty.textContent = "没有稍后事项";
    list.append(empty);
    return;
  }
  items.forEach((item, index) => {
    const row = document.createElement("li");
    row.className = item.done ? "todo done" : "todo";
    row.dataset.which = which;
    row.dataset.index = String(index);
    if (date) row.dataset.date = date;

    const box = document.createElement("input");
    box.type = "checkbox";
    box.checked = item.done;
    box.setAttribute("aria-label", item.done ? "标为未完成" : "标为完成");

    const text = document.createElement("input");
    text.className = "todo-text";
    text.value = item.text;
    text.maxLength = 200;
    text.setAttribute("aria-label", "待办内容");

    const remove = document.createElement("button");
    remove.type = "button";
    remove.className = "btn ghost";
    remove.dataset.action = "delete";
    remove.textContent = "删除";

    row.append(box, text, remove);
    list.append(row);
  });
}

function renderDaySection() {
  dayList.replaceChildren();
  const single = rangeStart === rangeEnd;
  if (single) {
    const items = bucket(rangeStart)?.items ?? [];
    if (items.length === 0) {
      const empty = document.createElement("li");
      empty.className = "empty";
      empty.textContent = "这一天还没有待办";
      dayList.append(empty);
      return;
    }
    renderTodoList(dayList, items, "day", rangeStart);
    return;
  }

  const filled = dayBuckets.filter((item) => item.items.length > 0);
  if (filled.length === 0) {
    const empty = document.createElement("li");
    empty.className = "empty";
    empty.textContent = "这一段还没有待办";
    dayList.append(empty);
    return;
  }
  for (const day of filled) {
    const group = document.createElement("li");
    group.className = "day-group";
    const title = document.createElement("div");
    title.className = "day-group-title";
    title.textContent = `${prettyDate(day.date)} · ${day.date}`;
    const inner = document.createElement("ul");
    inner.className = "todo-list nested";
    renderTodoList(inner, day.items, "day", day.date);
    group.append(title, inner);
    dayList.append(group);
  }
}

function renderTodos() {
  const today = todayISO();
  const single = rangeStart === rangeEnd;
  dateKicker.textContent = single ? rangeStart : `${rangeStart} 至 ${rangeEnd}`;
  dateHeading.textContent = rangeTitle(rangeStart, rangeEnd, today);
  dateStart.value = rangeStart;
  dateEnd.value = rangeEnd;
  dateStartText.textContent = rangeStart.replaceAll("-", "/");
  dateEndText.textContent = rangeEnd.replaceAll("-", "/");
  dateToday.disabled = single && rangeStart === today;
  dateToday.setAttribute("aria-pressed", dateToday.disabled ? "true" : "false");
  const all = dayBuckets.flatMap((item) => item.items);
  const dayCount = document.querySelector("#day-count");
  if (dayCount) {
    const done = all.filter((item) => item.done).length;
    dayCount.textContent = all.length ? `${done} / ${all.length}` : "";
  }
  if (single) {
    dayInput.placeholder =
      rangeStart === today ? "写下今天要做的一件事，回车添加" : "写下这一天要做的一件事，回车添加";
  } else {
    dayInput.placeholder = `添加到 ${prettyDate(rangeEnd, today)}，回车添加`;
  }
  renderDaySection();
  renderTodoList(laterList, laterItems, "later");
}

async function setRange(start: string, end: string) {
  if (!isValidISODate(start) || !isValidISODate(end)) return;
  const next = normalizeRange(start, end);
  if (next.clamped) showToast(`一次最多看 ${MAX_RANGE_DAYS} 天，已截到这个范围`);
  rangeStart = next.start;
  rangeEnd = next.end;
  await api.carryUnfinished(todayISO());
  dayBuckets = [];
  for (const date of eachDay(rangeStart, rangeEnd)) {
    dayBuckets.push({ date, items: await api.loadDay(date) });
  }
  renderTodos();
}

function bindTodoList(list: HTMLUListElement) {
  list.addEventListener("change", (event) => {
    const target = event.target as HTMLInputElement;
    if (target.type !== "checkbox") return;
    const row = target.closest<HTMLElement>(".todo");
    if (!row?.dataset.which || row.dataset.index == null) return;
    const which = row.dataset.which as ListName;
    const index = Number(row.dataset.index);
    if (which === "day") {
      const current = bucket(row.dataset.date ?? "");
      if (!current) return;
      current.items[index].done = target.checked;
      row.classList.toggle("done", target.checked);
      void persistDay(current.date);
      return;
    }
    laterItems[index].done = target.checked;
    row.classList.toggle("done", target.checked);
    void persistLater();
  });

  list.addEventListener("focusout", (event) => {
    const target = event.target as HTMLInputElement;
    if (!target.classList.contains("todo-text")) return;
    const row = target.closest<HTMLElement>(".todo");
    if (!row?.dataset.which || row.dataset.index == null) return;
    const which = row.dataset.which as ListName;
    const index = Number(row.dataset.index);
    const source = which === "day" ? bucket(row.dataset.date ?? "")?.items : laterItems;
    if (!source) return;
    const text = target.value.trim();
    if (!text) {
      target.value = source[index].text;
      showError("待办内容不能为空");
      return;
    }
    if (text === source[index].text) return;
    source[index].text = text;
    if (which === "day") void persistDay(row.dataset.date ?? "");
    else void persistLater();
  });

  list.addEventListener("click", (event) => {
    const button = (event.target as HTMLElement).closest("button");
    if (button?.dataset.action !== "delete") return;
    const row = button.closest<HTMLElement>(".todo");
    if (!row?.dataset.which || row.dataset.index == null) return;
    const which = row.dataset.which as ListName;
    const index = Number(row.dataset.index);
    if (which === "day") {
      const current = bucket(row.dataset.date ?? "");
      if (!current) return;
      current.items.splice(index, 1);
      renderTodos();
      void persistDay(current.date);
      return;
    }
    laterItems.splice(index, 1);
    renderTodos();
    void persistLater();
  });
}

function bindAddForm(form: HTMLFormElement, input: HTMLInputElement, which: ListName) {
  form.addEventListener("submit", (event) => {
    event.preventDefault();
    const text = input.value.trim();
    if (!text) return;
    if (which === "day") {
      const current = endBucket();
      if (!dayBuckets.includes(current)) dayBuckets.push(current);
      current.items.push({ text, done: false });
      input.value = "";
      renderTodos();
      void persistDay(current.date);
    } else {
      laterItems.push({ text, done: false });
      input.value = "";
      renderTodos();
      void persistLater();
    }
    input.focus();
  });
}

function showView(next: ViewName) {
  viewTodos.hidden = next !== "todos";
  viewNotes.hidden = next !== "notes";
  navTodos.classList.toggle("active", next === "todos");
  navNotes.classList.toggle("active", next === "notes");
  if (next === "todos") {
    navTodos.setAttribute("aria-current", "page");
    navNotes.removeAttribute("aria-current");
  } else {
    navNotes.setAttribute("aria-current", "page");
    navTodos.removeAttribute("aria-current");
  }
  if (next === "notes") viewNotes.classList.add("view");
}

function typeLabel(extension: string): string {
  return extension.toUpperCase().slice(0, 4);
}

async function refreshNotes(selectFileName?: string) {
  categories = await api.listCategories();
  if (!categories.includes(activeCategory)) activeCategory = categories[0] ?? "";
  notes = await api.listNotes(query ? null : activeCategory || null, query || null);
  renderCategories();
  renderNoteList();
  const wanted = selectFileName ?? loadedFileName;
  const match =
    notes.find((note) => note.category === activeCategory && note.fileName === wanted) ??
    (wanted ? notes.find((note) => note.fileName === wanted) : undefined);
  if (match) await showFile(match.category, match.fileName);
  else clearFile();
}

function renderCategories() {
  categoryList.replaceChildren();
  categoryCount.textContent = categories.length ? `${categories.length}` : "";
  if (categories.length === 0) {
    const empty = document.createElement("div");
    empty.className = "empty";
    empty.textContent = "还没有分类，在下方新建一个";
    categoryList.append(empty);
    return;
  }
  for (const category of categories) {
    const row = document.createElement("div");
    row.className =
      category === activeCategory && !query ? "cat-row active" : "cat-row";
    if (editingCategory === category) {
      row.classList.add("editing");
      row.append(categoryEditor(category));
      categoryList.append(row);
      continue;
    }
    const button = document.createElement("button");
    button.type = "button";
    button.className = "cat";
    button.textContent = category;
    button.addEventListener("click", () => {
      activeCategory = category;
      query = "";
      searchInput.value = "";
      loadedFileName = "";
      void refreshNotes();
    });
    const actions = document.createElement("div");
    actions.className = "cat-actions";
    const rename = document.createElement("button");
    rename.type = "button";
    rename.className = "btn quiet";
    rename.textContent = "改名";
    rename.setAttribute("aria-label", `重命名「${category}」`);
    rename.addEventListener("click", (event) => {
      event.stopPropagation();
      editingCategory = category;
      renderCategories();
    });
    const locate = document.createElement("button");
    locate.type = "button";
    locate.className = "btn quiet";
    locate.textContent = "位置";
    locate.setAttribute("aria-label", `打开「${category}」文件夹`);
    locate.addEventListener("click", (event) => {
      event.stopPropagation();
      void (async () => {
        try {
          await api.openCategoryDirectory(category);
        } catch (error) {
          showError(error);
        }
      })();
    });
    const remove = document.createElement("button");
    remove.type = "button";
    remove.className = "btn quiet danger-quiet";
    remove.textContent = "删除";
    remove.setAttribute("aria-label", `删除「${category}」`);
    remove.addEventListener("click", (event) => {
      event.stopPropagation();
      void removeCategory(category);
    });
    actions.append(rename, locate, remove);
    row.append(button, actions);
    categoryList.append(row);
  }
}

function categoryEditor(category: string): HTMLElement {
  const wrap = document.createElement("div");
  wrap.className = "cat-edit-wrap";
  const input = document.createElement("input");
  input.className = "cat-edit";
  input.value = category;
  input.maxLength = 60;
  input.setAttribute("aria-label", `分类「${category}」的新名称`);
  let done = false;
  const finish = (commit: boolean) => {
    if (done) return;
    done = true;
    const next = input.value.trim();
    editingCategory = "";
    if (!commit || !next || next === category) {
      renderCategories();
      return;
    }
    void (async () => {
      try {
        await api.renameCategory(category, next);
        if (activeCategory === category) activeCategory = next;
        await refreshNotes();
        showToast("已改名");
      } catch (error) {
        showError(error);
        renderCategories();
      }
    })();
  };
  input.addEventListener("keydown", (event) => {
    if (event.key === "Enter") {
      event.preventDefault();
      finish(true);
    } else if (event.key === "Escape") {
      event.preventDefault();
      finish(false);
    }
  });
  input.addEventListener("blur", () => finish(true));
  wrap.append(input);
  queueMicrotask(() => {
    input.focus();
    input.select();
  });
  return wrap;
}

async function removeCategory(category: string) {
  const ok = await confirmAction(`删除分类「${category}」和里面的全部文件？`);
  if (!ok) return;
  try {
    await api.deleteCategory(category);
    if (activeCategory === category) {
      activeCategory = "";
      loadedFileName = "";
    }
    editingCategory = "";
    await refreshNotes();
  } catch (error) {
    showError(error);
  }
}

function renderNoteList() {
  noteList.replaceChildren();
  if (notes.length === 0) {
    const empty = document.createElement("div");
    empty.className = "empty";
    if (query) {
      empty.textContent = "没有匹配的文件";
      noteList.append(empty);
    }
    return;
  }
  for (const note of notes) {
    const row = document.createElement("div");
    row.className = "file-row";
    const button = document.createElement("button");
    button.type = "button";
    button.className =
      note.category === activeCategory && note.fileName === loadedFileName ? "row active" : "row";
    const chip = document.createElement("span");
    chip.className = `type-chip ${note.extension}`;
    chip.setAttribute("aria-hidden", "true");
    chip.textContent = typeLabel(note.extension);
    const meta = document.createElement("span");
    meta.append(document.createTextNode(note.title));
    const small = document.createElement("small");
    small.textContent = query ? `${note.category} · ${note.extension.toUpperCase()}` : note.extension.toUpperCase();
    meta.append(small);
    button.append(chip, meta);
    button.addEventListener("click", () => {
      void openFile(note.category, note.fileName);
    });

    const actions = document.createElement("div");
    actions.className = "file-actions";
    const open = actionButton("打开", `用系统程序打开「${note.title}」`, () => {
      void openFile(note.category, note.fileName);
    });
    const reveal = actionButton("位置", `打开「${note.title}」所在位置`, () => {
      loadedFileName = note.fileName;
      activeCategory = note.category;
      renderNoteList();
      void api.revealFile(note.category, note.fileName).catch(showError);
    });
    const remove = actionButton("删除", `删除「${note.title}」`, () => {
      void removeNote(note.category, note.fileName);
    });
    remove.classList.add("danger-quiet");
    actions.append(open, reveal, remove);
    row.append(button, actions);
    noteList.append(row);
  }
  noteList.querySelector(".row.active")?.scrollIntoView({ block: "nearest" });
}

function actionButton(label: string, aria: string, onClick: () => void): HTMLButtonElement {
  const button = document.createElement("button");
  button.type = "button";
  button.className = "btn quiet";
  button.textContent = label;
  button.setAttribute("aria-label", aria);
  button.addEventListener("click", (event) => {
    event.stopPropagation();
    onClick();
  });
  return button;
}

function clearFile() {
  loadedFileName = "";
  fileEmpty.hidden = query.length > 0 || notes.length > 0;
}

async function showFile(category: string, fileName: string) {
  const note = await api.readNote(category, fileName);
  activeCategory = note.category;
  loadedFileName = note.fileName;
  fileEmpty.hidden = true;
  renderNoteList();
}

async function openFile(category: string, fileName: string) {
  try {
    activeCategory = category;
    loadedFileName = fileName;
    fileEmpty.hidden = true;
    renderNoteList();
    await api.openWithSystem(category, fileName);
  } catch (error) {
    showError(error);
  }
}

async function removeNote(category: string, fileName: string) {
  const ok = await confirmAction(`删除「${fileName}」？`);
  if (!ok) return;
  try {
    await api.deleteNote(category, fileName);
    if (loadedFileName === fileName && activeCategory === category) loadedFileName = "";
    await refreshNotes();
  } catch (error) {
    showError(error);
  }
}

navTodos.addEventListener("click", () => showView("todos"));
navNotes.addEventListener("click", () => {
  showView("notes");
  void refreshNotes();
});

function openDatePicker(input: HTMLInputElement) {
  if (typeof input.showPicker === "function") {
    try {
      input.showPicker();
      return;
    } catch {
      /* 已打开或当前环境不支持时，退回聚焦 */
    }
  }
  input.focus();
}

dateStartChip.addEventListener("click", () => openDatePicker(dateStart));
dateEndChip.addEventListener("click", () => openDatePicker(dateEnd));
datePrev.addEventListener("click", () => void setRange(addDays(rangeStart, -1), addDays(rangeEnd, -1)));
dateNext.addEventListener("click", () => void setRange(addDays(rangeStart, 1), addDays(rangeEnd, 1)));
dateToday.addEventListener("click", () => void setRange(todayISO(), todayISO()));
exportTodos.addEventListener("click", () => {
  void (async () => {
    try {
      const content = formatTodoExport(rangeStart, rangeEnd, dayBuckets);
      const saved = await api.saveExport(rangeStart, rangeEnd, content);
      if (saved) showToast("已导出");
    } catch (error) {
      showError(error);
    }
  })();
});
dateStart.addEventListener("change", () => void setRange(dateStart.value, rangeEnd));
dateEnd.addEventListener("change", () => void setRange(rangeStart, dateEnd.value));

bindTodoList(dayList);
bindTodoList(laterList);
bindAddForm(dayForm, dayInput, "day");
bindAddForm(laterForm, laterInput, "later");

categoryForm.addEventListener("submit", (event) => {
  event.preventDefault();
  const name = categoryInput.value.trim();
  if (!name) return;
  void (async () => {
    try {
      await api.createCategory(name);
      categoryInput.value = "";
      activeCategory = name;
      loadedFileName = "";
      await refreshNotes();
    } catch (error) {
      showError(error);
    }
  })();
});

importFilesBtn.addEventListener("click", () => {
  if (!activeCategory) {
    showError("先选择一个分类");
    return;
  }
  void (async () => {
    try {
      const imported = await api.pickAndImport(activeCategory);
      if (imported.length === 0) return;
      query = "";
      searchInput.value = "";
      await refreshNotes(imported[imported.length - 1].fileName);
      showToast(`已导入 ${imported.length} 个文件`);
    } catch (error) {
      showError(error);
    }
  })();
});

document.addEventListener("contextmenu", (event) => {
  event.preventDefault();
});

searchInput.addEventListener("input", () => {
  window.clearTimeout(searchTimer);
  searchTimer = window.setTimeout(() => {
    query = searchInput.value.trim();
    void refreshNotes();
  }, 160);
});

document.addEventListener("keydown", (event) => {
  if (viewNotes.hidden) return;
  const target = event.target as HTMLElement;
  if (target.matches("input, textarea")) return;
  if (event.key === "Delete" && loadedFileName) {
    void removeNote(activeCategory, loadedFileName);
    return;
  }
  if (event.key === "Enter" && loadedFileName) {
    event.preventDefault();
    void openFile(activeCategory, loadedFileName);
    return;
  }
  if (event.key !== "ArrowDown" && event.key !== "ArrowUp") return;
  if (notes.length === 0) return;
  event.preventDefault();
  const index = notes.findIndex(
    (note) => note.category === activeCategory && note.fileName === loadedFileName,
  );
  const nextIndex =
    event.key === "ArrowDown"
      ? Math.min(notes.length - 1, index < 0 ? 0 : index + 1)
      : Math.max(0, index <= 0 ? 0 : index - 1);
  const note = notes[nextIndex];
  void showFile(note.category, note.fileName);
});

async function bindWindowControls() {
  if (!("__TAURI_INTERNALS__" in window)) return;
  const { getCurrentWindow } = await import("@tauri-apps/api/window");
  const win = getCurrentWindow();
  const on = (id: string, action: () => Promise<void>) => {
    $(id).addEventListener("click", (event) => {
      event.preventDefault();
      event.stopPropagation();
      void action().catch((error) => showError(error));
    });
  };
  on("#win-min", () => win.minimize());
  on("#win-max", () => win.toggleMaximize());
  on("#win-close", () => win.close());
}

async function boot() {
  try {
    await bindWindowControls();
    laterItems = await api.loadLater();
    await setRange(todayISO(), todayISO());
    categories = await api.listCategories();
    activeCategory = categories[0] ?? "";
  } catch (error) {
    showError(error);
  }
}

void boot();

