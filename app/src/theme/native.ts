// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { commands } from "../bindings";
import type { Appearance, Theme } from "./theme";

/**
 * **The native window follows the theme the window draws** (#2328, spec #2313): the operating
 * system's own parts of the window — the traffic lights, the menus, the dialogs — take the drawn
 * appearance's tag, and the window's own colour is the theme's `window.background`.
 *
 * The window is created in both already, for the theme its first frame draws
 * (`src-tauri/src/nativewindow.rs`), so nothing of another colour shows before that frame. This
 * is the other half: every theme drawn afterwards — a project's pick, the person's own theme
 * for more contrast, an extension's — is handed over as it is drawn, and the core applies it. A
 * theme that follows the operating system leaves the native parts to the system, so
 * `prefers-color-scheme` goes on answering the system's appearance rather than the one purlis
 * set.
 *
 * On macOS and Linux the platform keeps one appearance for the whole app, so a window says it
 * again when it takes focus: the native parts are the focused window's.
 */

/** What the native window is drawn in for `theme`. */
export type NativeWindow = {
  appearance: Appearance;
  /** The theme's `window.background`. */
  background: string;
  /** Whether the theme follows the operating system: its native parts are then the system's. */
  followsSystem: boolean;
};

/** **What the native window is drawn in while `theme` is drawn.** */
export function nativeWindow(theme: Theme, followsSystem = false): NativeWindow {
  return {
    appearance: theme.appearance,
    background: theme.values["window.background"],
    followsSystem,
  };
}

/** What the window was last told, so a theme drawn again tells it nothing. */
let told: string | undefined;
/** What the window was last asked to be drawn in, for when it takes focus again. */
let last: NativeWindow | undefined;
/** Whether focus is listened for. One listener for the life of the window. */
let listening = false;

/**
 * **Draws the native window as `theme`**: tells the core once per change, and never throws — a
 * window outside the app, or a core that cannot draw it, keeps its native parts as they are.
 */
export function drawNatively(theme: Theme, followsSystem = false): void {
  tell(nativeWindow(theme, followsSystem));
  if (!listening && typeof window !== "undefined") {
    listening = true;
    window.addEventListener("focus", () => {
      if (last === undefined) return;
      told = undefined;
      tell(last);
    });
  }
}

function tell(native: NativeWindow): void {
  last = native;
  const key = `${native.appearance} ${native.background} ${native.followsSystem}`;
  if (key === told) return;
  told = key;
  try {
    void commands
      .drawNativeWindow(native.appearance, native.background, native.followsSystem)
      .catch(() => undefined);
  } catch {
    // No core to ask: nothing to draw natively.
  }
}

/** For tests: forget what the window was told. */
export function forgetNativeWindow(): void {
  told = undefined;
  last = undefined;
}
