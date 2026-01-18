function clampNumber(value: number, min: number, max: number) {
  return Math.max(min, Math.min(max, value));
}

export function dayKeyFromLocalDate(d: Date) {
  const year = d.getFullYear();
  const month = String(d.getMonth() + 1).padStart(2, "0");
  const day = String(d.getDate()).padStart(2, "0");
  return `${year}-${month}-${day}`;
}

export function buildRecentDayKeys(days: number) {
  const n = clampNumber(Math.floor(days), 1, 60);
  const out: string[] = [];
  for (let delta = n - 1; delta >= 0; delta -= 1) {
    const d = new Date();
    d.setDate(d.getDate() - delta);
    out.push(dayKeyFromLocalDate(d));
  }
  return out;
}
