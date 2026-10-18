### Changed

- **A theme names the schema of the purlis it is for.** The theme schema is published per format
  and release, at `https://raw.githubusercontent.com/purlis/purlis/v<version>/docs/schema/theme.v2.schema.json`,
  so an editor completes only the tokens that purlis knows. Within a format the schema only
  grows. Purlis Classic's folder names its release's schema, and `purlis theme list --json` and
  `purlis theme check --json` give the address as `theme_schema` (#2448).
