### Added

- **Harnesses have icons** (#2922). Claude Code, opencode and Codex are drawn with glyphs
  purlis draws (a spark, braces and a terminal), never a vendor's mark, beside a profile's name
  in Settings, and on its card in Start a chat once that card draws it. A profile can name its
  icon in `purlis.local.toml`: `icon = "spark"` for a glyph, or `icon = "mine.svg"` for an SVG of
  your own in purlis's `icons` folder in your config home. A profile with no icon shows its
  harness's monogram. The glyphs are part of the theme's icon set, so an icon pack can redraw
  them under `glyphs`.
