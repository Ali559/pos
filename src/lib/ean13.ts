// EAN-13 barcodes for shelf labels: the 13-digit codes on most packaged
// goods (and the in-store 2x… codes). Anything else falls back to Code 128.
import { code128Bars } from "./code128";

const L = ["0001101", "0011001", "0010011", "0111101", "0100011", "0110001", "0101111", "0111011", "0110111", "0001011"];
const G = ["0100111", "0110011", "0011011", "0100001", "0011101", "0111001", "0000101", "0010001", "0001001", "0010111"];
const R = ["1110010", "1100110", "1101100", "1000010", "1011100", "1001110", "1010000", "1000100", "1001000", "1110100"];
// Which of L/G each of the left six digits uses, chosen by the first digit
const PARITY = ["LLLLLL", "LLGLGG", "LLGGLG", "LLGGGL", "LGLLGG", "LGGLLG", "LGGGLL", "LGLGLG", "LGLGGL", "LGGLGL"];

export const ean13Check = (first12: string) => {
  const sum = [...first12].reduce((n, d, i) => n + Number(d) * (i % 2 === 0 ? 1 : 3), 0);
  return String((10 - (sum % 10)) % 10);
};

/** The full 13 digits for a 12- or 13-digit code with a valid check digit, else null */
export const asEan13 = (code: string) => {
  if (/^\d{12}$/.test(code)) return code + ean13Check(code);
  if (/^\d{13}$/.test(code) && ean13Check(code.slice(0, 12)) === code[12]) return code;
  return null;
};

/** The 95 modules of an EAN-13, as "1"/"0" */
export const ean13Modules = (code13: string) => {
  const d = [...code13].map(Number);
  const parity = PARITY[d[0]!]!;
  let m = "101";
  for (let i = 1; i <= 6; i++) m += (parity[i - 1] === "L" ? L : G)[d[i]!]!;
  m += "01010";
  for (let i = 7; i <= 12; i++) m += R[d[i]!]!;
  return m + "101";
};

/** Bars as [x, width] in modules plus total width, for any code: EAN-13
 *  when it is one, else Code 128 */
export const barcodeBars = (code: string) => {
  const ean = asEan13(code);
  if (!ean) return { ...code128Bars(code), text: code, ean: false };
  const modules = ean13Modules(ean);
  const bars: [number, number][] = [];
  for (let x = 0; x < modules.length; x++) {
    if (modules[x] !== "1") continue;
    let w = 1;
    while (modules[x + w] === "1") w++;
    bars.push([x, w]);
    x += w - 1;
  }
  return { bars, width: modules.length, text: ean, ean: true };
};
