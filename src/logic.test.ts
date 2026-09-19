import { addDays, dateLabel, eachDay, isValidISODate, normalizeRange, prettyDate, rangeTitle, todayISO } from "./dates";
import { formatTodoExport } from "./export";
import { describe, expect, it } from "vitest";
import { uniqueTitle } from "./names";

describe("dates", () => {
  it("formats local dates without utc shift", () => {
    expect(todayISO(new Date(2026, 8, 19, 23, 30))).toBe("2026-09-19");
  });

  it("rejects impossible dates and walks month boundaries", () => {
    expect(isValidISODate("2026-02-29")).toBe(false);
    expect(isValidISODate("2024-02-29")).toBe(true);
    expect(addDays("2026-01-31", 1)).toBe("2026-02-01");
    expect(addDays("2024-02-28", 1)).toBe("2024-02-29");
    expect(addDays("2026-03-01", -1)).toBe("2026-02-28");
  });

  it("labels nearby days", () => {
    expect(dateLabel("2026-09-19", "2026-09-19")).toBe("今天");
    expect(dateLabel("2026-09-18", "2026-09-19")).toBe("昨天");
    expect(dateLabel("2026-09-20", "2026-09-19")).toBe("明天");
    expect(dateLabel("2026-09-01", "2026-09-19")).toBe("2026-09-01");
  });

  it("walks and clamps a date span", () => {
    expect(eachDay("2026-09-18", "2026-09-20")).toEqual(["2026-09-18", "2026-09-19", "2026-09-20"]);
    expect(eachDay("2026-09-20", "2026-09-18")).toEqual([]);
    expect(normalizeRange("2026-09-20", "2026-09-18")).toEqual({
      start: "2026-09-18",
      end: "2026-09-20",
      clamped: false,
    });
    const long = normalizeRange("2026-01-01", "2026-12-31");
    expect(long.clamped).toBe(true);
    expect(eachDay(long.start, long.end)).toHaveLength(62);
    expect(prettyDate("2026-09-01", "2026-09-19")).toBe("9月1日");
    expect(rangeTitle("2026-09-18", "2026-09-19", "2026-09-19")).toBe("昨天 – 今天");
  });

  it("exports a date span as markdown and skips empty days", () => {
    const text = formatTodoExport("2026-09-01", "2026-09-03", [
      { date: "2026-09-01", items: [{ text: "写周报", done: false }] },
      { date: "2026-09-02", items: [] },
      { date: "2026-09-03", items: [{ text: "已核对", done: true }] },
    ]);
    expect(text).toContain("# 2026-09-01 至 2026-09-03");
    expect(text).toContain("## 2026-09-01\n\n- [ ] 写周报");
    expect(text).not.toContain("2026-09-02");
    expect(text).toContain("- [x] 已核对");
    expect(formatTodoExport("2026-09-01", "2026-09-01", [{ date: "2026-09-01", items: [] }])).toBe(
      "# 2026-09-01\n\n这一段没有待办。\n",
    );
  });
});

describe("uniqueTitle", () => {
  it("picks the next free name", () => {
    expect(uniqueTitle([])).toBe("未命名");
    expect(uniqueTitle(["未命名"])).toBe("未命名 2");
    expect(uniqueTitle(["未命名", "未命名 2"])).toBe("未命名 3");
  });
});
