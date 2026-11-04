// Local-time date ranges for reports. A range is [from, to): `to` is the
// first moment *after* the range. Sales are stored as RFC 3339 UTC
// ("2026-11-04T15:00:00Z") and expenses as local dates ("2026-11-04"), so
// each gets its own conversion.

export type Preset =
  | "today" | "yesterday" | "thisWeek" | "lastWeek" | "thisMonth" | "lastMonth"
  | "last3Months" | "thisYear" | "custom";

export interface Range {
  from: Date;
  to: Date;
}

// Iraqi work weeks start on Saturday (same as the schedule)
const WEEK_START_DAY = 6;
const day = (d: Date, add = 0) => new Date(d.getFullYear(), d.getMonth(), d.getDate() + add);
const weekStart = (d: Date) => day(d, -((d.getDay() - WEEK_START_DAY + 7) % 7));

export const presetRange = (preset: Exclude<Preset, "custom">, now = new Date()): Range => {
  const today = day(now);
  switch (preset) {
    case "today":
      return { from: today, to: day(today, 1) };
    case "yesterday":
      return { from: day(today, -1), to: today };
    case "thisWeek":
      return { from: weekStart(today), to: day(today, 1) };
    case "lastWeek": {
      const start = day(weekStart(today), -7);
      return { from: start, to: day(start, 7) };
    }
    case "thisMonth":
      return { from: new Date(now.getFullYear(), now.getMonth(), 1), to: day(today, 1) };
    case "lastMonth":
      return {
        from: new Date(now.getFullYear(), now.getMonth() - 1, 1),
        to: new Date(now.getFullYear(), now.getMonth(), 1),
      };
    case "last3Months":
      return { from: new Date(now.getFullYear(), now.getMonth() - 2, 1), to: day(today, 1) };
    case "thisYear":
      return { from: new Date(now.getFullYear(), 0, 1), to: day(today, 1) };
  }
};

/** The same-length range just before. */
export const previousRange = ({ from, to }: Range): Range => {
  const len = to.getTime() - from.getTime();
  return { from: new Date(from.getTime() - len), to: from };
};

const pad = (n: number) => String(n).padStart(2, "0");

/** "2026-11-04" in local time (for expenses and <input type=date>). */
export const toLocalDate = (d: Date) => `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;

/** "2026-11-04T21:00:00Z": the stored format, so text comparison works. */
export const toUtc = (d: Date) => d.toISOString().replace(/\.\d{3}Z$/, "Z");

export const fromLocalDate = (s: string) => {
  const [y, m, d] = s.split("-").map(Number);
  return new Date(y, m - 1, d);
};

export const rangeDays = ({ from, to }: Range) => Math.round((to.getTime() - from.getTime()) / 86_400_000);
