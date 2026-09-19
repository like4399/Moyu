export function uniqueTitle(existing: string[], base = "未命名"): string {
  if (!existing.includes(base)) return base;
  let index = 2;
  while (existing.includes(`${base} ${index}`)) index += 1;
  return `${base} ${index}`;
}
