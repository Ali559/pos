import { i18n } from "@/utils/i18n";

/** The shape Rust commands reject with (src-tauri/src/error.rs). */
export interface AppError {
  code: string;
  message: string;
  detail?: string;
}

export const isAppError = (e: unknown): e is AppError =>
  typeof e === "object" && e !== null && "code" in e && "message" in e;

/** A translated, user-facing message for any thrown value. */
export const errorMessage = (e: unknown): string => {
  const { t, te } = i18n.global;
  if (isAppError(e)) {
    if (e.code === "forbidden" && e.detail) {
      return t("errors.forbidden", { permission: t(`permissions.${e.detail}`) });
    }
    // `detail` names what the rule is about, e.g. which product is short
    // (isolated so an English name reads correctly inside RTL text)
    if (te(`errors.${e.code}`)) return t(`errors.${e.code}`, { detail: `\u2068${e.detail ?? ""}\u2069` });
    return e.message;
  }
  if (e instanceof Error) return e.message;
  return String(e);
};
