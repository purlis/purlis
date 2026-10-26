// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// The parts manifest's release step: one version per release (ADR 0103 amended 2026-10-25).
// Run with `node --test tools/`.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdirSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

import {
  PARTS,
  files,
  literals,
  literalsIn,
  markers,
  mode,
  pending,
  release,
  replace,
  versionIn,
} from "./theme-parts-release.mjs";

const SCRIPT = fileURLToPath(new URL("./theme-parts-release.mjs", import.meta.url));
const REPO = fileURLToPath(new URL("..", import.meta.url));

const PARTS_RS = `/// The last release's: what is added is \`since: NEXT\` until [\`NEXT\`] is replaced.
pub const VERSION: u32 = 12;

/// The marker.
pub const NEXT: u32 = VERSION + 1;

fn said() -> String {
    format!("plus unreleased additions marked since {NEXT}")
}

pub const MANIFEST: &[Family] = &[
    Family { name: "button", since: 1 },
    Family { name: "chip", since: NEXT }, // since: NEXT
    /* a NEXT in a block /* nested NEXT */ comment */
    Family { name: r#"NEXT"#, since: 3 },
];

impl Prop {
    fn since(self) -> u32 {
        match self {
            Self::Brand => NEXT,
            _ => 1,
        }
    }
}

fn lifetimes<'a>(one: &'a str) -> char { let _ = one; 'N' }
`;

const FAMILY_RS = `use super::{Family, Part};

pub(super) const CHIP: Family = Family {
    since: super::NEXT,
    parts: &[Part { name: "root", since: super::NEXT, description: "Not NEXT." }],
};
`;

const TEST_RS = `#[test]
fn the_chip_is_unreleased() {
    assert_eq!(family["since"], parts::NEXT, "the chip is since NEXT");
    assert_eq!(other["since"], purlis_core::theme::parts::NEXT);
}
`;

function tree(files) {
  const root = mkdtempSync(join(tmpdir(), "parts-release-"));
  for (const [path, text] of Object.entries(files)) {
    mkdirSync(join(root, path, ".."), { recursive: true });
    writeFileSync(join(root, path), text);
  }
  return root;
}

test("a NEXT is found only where it is code, and never its own definition", () => {
  const lines = markers(PARTS_RS).map((one) => one.line);
  // `since: NEXT` in the chip, and `=> NEXT` in a match: not the doc comment, the definition, the
  // format string, the line or block comment, the raw string or the char literal.
  assert.deepEqual(lines, [13, 21]);
  assert.deepEqual(
    markers(FAMILY_RS).map((one) => one.line),
    [4, 5],
  );
  assert.deepEqual(
    markers(TEST_RS).map((one) => one.line),
    [3, 4],
  );
});

test("the rewrite replaces each NEXT, path and all, with the version", () => {
  const out = replace(FAMILY_RS, 13);
  assert.match(
    out,
    /since: 13,\n {4}parts: &\[Part \{ name: "root", since: 13, description: "Not NEXT\." \}\]/,
  );
  assert.doesNotMatch(out, /super::NEXT/);
  const test = replace(TEST_RS, 13);
  assert.match(test, /assert_eq!\(family\["since"\], 13, "the chip is since NEXT"\);/);
  assert.match(test, /assert_eq!\(other\["since"\], 13\);/);
  const parts = replace(PARTS_RS, 13);
  assert.match(parts, /pub const NEXT: u32 = VERSION \+ 1;/);
  assert.match(parts, /since \{NEXT\}/);
  assert.match(parts, /Self::Brand => 13,/);
  assert.deepEqual(markers(parts), []);
});

test("the release step raises the version once and leaves no NEXT behind", () => {
  const root = tree({
    [PARTS]: PARTS_RS,
    "crates/purlis-core/src/theme/parts_chip.rs": FAMILY_RS,
    "crates/purlis-cli/tests/theme.rs": TEST_RS,
    "crates/purlis-core/src/elsewhere.rs": "static NEXT: u32 = 0;\n",
  });
  assert.equal(pending(root).length, 6);
  assert.equal(release(root), 13);
  assert.deepEqual(pending(root), []);
  assert.equal(versionIn(readFileSync(join(root, PARTS), "utf8")), 13);
  // Outside the theme engine and the CLI's tests, a NEXT is something else and is left alone.
  assert.equal(
    readFileSync(join(root, "crates/purlis-core/src/elsewhere.rs"), "utf8"),
    "static NEXT: u32 = 0;\n",
  );
  // A second run finds nothing to release, and the version stays.
  assert.equal(release(root), null);
  assert.equal(versionIn(readFileSync(join(root, PARTS), "utf8")), 13);
});

test("--check refuses while a NEXT is left, and passes once the release step ran", () => {
  // The script reads the tree it sits in, so it runs from a copy of itself in the tree.
  const root = tree({
    [PARTS]: PARTS_RS,
    "tools/theme-parts-release.mjs": readFileSync(SCRIPT, "utf8"),
  });
  const copy = join(root, "tools/theme-parts-release.mjs");
  const run = (...args) => spawnSync(process.execPath, [copy, ...args], { encoding: "utf8" });
  const refused = run("--check");
  assert.equal(refused.status, 1, refused.stderr);
  assert.match(refused.stderr, /crates\/purlis-core\/src\/theme\/parts\.rs:13/);
  const rewritten = run("--no-cargo");
  assert.equal(rewritten.status, 0, rewritten.stderr);
  assert.match(rewritten.stdout, /now version 13/);
  assert.equal(run("--check").status, 0);
});

test("an unknown argument is refused", () => {
  assert.deepEqual(mode(["--bogus"]), { refused: "--bogus" });
  assert.equal(spawnSync(process.execPath, [SCRIPT, "--bogus"]).status, 2);
});

test("this repository's own NEXT definition is never counted as one left", () => {
  const text = readFileSync(join(REPO, PARTS), "utf8");
  assert.match(text, /^pub const NEXT: u32 = VERSION \+ 1;$/m);
  const lines = text.split("\n");
  for (const one of pending(REPO)) {
    const [file, line] = one.split(":");
    if (file === PARTS) assert.doesNotMatch(lines[Number(line) - 1], /pub const NEXT/, one);
  }
});

const LITERAL_RS = `pub const VERSION: u32 = 12;
pub const NEXT: u32 = VERSION + 1;

pub const CHIP: Family = Family {
    name: "chip",
    since: 13,
    parts: &[Part { name: "root", since: 12, height: 13, description: "since: 13" }],
};

impl Prop {
    fn since(self) -> u32 {
        match self {
            Self::Brand => 13, // => 13
            _ => 1,
        }
    }

    fn height(self) -> u32 {
        match self {
            Self::Brand => 13,
            _ => 1,
        }
    }
}
`;

test("a literal VERSION + 1 as a since is found, in a field and in a since() arm only", () => {
  assert.deepEqual(
    literals(LITERAL_RS, 13).map((one) => one.line),
    [6, 13],
  );
  // A released version, a size of 13 and the words in a comment or a string are not one.
  assert.deepEqual(literals(LITERAL_RS, 14), []);
});

test("--check refuses a literal VERSION + 1, and the release step gives it its version", () => {
  const root = tree({
    [PARTS]: LITERAL_RS,
    "tools/theme-parts-release.mjs": readFileSync(SCRIPT, "utf8"),
  });
  const copy = join(root, "tools/theme-parts-release.mjs");
  const run = (...args) => spawnSync(process.execPath, [copy, ...args], { encoding: "utf8" });
  const refused = run("--check");
  assert.equal(refused.status, 1, refused.stderr);
  assert.match(refused.stderr, /parts\.rs:6\n.*parts\.rs:13/s);
  assert.deepEqual(pending(root).length, 2);
  // No NEXT to replace, but what it says is unreleased: the release still raises the version.
  assert.equal(release(root), 13);
  assert.deepEqual(literalsIn(root), []);
  assert.equal(run("--check").status, 0);
});

test("this repository writes no literal VERSION + 1 as a since: an addition is since NEXT", () => {
  assert.deepEqual(
    literalsIn(REPO),
    [],
    "write each of these as `since: NEXT` (`super::NEXT` in a family's own file); a literal " +
      "VERSION + 1 is never given a version by the release step and would ship unpinned",
  );
});

test("the release step's rewrite leaves this repository's code compiling: no NEXT is a conversion's argument", () => {
  // `u64::from(parts::NEXT)` becomes `u64::from(13)`, an i32 no `From` takes (E0277): the
  // release step's first cargo run would not compile. Write such a value as `VERSION + 1`.
  const conversion = /\bfrom\(\s*\d+\s*\)/g;
  const broken = [];
  for (const file of files(REPO)) {
    const text = readFileSync(join(REPO, file), "utf8");
    const before = (text.match(conversion) ?? []).length;
    const after = (replace(text, 9999).match(conversion) ?? []).length;
    if (after !== before) broken.push(file);
  }
  assert.deepEqual(broken, [], "a NEXT passed to `from(...)` turns into an untyped literal");
});
