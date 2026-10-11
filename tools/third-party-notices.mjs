// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// The third-party notices (#1784): every Rust crate and npm package purlis ships, with the
// licence text it came with, generated from the locked dependency graph at build time.
//
//   node tools/third-party-notices.mjs [--out <file>]
//       exit 1 naming each shipped package with no licence text; with `--out`, write the
//       notices file there
//
// **When it runs.** `vite build` generates it (`app/vite.config.ts`): the Licences view reads it
// as a lazy chunk, and the build writes it beside the window as `THIRD-PARTY-NOTICES`, which the
// release bundles carry as a resource (`tauri.release.conf.json`). Nothing is committed, so the
// file cannot go stale: each build lists the graph it was built from. A build that cannot find a
// shipped package's licence text fails, and so does this script, which CI runs on its own.
//
// **What is shipped.** Crates: what `purlis-app`, `purlis-cli` and `persona-statistics` (the
// binaries a bundle carries) reach through normal and build dependencies in `cargo metadata`,
// for the platform being built; a development-only dependency is never shipped, and neither is
// one only another workspace member needs. `cargo metadata` resolves features for the whole
// workspace, development dependencies included (a test server's dev-dependency turns on
// `hyper`'s HTTP/2, say), so the walk is cut to what `cargo tree -e normal,build` builds for
// those binaries alone: a crate only a test's features pull in is not shipped either. The workspace's own crates are purlis, under its own
// licence. npm: every entry of `app/package-lock.json` that is not development-only (the same
// line `tools/npm-licences.mjs` draws), read from `app/node_modules`.
//
// **What a licence text is.** The files beside a crate's `Cargo.toml` or in an npm package's
// folder named LICENSE, LICENCE, COPYING, NOTICE or UNLICENSE (any case, any suffix), and a
// crate's `license-file`. A package that ships none is given the standard text of the licence
// it names, from `tools/licence-texts/` (and the repository's own LICENSE for Apache-2.0, which
// is that licence's text unchanged), with the authors its manifest names, and the notices say
// so. Of `A OR B` it takes one, Apache-2.0 first; of `A AND B`, both. A licence with no
// standard text there fails the build: add its text rather than leave a package out.
import { spawnSync } from "node:child_process";
import {
  existsSync,
  readFileSync,
  readdirSync,
  realpathSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { basename, dirname, join } from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

/** The workspace crates whose binaries a bundle carries. */
export const SHIPPED_CRATES = [
  "purlis-app",
  "purlis-cli",
  "persona-statistics",
];

/** What the notices file is called, in the window's build and in each bundle. */
export const NOTICES_FILE = "THIRD-PARTY-NOTICES";

const LICENCE_FILE = /^(licen[cs]e|copying|notice|unlicense)/i;

/**
 * The crates `cargo tree --prefix none --format {p}` lists, as `name version` keys: the build
 * of the shipped binaries alone, with the features that build turns on and no others.
 */
export function builtCrates(tree) {
  const built = new Set();
  for (const line of tree.split("\n")) {
    const [name, version] = line.trim().split(/\s+/);
    if (name && version?.startsWith("v"))
      built.add(`${name} ${version.slice(1)}`);
  }
  return built;
}

/**
 * The third-party crates `roots` ship: every package they reach through normal or build
 * dependencies, less the workspace's own, sorted by name and version. With `built`
 * (`builtCrates`), only the crates that build really compiles.
 */
export function shippedCrates(
  metadata,
  roots = SHIPPED_CRATES,
  built = undefined,
) {
  const byId = new Map(metadata.packages.map((one) => [one.id, one]));
  const nodes = new Map(metadata.resolve.nodes.map((node) => [node.id, node]));
  const members = new Set(metadata.workspace_members);
  const start = metadata.workspace_members.filter((id) =>
    roots.includes(byId.get(id)?.name),
  );
  const missingRoots = roots.filter(
    (name) => !start.some((id) => byId.get(id).name === name),
  );
  if (missingRoots.length > 0)
    throw new Error(`the workspace has no ${missingRoots.join(", ")}`);
  const seen = new Set();
  const queue = [...start];
  while (queue.length > 0) {
    const id = queue.pop();
    if (seen.has(id)) continue;
    seen.add(id);
    for (const one of nodes.get(id)?.deps ?? []) {
      if (one.dep_kinds.some((kind) => kind.kind !== "dev"))
        queue.push(one.pkg);
    }
  }
  return [...seen]
    .filter((id) => !members.has(id) && byId.get(id).source !== null)
    .filter((id) => {
      const one = byId.get(id);
      return built === undefined || built.has(`${one.name} ${one.version}`);
    })
    .map((id) => {
      const one = byId.get(id);
      const dir = dirname(one.manifest_path);
      return {
        name: one.name,
        version: one.version,
        licence: one.license ?? undefined,
        dir,
        authors: one.authors ?? [],
        licenceFile: one.license_file ? join(dir, one.license_file) : undefined,
      };
    })
    .sort(byNameAndVersion);
}

function byNameAndVersion(a, b) {
  return a.name < b.name
    ? -1
    : a.name > b.name
      ? 1
      : a.version < b.version
        ? -1
        : 1;
}

/** An npm manifest's author, as one name. */
function authorOf(manifest) {
  const author = manifest?.author;
  if (typeof author === "string") return [author];
  if (
    author !== null &&
    typeof author === "object" &&
    typeof author.name === "string"
  )
    return [author.name];
  return [];
}

/** The npm packages `lock` ships, each read from its folder under `appDir`, sorted by name. */
export function shippedNpm(lock, appDir) {
  const out = [];
  for (const [path, entry] of Object.entries(lock.packages ?? {})) {
    if (path === "" || entry.link === true || entry.dev === true) continue;
    const dir = join(appDir, path);
    const installed = existsSync(dir);
    // An optional package the install skipped (another platform's binary) is not shipped.
    if (!installed && entry.optional === true) continue;
    let manifest;
    try {
      manifest = JSON.parse(readFileSync(join(dir, "package.json"), "utf8"));
    } catch {
      manifest = undefined;
    }
    const at = path.lastIndexOf("node_modules/");
    out.push({
      name:
        entry.name ??
        (at === -1 ? path : path.slice(at + "node_modules/".length)),
      version: entry.version,
      licence:
        typeof entry.license === "string"
          ? entry.license
          : (entry.license?.type ?? undefined),
      dir,
      authors: authorOf(manifest),
      installed,
    });
  }
  return out.sort(byNameAndVersion);
}

/**
 * The licences whose standard text stands in for a package that ships none: one of an OR
 * (Apache-2.0 first), every one of an AND, or `undefined` where `templates` lacks one. A
 * `WITH` exception has no standard text here.
 */
export function fallbackFor(expression, templates) {
  if (typeof expression !== "string" || /\bWITH\b/.test(expression))
    return undefined;
  // Old crates write `MIT/Apache-2.0` for an OR.
  const spelled = expression.replace(/\//g, " OR ").replace(/[()]/g, " ");
  const ids = spelled
    .split(/\s+/)
    .filter((word) => word !== "" && word !== "OR" && word !== "AND");
  if (ids.length === 0) return undefined;
  if (/\bAND\b/.test(spelled)) {
    if (/\bOR\b/.test(spelled)) return undefined;
    return ids.every((id) => id in templates) ? ids : undefined;
  }
  if (ids.includes("Apache-2.0") && "Apache-2.0" in templates)
    return ["Apache-2.0"];
  const one = ids.find((id) => id in templates);
  return one === undefined ? undefined : [one];
}

/** A text with its line ends and outer blank lines evened, so equal texts compare equal. */
function even(text) {
  return text.replace(/\r\n?/g, "\n").trim();
}

/** Whether `path` is a file, following a link; a broken link is not. */
function isFile(path) {
  try {
    return statSync(path).isFile();
  } catch {
    return false;
  }
}

/** The licence files in `dir`, by name. */
function licenceFilesIn(dir) {
  let names;
  try {
    names = readdirSync(dir);
  } catch {
    return [];
  }
  return names
    .filter((name) => LICENCE_FILE.test(name) && isFile(join(dir, name)))
    .sort()
    .map((name) => join(dir, name));
}

/**
 * Every package's licence texts, each distinct text kept once: `packages` name theirs by index
 * into `texts`. `missing` says which shipped package has none, which fails the build.
 */
export function collect({ crates, npm, templates }) {
  const texts = [];
  const index = new Map();
  const keep = (text) => {
    const evened = even(text);
    if (!index.has(evened)) {
      index.set(evened, texts.length);
      texts.push(evened);
    }
    return index.get(evened);
  };
  const packages = [];
  const missing = [];
  const kinds = [
    ["crate", "the crate", crates],
    ["npm", "the npm package", npm],
  ];
  for (const [kind, called, list] of kinds) {
    for (const one of [...list].sort(byNameAndVersion)) {
      const named = `${called} ${one.name} ${one.version}`;
      if (one.installed === false) {
        missing.push(`${named} is not installed in node_modules`);
        continue;
      }
      const files = licenceFilesIn(one.dir);
      if (one.licenceFile !== undefined && !files.includes(one.licenceFile))
        files.push(one.licenceFile);
      const entry = {
        kind,
        name: one.name,
        version: one.version,
        licence: one.licence,
      };
      if (files.length > 0) {
        entry.texts = files.map((file) => keep(readFileSync(file, "utf8")));
      } else {
        const standard = fallbackFor(one.licence, templates);
        if (standard === undefined) {
          missing.push(
            `${named} (${one.licence ?? "no licence named"}) ships no licence file, and purlis has no standard text for its licence`,
          );
          continue;
        }
        entry.texts = standard.map((id) => keep(templates[id]));
        entry.standard = standard;
        entry.authors = one.authors;
      }
      packages.push(entry);
    }
  }
  return { packages, texts, missing };
}

const RULE = "=".repeat(80);
const LINE = "-".repeat(80);

/** One package as the notices file names it. */
function named(one) {
  const kind = one.kind === "crate" ? "Rust crate" : "npm package";
  const head = `${one.name} ${one.version} (${kind}, ${one.licence ?? "no licence named"})`;
  if (one.standard === undefined) return head;
  const authors = one.authors.length > 0 ? one.authors.join(", ") : "not named";
  return `${head}: it ships no licence file, so this is the standard ${one.standard.join(" and ")} text; its authors: ${authors}`;
}

const counted = (n, one) => `${n} ${one}${n === 1 ? "" : "s"}`;

/** The notices file: a short head, then each text after the packages that ship it. */
export function render({ packages, texts }) {
  const crates = packages.filter((one) => one.kind === "crate").length;
  const lines = [
    "Third-party notices",
    "",
    `purlis is built with the ${counted(crates, "Rust crate")} and ${counted(packages.length - crates, "npm package")} below, which others wrote.`,
    "Each part names the packages that ship one licence text, then gives that text.",
    "purlis's own licence and notice are in LICENSE and NOTICE.",
  ];
  texts.forEach((text, at) => {
    lines.push("", RULE);
    for (const one of packages)
      if (one.texts.includes(at)) lines.push(named(one));
    lines.push(LINE, text);
  });
  return `${lines.join("\n")}\n`;
}

/** The standard licence texts: `tools/licence-texts/<SPDX id>.txt`, and Apache-2.0 from LICENSE. */
export function templatesIn(repo) {
  const dir = join(repo, "tools", "licence-texts");
  const templates = {
    "Apache-2.0": readFileSync(join(repo, "LICENSE"), "utf8"),
  };
  for (const name of readdirSync(dir)) {
    if (name.endsWith(".txt"))
      templates[basename(name, ".txt")] = readFileSync(join(dir, name), "utf8");
  }
  return templates;
}

/** The platform being built: what Tauri says it builds for, or this machine's own. */
function targetTriple(env) {
  if (env.TAURI_ENV_TARGET_TRIPLE) return env.TAURI_ENV_TARGET_TRIPLE;
  const ran = spawnSync("rustc", ["-vV"], { encoding: "utf8" });
  const host = /^host: (\S+)$/m.exec(ran.stdout ?? "")?.[1];
  if (ran.status !== 0 || host === undefined)
    throw new Error(
      `rustc -vV did not name this machine's platform: ${ran.stderr ?? ""}`,
    );
  return host;
}

/** The notices for the repository at `repo`, from `cargo metadata` and the app's lock. */
export function generate({ repo, env = process.env }) {
  const target = targetTriple(env);
  const manifest = join(repo, "Cargo.toml");
  const cargo = (args, what) => {
    const ran = spawnSync("cargo", args, {
      encoding: "utf8",
      maxBuffer: 1 << 30,
    });
    if (ran.status !== 0) throw new Error(`${what} failed:\n${ran.stderr}`);
    return ran.stdout;
  };
  const metadata = cargo(
    [
      "metadata",
      "--format-version",
      "1",
      "--locked",
      "--filter-platform",
      target,
      "--manifest-path",
      manifest,
    ],
    "cargo metadata",
  );
  const tree = cargo(
    [
      "tree",
      "--locked",
      "--edges",
      "normal,build",
      "--target",
      target,
      "--prefix",
      "none",
      "--format",
      "{p}",
      "--manifest-path",
      manifest,
      ...SHIPPED_CRATES.flatMap((name) => ["--package", name]),
    ],
    "cargo tree",
  );
  const app = join(repo, "app");
  return collect({
    crates: shippedCrates(
      JSON.parse(metadata),
      SHIPPED_CRATES,
      builtCrates(tree),
    ),
    npm: shippedNpm(
      JSON.parse(readFileSync(join(app, "package-lock.json"), "utf8")),
      app,
    ),
    templates: templatesIn(repo),
  });
}

/** Whether this file is the script node was asked to run, however the path to it was spelled. */
function runAsScript() {
  try {
    return (
      realpathSync(process.argv[1]) ===
      realpathSync(fileURLToPath(import.meta.url))
    );
  } catch {
    return false;
  }
}

if (runAsScript()) {
  const repo = fileURLToPath(new URL("..", import.meta.url));
  const notices = generate({ repo });
  for (const one of notices.missing) console.error(`error: ${one}`);
  if (notices.missing.length > 0) process.exit(1);
  const out = process.argv.indexOf("--out");
  if (out !== -1) writeFileSync(process.argv[out + 1], render(notices));
  const standard = notices.packages.filter((one) => one.standard !== undefined);
  for (const one of standard)
    console.log(
      `note: ${one.name} ${one.version} ships no licence file; given the standard ${one.standard.join(" and ")} text`,
    );
  console.log(
    `every shipped package has its licence text: ${notices.packages.length} packages, ${notices.texts.length} distinct texts`,
  );
}
