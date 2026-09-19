import type { TodoItem } from "./types";

export function formatTodoExport(
  start: string,
  end: string,
  days: { date: string; items: TodoItem[] }[],
): string {
  const title = start === end ? start : `${start} 至 ${end}`;
  const filled = days.filter((day) => day.items.length > 0);
  if (filled.length === 0) return `# ${title}\n\n这一段没有待办。\n`;
  const lines = [`# ${title}`, ""];
  for (const day of filled) {
    lines.push(`## ${day.date}`, "");
    for (const item of day.items) {
      lines.push(`- [${item.done ? "x" : " "}] ${item.text}`);
    }
    lines.push("");
  }
  return `${lines.join("\n").trimEnd()}\n`;
}
