import { formatDateTime } from "@/lib/locale";

export const getTimeofDay = (): string => formatDateTime(new Date());
