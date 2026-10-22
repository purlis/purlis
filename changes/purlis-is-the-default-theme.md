### Changed

- **Purlis is the default theme.** A new install, and any project, workspace or machine where
  nobody chose a theme, now draws Purlis: the lavender dark from the designer's foundation, in
  IBM Plex Sans and JetBrains Mono with Lucide's icons. When your system asks for more contrast,
  it draws Purlis's high-contrast dark. A dark, light or "Follow the system" pick now draws
  Purlis too: `purlis-dark` (and `charter-dark`) draw Purlis's dark, `purlis-light` (and
  `charter-light`) its light, and `system` follows the system with your own `theme.json`, or
  else with Purlis. No file is rewritten. Settings › Theme no longer offers `purlis-dark` and
  `purlis-light`, and labels a file that still holds one. To keep the green look purlis had
  until now, choose **Purlis Classic** (`purlis-classic`). Purlis is no longer marked Preview,
  and a new theme draft extends Purlis (#3034).
- **A red, orange or pink workspace keeps its colour on its strip.** A workspace's colour now
  dims only the accent and the marks beside the danger colour, which must stay apart from it.
  The strip and every other token it tints keep the colour's full hue in every theme (#3034).
