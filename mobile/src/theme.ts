// The phone's colours are the desktop's (`app/src/theme`): the same two themes, drawn by the
// same code, following the system's appearance. Nothing here names a colour.

import {
  PREFERS_LIGHT,
  drawIn,
  inForce,
  systemTheme,
  xtermTheme,
  type Theme,
} from "../../app/src/theme/theme";

/** Draw the theme the system's appearance asks for, now and whenever it changes. */
export function drawTheme(): void {
  drawIn(systemTheme());
  const media = window.matchMedia(PREFERS_LIGHT);
  media.addEventListener("change", () => drawIn(systemTheme()));
}

/** The theme in force, for a terminal opening now. */
export function theme(): Theme {
  return inForce();
}

export { xtermTheme };
