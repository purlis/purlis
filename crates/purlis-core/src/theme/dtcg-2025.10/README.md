# The design-tokens format 2025.10, vendored

What purlis's design-tokens export is held to (#2926). Test data only: nothing here ships.

| File | What it is | Where it came from |
| --- | --- | --- |
| `format.json` | The Design Tokens Community Group's bundled JSON Schema of the Format, version 2025.10 (draft-07) | `https://www.designtokens.org/schemas/2025.10/format.json`, fetched 2026-10-21, sha-256 `32e93b780e4e4bca778d0780cb797a560deedc470c608af16576223f7e42915f` |
| `test-suite/` | The format's official conformance fixtures (`manifest.json`, `positive/`, `negative/`), version 2025.10 | `test-suite/tests/2025.10/format/` of `design-tokens/community-group` at commit `882ebd6716abef9a46d8ef5fb12e82009d6cee2e` |
| `LICENSE.md` | The community group's licence statement | the same commit |
| `LICENSE-W3C-BSD` | The licence the schema and the test suite are contributed under | `https://www.w3.org/Consortium/Legal/2008/03-bsd-license.html` |

The bundle is built upstream from `schemas/src/2025.10/` at that commit. Each of its schemas is
the source file of its `$id`, except that the upstream bundler drops the descriptions written
beside a `$ref`, which draft-07 does not read.

Neither is changed here: `format.json` is byte for byte the file at the address above, as its
sha-256 shows, and the test suite is the commit's. To move to a newer version, vendor its schema and its test suite in a folder of
their own and point the tests at it: `theme/design_tokens_tests.rs` checks every fixture of the
suite gets the verdict its manifest gives before it trusts an export's.
