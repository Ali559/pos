// Settings that belong to this computer, not the shop (src-tauri/src/device.rs).
// On an extra till they stay on the till; everything else is on the main
// computer.
import { call } from "./ipc";

export const DEVICE_KEYS = ["receipt_printer", "receipt_width", "auto_print", "fullscreen"];

export const localSetting = (key: string) => call<string | null>("get_device_setting", { key });

export const setLocalSetting = (key: string, value: string) => call("set_device_setting", { key, value });
