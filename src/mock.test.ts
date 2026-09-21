import { beforeEach, describe, expect, it } from "vitest";
import * as mock from "./mock";

describe("mock api", () => {
  beforeEach(() => {
    localStorage.clear();
  });

  it("roundtrips day todos and sinks completed items", async () => {
    await mock.saveDay("2026-09-19", [
      { text: " 写周报 ", done: false },
      { text: "", done: false },
      { text: "已完成", done: true },
    ]);
    expect(await mock.loadDay("2026-09-19")).toEqual([
      { text: "写周报", done: false },
      { text: "已完成", done: true },
    ]);
    await mock.saveDay("2026-09-20", [
      { text: "已做完", done: true },
      { text: "还没做", done: false },
    ]);
    expect(await mock.loadDay("2026-09-20")).toEqual([
      { text: "还没做", done: false },
      { text: "已做完", done: true },
    ]);
  });

  it("moves unfinished todos from past days onto today", async () => {
    await mock.saveDay("2026-09-18", [
      { text: "没做完", done: false },
      { text: "做完了", done: true },
    ]);
    await mock.saveDay("2026-09-19", [{ text: "今天", done: false }]);
    expect(await mock.carryUnfinished("2026-09-19")).toBe(1);
    expect(await mock.loadDay("2026-09-18")).toEqual([{ text: "做完了", done: true }]);
    expect(await mock.loadDay("2026-09-19")).toEqual([
      { text: "没做完", done: false },
      { text: "今天", done: false },
    ]);
  });

  it("creates categories, notes and imports files", async () => {
    expect(await mock.listCategories()).toEqual([]);
    await mock.createCategory("临时");
    await mock.renameCategory("临时", "归档");
    expect(await mock.listCategories()).toEqual(["归档"]);
    const saved = await mock.saveNote({
      category: "归档",
      title: "草稿",
      body: "hello",
      previousFileName: null,
    });
    expect(saved.fileName).toBe("草稿.md");
    const renamed = await mock.saveNote({
      category: "归档",
      title: "正式",
      body: "world",
      previousFileName: "草稿.md",
    });
    expect(renamed.fileName).toBe("正式.md");
    await expect(
      mock.saveNote({
        category: "归档",
        title: "正式",
        body: "冲突",
        previousFileName: null,
      }),
    ).rejects.toThrow(/同名/);
    const imported = await mock.importFile("归档", "C:/a/手册.xlsx");
    expect(imported.extension).toBe("xlsx");
    expect(imported.kind).toBe("file");
    const hits = await mock.listNotes(null, "手册");
    expect(hits.some((item) => item.fileName.startsWith("手册"))).toBe(true);
  });

  it("reorders categories and files", async () => {
    await mock.createCategory("甲");
    await mock.createCategory("乙");
    await mock.reorderCategories(["乙", "甲"]);
    expect(await mock.listCategories()).toEqual(["乙", "甲"]);
    await mock.saveNote({ category: "乙", title: "后", body: "", previousFileName: null });
    await mock.saveNote({ category: "乙", title: "先", body: "", previousFileName: null });
    await mock.reorderFiles("乙", ["先.md", "后.md"]);
    expect((await mock.listNotes("乙", null)).map((note) => note.fileName)).toEqual(["先.md", "后.md"]);
  });
});
