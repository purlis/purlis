// The terminal mirror (iOS design §6.1): xterm at the desktop's size, drawn with WebGL where
// the phone has it, and every chunk acknowledged once the terminal has finished writing it —
// the host's window follows the screen, not the link (what the spike measured).

import { Terminal } from "@xterm/xterm";
import { WebglAddon } from "@xterm/addon-webgl";

import { theme, xtermTheme } from "./theme.ts";

/** What draws bytes and says when it has: xterm, or a test's double. */
export type Drawer = {
  write(bytes: Uint8Array, drawn: () => void): void;
};

export function fromB64(b64: string): Uint8Array {
  const bin = atob(b64);
  const out = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i);
  return out;
}

/** Hand one chunk to the terminal; `ack` hears its length exactly once, after it is drawn. */
export function feed(drawer: Drawer, b64: string, ack: (bytes: number) => void): void {
  const bytes = fromB64(b64);
  let once = false;
  drawer.write(bytes, () => {
    if (once) return;
    once = true;
    ack(bytes.length);
  });
}

/** The mirror as the Chat screen drives it: what it draws, what is typed into it. */
export type Mirror = Drawer & {
  renderer: "webgl" | "dom";
  /** Typed text, as xterm hands it over. */
  onData(handler: (data: string) => void): void;
  /** Whether typing into the mirror does anything: off while the desktop is away. */
  stdin(enabled: boolean): void;
  dispose(): void;
};

export type Opener = (holder: HTMLElement, cols: number, rows: number) => Mirror;

/** A terminal at `cols × rows` in `holder`, the theme's colours, WebGL when it takes. */
export const openTerminal: Opener = (holder, cols, rows) => {
  const term = new Terminal({
    cols,
    rows,
    fontSize: 11,
    scrollback: 2000,
    theme: xtermTheme(theme()),
  });
  term.open(holder);
  let renderer: "webgl" | "dom" = "dom";
  let webgl: WebglAddon | undefined;
  try {
    webgl = new WebglAddon();
    webgl.onContextLoss(() => {
      webgl?.dispose();
      webgl = undefined;
    });
    term.loadAddon(webgl);
    renderer = "webgl";
  } catch {
    webgl = undefined;
  }
  return {
    renderer,
    write(bytes, drawn) {
      term.write(bytes, drawn);
    },
    onData(handler) {
      term.onData(handler);
    },
    stdin(enabled) {
      term.options.disableStdin = !enabled;
    },
    dispose() {
      webgl?.dispose();
      term.dispose();
    },
  };
};
