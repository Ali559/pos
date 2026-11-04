import { getTimeofDay } from "@/utils/dateUtils";
import { onUnmounted, ref } from "vue";

export const useTimeManager = () => {
  // ? State
  const timeOfDay = ref<string>(getTimeofDay());

  const timer = setInterval(() => (timeOfDay.value = getTimeofDay()), 1000);
  onUnmounted(() => clearInterval(timer));

  return {
    timeOfDay,
  };
};
