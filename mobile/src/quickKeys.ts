// The quick-key bar under the terminal (iOS design §6.1): what each key sends, as bytes, by
// raw `write`. The same table the spike measured with.

export type QuickKey = { label: string; bytes: number[] };

export const QUICK_KEYS: readonly QuickKey[] = [
  { label: "Esc", bytes: [0x1b] },
  { label: "↑", bytes: [0x1b, 0x5b, 0x41] },
  { label: "↓", bytes: [0x1b, 0x5b, 0x42] },
  { label: "Enter", bytes: [0x0d] },
  { label: "Tab", bytes: [0x09] },
  { label: "y", bytes: [0x79] },
  { label: "n", bytes: [0x6e] },
  { label: "Ctrl-C", bytes: [0x03] },
];

/** Typed text as the terminal's bytes: UTF-8, as xterm hands it over. */
export function bytesOf(text: string): number[] {
  return Array.from(new TextEncoder().encode(text));
}
