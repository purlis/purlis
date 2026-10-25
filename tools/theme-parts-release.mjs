// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// **The component parts manifest takes one version per release** (operator ruling 2026-10-25,
// ADR 0103 amended 2026-10-25). `parts::VERSION` is the last version a release shipped; a pull
// request that adds a family, a part, a variant, a state or a property gives it `since: NEXT`
// and never touches `VERSION` or the digest table. Release prep runs this, as it runs
// `tools/changelog-fold.mjs`:
//
//   node tools/theme-parts-release.mjs            release prep: every `NEXT` written in code
//                                                 becomes VERSION + 1, VERSION is raised, the
//                                                 new version's digest is pinned and the theme
//                                                 schema written again (both through cargo)
//   node tools/theme-parts-release.mjs --check    exit 1 when a `NEXT` is left: the release
//                                                 workflow refuses a stable tag on it
//   node tools/theme-parts-release.mjs --no-cargo the rewrite alone, for a test
//   node tools/theme-parts-release.mjs --help     say this; any other argument is refused (exit 2)
//
// A manifest with nothing `since: NEXT` is left as it is: a release that added no part keeps its
// version. `NEXT` is replaced only where it is code: never in a comment or a string, and never in
// its own definition. The files read are the theme engine's (`crates/purlis-core/src/theme/`)
// and the CLI's theme tests. A family file names it `super::NEXT` and a test `parts::NEXT`; an
// import of it would be replaced too and not compile, so it is never imported.
import { spawnSync } from "node:child_process";
import { readdirSync, readFileSync, realpathSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

/** Where the manifest's version is, and the files a `NEXT` is looked for in. */
export const PARTS = "crates/purlis-core/src/theme/parts.rs";
export const DIRS = [
  "crates/purlis-core/src/theme",
  "crates/purlis-cli/src",
  "crates/purlis-cli/tests",
];

const VERSION_LINE = /^pub const VERSION: u32 = (\d+);$/m;
const DEFINITION = "pub const NEXT: u32 = VERSION + 1;";
/** `NEXT`, alone or at the end of a path (`super::NEXT`, `parts::NEXT`). */
const MARKER = /(?:\b[A-Za-z_][A-Za-z0-9_]*::)*\bNEXT\b/g;

/**
 * The spans of `text` that are code: what is left once comments, strings and char literals are
 * taken out, as `[start, end)` pairs. Enough of Rust's lexer for this: line and block comments
 * (nested), strings with escapes, raw strings (`r#"…"#`, `br"…"`), and char literals told apart
 * from lifetimes.
 */
export function codeSpans(text) {
  const spans = [];
  let start = 0;
  let i = 0;
  const cut = (from, to) => {
    if (from > start) spans.push([start, from]);
    start = to;
  };
  while (i < text.length) {
    const c = text[i];
    const next = text[i + 1];
    if (c === "/" && next === "/") {
      const end = text.indexOf("\n", i);
      const to = end === -1 ? text.length : end;
      cut(i, to);
      i = to;
    } else if (c === "/" && next === "*") {
      let depth = 1;
      let j = i + 2;
      while (j < text.length && depth > 0) {
        if (text[j] === "/" && text[j + 1] === "*") {
          depth += 1;
          j += 2;
        } else if (text[j] === "*" && text[j + 1] === "/") {
          depth -= 1;
          j += 2;
        } else {
          j += 1;
        }
      }
      cut(i, j);
      i = j;
    } else if (/[rb]/.test(c) && !/[A-Za-z0-9_]/.test(text[i - 1] ?? "")) {
      const raw = /^(?:br|r)(#*)"/.exec(text.slice(i, i + 260));
      if (raw) {
        const close = `"${raw[1]}`;
        const end = text.indexOf(close, i + raw[0].length);
        const to = end === -1 ? text.length : end + close.length;
        cut(i, to);
        i = to;
      } else {
        i += 1;
      }
    } else if (c === '"') {
      let j = i + 1;
      while (j < text.length && text[j] !== '"') j += text[j] === "\\" ? 2 : 1;
      cut(i, j + 1);
      i = j + 1;
    } else if (c === "'") {
      // A char literal ('a', '\n', '\u{1F600}'), or a lifetime ('a), which is code.
      const char = /^'(?:\\(?:u\{[0-9A-Fa-f]+\}|x[0-9A-Fa-f]{2}|.)|[^\\'])'/u.exec(
        text.slice(i, i + 12),
      );
      if (char) {
        cut(i, i + char[0].length);
        i += char[0].length;
      } else {
        i += 1;
      }
    } else {
      i += 1;
    }
  }
  if (start < text.length) spans.push([start, text.length]);
  return spans;
}

/** Every `NEXT` written in code in `text`, but its definition: `{ index, length, line }`. */
export function markers(text) {
  const found = [];
  for (const [from, to] of codeSpans(text)) {
    const code = text.slice(from, to);
    for (const one of code.matchAll(MARKER)) {
      const index = from + one.index;
      const lineStart = text.lastIndexOf("\n", index) + 1;
      const lineEnd = text.indexOf("\n", index);
      const line = text.slice(lineStart, lineEnd === -1 ? text.length : lineEnd);
      if (line.trim() === DEFINITION) continue;
      found.push({ index, length: one[0].length, line: text.slice(0, index).split("\n").length });
    }
  }
  return found;
}

/** `text` with every `NEXT` in code replaced by `version`. */
export function replace(text, version) {
  let out = text;
  for (const one of markers(text).reverse()) {
    out = out.slice(0, one.index) + String(version) + out.slice(one.index + one.length);
  }
  return out;
}

/** The manifest's version, as `parts.rs` says it. */
export function versionIn(text) {
  const found = VERSION_LINE.exec(text);
  if (!found) throw new Error(`${PARTS} has no \`pub const VERSION: u32 = <n>;\` line`);
  return Number(found[1]);
}

/** Every `.rs` file under the directories a `NEXT` is looked for in, relative to `root`. */
export function files(root) {
  const all = [];
  const walk = (dir) => {
    let entries;
    try {
      entries = readdirSync(join(root, dir), { withFileTypes: true });
    } catch {
      return;
    }
    for (const entry of entries) {
      const path = join(dir, entry.name);
      if (entry.isDirectory()) walk(path);
      else if (entry.name.endsWith(".rs")) all.push(path);
    }
  };
  for (const dir of DIRS) walk(dir);
  return all.sort();
}

/** Every `NEXT` left in `root`'s files, as `<file>:<line>`. */
export function pending(root) {
  return files(root).flatMap((file) =>
    markers(readFileSync(join(root, file), "utf8")).map((one) => `${file}:${one.line}`),
  );
}

/**
 * The release step's rewrite: every `NEXT` becomes VERSION + 1 and VERSION is raised to it.
 * Returns the new version, or `null` when nothing was `since: NEXT`.
 */
export function release(root) {
  const left = pending(root);
  if (left.length === 0) return null;
  const parts = join(root, PARTS);
  const version = versionIn(readFileSync(parts, "utf8")) + 1;
  for (const file of files(root)) {
    const path = join(root, file);
    const text = readFileSync(path, "utf8");
    const replaced = replace(text, version);
    if (replaced !== text) writeFileSync(path, replaced);
  }
  const text = readFileSync(parts, "utf8");
  writeFileSync(parts, text.replace(VERSION_LINE, `pub const VERSION: u32 = ${version};`));
  return version;
}

/** What the arguments ask for, or `{ refused }`. */
export function mode(args) {
  const known = new Set(["--check", "--no-cargo", "--help"]);
  const unknown = args.find((one) => !known.has(one));
  if (unknown) return { refused: unknown };
  return {
    check: args.includes("--check"),
    cargo: !args.includes("--no-cargo"),
    help: args.includes("--help"),
  };
}

function cargo(root, env, test) {
  const ran = spawnSync("cargo", ["test", "-p", "purlis-core", "--lib", test], {
    cwd: root,
    env: { ...process.env, ...env },
    stdio: "inherit",
  });
  if (ran.status !== 0) {
    console.error(`cargo test ${test} failed (exit ${ran.status ?? ran.signal})`);
    process.exit(1);
  }
}

function main() {
  const root = join(fileURLToPath(new URL(".", import.meta.url)), "..");
  const asked = mode(process.argv.slice(2));
  if (asked.refused) {
    console.error(`unknown argument ${asked.refused}; see --help`);
    process.exit(2);
  }
  if (asked.help) {
    const self = readFileSync(fileURLToPath(import.meta.url), "utf8");
    console.log(self.split("\nimport ")[0].replace(/^\/\/ ?/gm, ""));
    return;
  }
  if (asked.check) {
    const left = pending(root);
    if (left.length > 0) {
      console.error(
        `::error title=Theme parts manifest::${left.length} \`NEXT\` left, so the manifest holds additions no version names: run \`node tools/theme-parts-release.mjs\` in release prep (docs/updating.md).\n${left.join("\n")}`,
      );
      process.exit(1);
    }
    console.log("No `NEXT` left: every manifest addition has its version.");
    return;
  }
  const version = release(root);
  if (version === null) {
    console.log(
      `Nothing in the manifest is since NEXT: it stays at version ${versionIn(readFileSync(join(root, PARTS), "utf8"))}.`,
    );
    return;
  }
  console.log(`The manifest is now version ${version}.`);
  if (!asked.cargo) return;
  cargo(root, { PURLIS_THEME_PARTS_PIN: "1" }, "every_released_manifest_keeps_its_digest");
  cargo(root, { PURLIS_THEME_SCHEMA_WRITE: "1" }, "the_published_schema");
  console.log(`Version ${version}'s digest is pinned and the theme schema written: commit them.`);
}

if (realpathSync(process.argv[1] ?? "") === realpathSync(fileURLToPath(import.meta.url))) {
  main();
}
