import { getTimeofDay } from "@/utils/dateUtils";
import { ref } from "vue";

export const useTimeManager = () => {
  // ? State
  const timeOfDay = ref<string>();

  setInterval(() => (timeOfDay.value = getTimeofDay()), 1000);

  return {
    timeOfDay,
  };
};
