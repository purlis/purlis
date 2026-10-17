# Amendment, 2026-10-17: a theme names its schema by an address purlis never fetches

**Decided by the dispatcher on 2026-10-17** for #2322 (spec #2313, ADR 0097 §3.1).

§9 holds that only the updater and `purlis report` name purlis's own address, so a run without
an account reaches purlis only through them. Format 2's published JSON Schema has an address
of its own: `theme::SCHEMA_URL`, the raw file `docs/schema/theme.schema.json` in purlis's
repository, which the schema's `$id` and a theme's `$schema` name.

- **purlis never fetches it.** The theme engine reads a theme against its own vocabulary, in
  process; the schema is generated from that vocabulary and committed, and nothing in purlis
  requests it. The address is a name for an editor, which fetches it when a person opens a
  `theme.json`.
- **One constant, one line.** The §9 guard allows `crates/purlis-core/src/theme/schema.rs` that
  one line and nothing else, with this record as its reason.
