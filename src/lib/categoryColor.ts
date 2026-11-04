// Category colors are stored as one hex per category (`categories.color`).
// Tile backgrounds and text are mixed from it against the theme tokens, so
// the same stored color works in light and dark mode.
const FALLBACK = "var(--muted-foreground)";

/** Soft tile background. */
export const categoryTint = (color: string | null) =>
  `color-mix(in oklab, ${color || FALLBACK} 16%, var(--card))`;

/** Readable text/dot color on that tint. */
export const categoryInk = (color: string | null) =>
  `color-mix(in oklab, ${color || FALLBACK} 75%, var(--foreground))`;
