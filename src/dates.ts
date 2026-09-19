export function formatISO(date: Date): string {
  const year = date.getFullYear();
  const month = String(date.getMonth() + 1).padStart(2, "0");
  const day = String(date.getDate()).padStart(2, "0");
  return `${year}-${month}-${day}`;
}

export function todayISO(now = new Date()): string {
  return formatISO(now);
}

export function isValidISODate(value: string): boolean {
  if (!/^\d{4}-\d{2}-\d{2}$/.test(value)) return false;
  const [year, month, day] = value.split("-").map(Number);
  const date = new Date(year, month - 1, day);
  return date.getFullYear() === year && date.getMonth() === month - 1 && date.getDate() === day;
}

export function addDays(iso: string, delta: number): string {
  const [year, month, day] = iso.split("-").map(Number);
  const date = new Date(year, month - 1, day);
  date.setDate(date.getDate() + delta);
  return formatISO(date);
}

export function dateLabel(iso: string, today = todayISO()): string {
  if (iso === today) return "今天";
  if (iso === addDays(today, -1)) return "昨天";
  if (iso === addDays(today, 1)) return "明天";
  return iso;
}

export const MAX_RANGE_DAYS = 62;

export function eachDay(start: string, end: string): string[] {
  if (!isValidISODate(start) || !isValidISODate(end) || start > end) return [];
  const days: string[] = [];
  let cursor = start;
  while (cursor <= end && days.length < 400) {
    days.push(cursor);
    cursor = addDays(cursor, 1);
  }
  return days;
}

export function normalizeRange(start: string, end: string): { start: string; end: string; clamped: boolean } {
  if (!isValidISODate(start) || !isValidISODate(end)) {
    return { start, end, clamped: false };
  }
  let from = start;
  let to = end;
  if (from > to) [from, to] = [to, from];
  const days = eachDay(from, to);
  if (days.length <= MAX_RANGE_DAYS) return { start: from, end: to, clamped: false };
  return { start: from, end: addDays(from, MAX_RANGE_DAYS - 1), clamped: true };
}

export function prettyDate(iso: string, today = todayISO()): string {
  const near = dateLabel(iso, today);
  if (near !== iso) return near;
  const [year, month, day] = iso.split("-").map(Number);
  if (String(year) === today.slice(0, 4)) return `${month}月${day}日`;
  return `${year}年${month}月${day}日`;
}

export function rangeTitle(start: string, end: string, today = todayISO()): string {
  if (start === end) return dateLabel(start, today);
  return `${prettyDate(start, today)} – ${prettyDate(end, today)}`;
}
