# Amendment, 2026-10-17: the theme schema's address names its format and its release

**Decided by the implementer on 2026-10-17** for #2448 (spec #2313, ADR 0097 §3.1), refining
the amendment of the same day, [a theme names its schema by an address purlis never
fetches](2026-10-17-a-theme-names-its-schema-by-an-address-purlis-never-fetches.md).

The address is now `theme::SCHEMA_URL` in `crates/purlis-core/src/theme/published.rs`: the
raw file `docs/schema/theme.v2.schema.json` at the release's tag,
`https://raw.githubusercontent.com/purlis/purlis/v<version>/docs/schema/theme.v2.schema.json`,
with `<version>` the crate's own. A theme written for one purlis names the schema that purlis
reads, and every release's schema stays at its address.

- **purlis still never fetches it.** Nothing changes in what the earlier amendment holds: the
  engine reads a theme against its own vocabulary, in process, and only an editor requests the
  address.
- **Still one constant, one line.** The §9 guard's one allowance for it moves to
  `theme/published.rs` and to the line naming the tag, and allows nothing else there.
- **The schema file carries no `$id`.** The file is the format's, the same in every release
  until the format grows; the address is the release's. An editor takes the address it fetched
  as the schema's base.
