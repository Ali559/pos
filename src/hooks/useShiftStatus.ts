import { computed, onUnmounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useAuth } from "@/stores/auth";
import { formatTime } from "@/lib/locale";

const WARN_MINUTES = 15;

/** Top-bar shift text: when the scheduled shift ends, or time on shift. */
export const useShiftStatus = () => {
  const { t } = useI18n();
  const { session } = useAuth();
  const now = ref(Date.now());
  const timer = setInterval(() => (now.value = Date.now()), 30_000);
  onUnmounted(() => clearInterval(timer));

  return computed(() => {
    const shift = session.value?.shift;
    if (!shift) return null;
    if (shift.scheduledEnd) {
      const end = new Date(shift.scheduledEnd);
      const minutesLeft = (end.getTime() - now.value) / 60_000;
      return {
        text:
          minutesLeft < 0
            ? t("shift.endedAt", { time: formatTime(end) })
            : t("shift.endsAt", { time: formatTime(end) }),
        tone: minutesLeft < WARN_MINUTES ? ("warn" as const) : ("normal" as const),
      };
    }
    const mins = Math.max(0, Math.floor((now.value - Date.parse(shift.clockedInAt)) / 60_000));
    return {
      text: t("shift.onShift", { h: Math.floor(mins / 60), m: mins % 60 }),
      tone: "normal" as const,
    };
  });
};
