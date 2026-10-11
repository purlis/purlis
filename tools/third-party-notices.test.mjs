// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

// The third-party notices (#1784). Run with `node --test tools/`. Every case reads a fixture
// graph and fixture folders, never the real lock: the real run needs every crate's source on
// disk, which only the jobs that build the app have, and it runs there (`ci.yml`).
import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";

import {
  builtCrates,
  collect,
  fallbackFor,
  render,
  shippedCrates,
  shippedNpm,
} from "./third-party-notices.mjs";

const MIT_TEXT =
  "MIT License\n\nCopyright (c) 2024 The a authors\n\nPermission...";
const APACHE_TEXT = "Apache License\nVersion 2.0, January 2004\n...";

/** A folder holding `files` (name to text), for one package's source. */
function folder(files) {
  const dir = mkdtempSync(join(tmpdir(), "notices-"));
  for (const [name, text] of Object.entries(files)) {
    mkdirSync(join(dir, name, ".."), { recursive: true });
    writeFileSync(join(dir, name), text);
  }
  return dir;
}

/** One `cargo metadata` package. `dir` null makes it a path (first-party) package. */
function pkg(
  name,
  { version = "1.0.0", licence = "MIT", dir, authors = [] } = {},
) {
  return {
    id: `${name} ${version}`,
    name,
    version,
    license: licence,
    license_file: null,
    authors,
    source:
      dir === null
        ? null
        : "registry+https://github.com/rust-lang/crates.io-index",
    manifest_path: join(dir ?? "/first-party", name, "Cargo.toml"),
  };
}

const dep = (to, ...kinds) => ({
  pkg: to.id,
  dep_kinds: kinds.map((kind) => ({ kind, target: null })),
});

test("the shipped crates are what the shipped binaries reach through normal and build dependencies, never dev-only ones", () => {
  const app = pkg("purlis-app", { dir: null });
  const core = pkg("purlis-core", { dir: null });
  const runtime = pkg("serde", { dir: "/r" });
  const build = pkg("cc", { dir: "/r" });
  const devOnly = pkg("insta", { dir: "/r" });
  const both = pkg("tempfile", { dir: "/r" });
  const underDev = pkg("similar", { dir: "/r" });
  const unrelated = pkg("release-manifest", { dir: null });
  const unshipped = pkg("clap", { dir: "/r" });
  const metadata = {
    packages: [
      app,
      core,
      runtime,
      build,
      devOnly,
      both,
      underDev,
      unrelated,
      unshipped,
    ],
    workspace_members: [app.id, core.id, unrelated.id],
    resolve: {
      nodes: [
        {
          id: app.id,
          deps: [
            dep(core, null),
            dep(build, "build"),
            dep(devOnly, "dev"),
            dep(both, "dev", null),
          ],
        },
        { id: core.id, deps: [dep(runtime, null)] },
        { id: runtime.id, deps: [] },
        { id: build.id, deps: [] },
        { id: devOnly.id, deps: [dep(underDev, null)] },
        { id: both.id, deps: [] },
        { id: underDev.id, deps: [] },
        { id: unrelated.id, deps: [dep(unshipped, null)] },
        { id: unshipped.id, deps: [] },
      ],
    },
  };
  assert.deepEqual(
    shippedCrates(metadata, ["purlis-app"]).map((one) => one.name),
    ["cc", "serde", "tempfile"],
  );
});

test("a crate only the workspace's development dependencies turn on, which cargo tree does not build for the shipped binaries, is not shipped", () => {
  const app = pkg("purlis-app", { dir: null });
  const hyper = pkg("hyper", { dir: "/r" });
  const h2 = pkg("h2", { version: "0.4.19", dir: "/r" });
  const metadata = {
    packages: [app, hyper, h2],
    workspace_members: [app.id],
    resolve: {
      nodes: [
        { id: app.id, deps: [dep(hyper, null)] },
        // `cargo metadata` unifies features over the workspace, a dev-dependency's included.
        { id: hyper.id, deps: [dep(h2, null)] },
        { id: h2.id, deps: [] },
      ],
    },
  };
  const built = builtCrates(
    "purlis-app v0.4.2 (/somewhere/app/src-tauri)\nhyper v1.0.0\nhyper v1.0.0 (*)\nserde_derive v1.0.0 (proc-macro)\n\n",
  );
  assert.deepEqual([...built].sort(), [
    "hyper 1.0.0",
    "purlis-app 0.4.2",
    "serde_derive 1.0.0",
  ]);
  assert.deepEqual(
    shippedCrates(metadata, ["purlis-app"], built).map((one) => one.name),
    ["hyper"],
  );
  assert.deepEqual(
    shippedCrates(metadata, ["purlis-app"]).map((one) => one.name),
    ["h2", "hyper"],
  );
});

test("a shipped crate's licence texts are the LICENSE, LICENCE, COPYING and NOTICE files beside its manifest", () => {
  const dir = folder({
    "a/LICENSE-MIT": MIT_TEXT,
    "a/LICENSE-APACHE": APACHE_TEXT,
    "a/NOTICE": "a notice",
    "a/README.md": "not a licence",
    "a/Cargo.toml": "",
  });
  const said = collect({
    crates: [
      {
        name: "a",
        version: "1.2.3",
        licence: "MIT OR Apache-2.0",
        dir: join(dir, "a"),
        authors: [],
      },
    ],
    npm: [],
    templates: {},
  });
  assert.deepEqual(said.missing, []);
  assert.deepEqual(said.packages, [
    {
      kind: "crate",
      name: "a",
      version: "1.2.3",
      licence: "MIT OR Apache-2.0",
      texts: [0, 1, 2],
    },
  ]);
  assert.deepEqual(said.texts, [APACHE_TEXT, MIT_TEXT, "a notice"]);
});

test("one text that several packages ship is kept once, named by all of them", () => {
  const dir = folder({
    "a/LICENSE": APACHE_TEXT,
    "b/LICENSE.txt": `${APACHE_TEXT}\r\n\r\n`,
  });
  const said = collect({
    crates: [
      {
        name: "b",
        version: "2.0.0",
        licence: "Apache-2.0",
        dir: join(dir, "b"),
        authors: [],
      },
      {
        name: "a",
        version: "1.0.0",
        licence: "Apache-2.0",
        dir: join(dir, "a"),
        authors: [],
      },
    ],
    npm: [],
    templates: {},
  });
  assert.deepEqual(said.texts, [APACHE_TEXT]);
  assert.deepEqual(
    said.packages.map((one) => [one.name, one.texts]),
    [
      ["a", [0]],
      ["b", [0]],
    ],
  );
});

test("a package that ships no licence file gets its licence's standard text, preferring Apache-2.0, and says so with its authors", () => {
  const dir = folder({ "bare/Cargo.toml": "", "plain/Cargo.toml": "" });
  const said = collect({
    crates: [
      {
        name: "bare",
        version: "0.1.0",
        licence: "MIT/Apache-2.0",
        dir: join(dir, "bare"),
        authors: ["Ada <ada@example.org>"],
      },
      {
        name: "plain",
        version: "0.2.0",
        licence: "MIT",
        dir: join(dir, "plain"),
        authors: [],
      },
    ],
    npm: [],
    templates: { "Apache-2.0": APACHE_TEXT, MIT: "MIT standard text" },
  });
  assert.deepEqual(said.missing, []);
  assert.deepEqual(said.packages, [
    {
      kind: "crate",
      name: "bare",
      version: "0.1.0",
      licence: "MIT/Apache-2.0",
      texts: [0],
      standard: ["Apache-2.0"],
      authors: ["Ada <ada@example.org>"],
    },
    {
      kind: "crate",
      name: "plain",
      version: "0.2.0",
      licence: "MIT",
      texts: [1],
      standard: ["MIT"],
      authors: [],
    },
  ]);
  assert.deepEqual(said.texts, [APACHE_TEXT, "MIT standard text"]);
});

test("the standard text needs every licence an AND names, and one an OR offers", () => {
  const templates = { "Apache-2.0": "a", MIT: "m", "BSD-3-Clause": "b" };
  assert.deepEqual(fallbackFor("MIT OR Apache-2.0", templates), ["Apache-2.0"]);
  assert.deepEqual(fallbackFor("Zlib OR MIT", templates), ["MIT"]);
  assert.deepEqual(fallbackFor("(MIT AND BSD-3-Clause)", templates), [
    "MIT",
    "BSD-3-Clause",
  ]);
  assert.equal(fallbackFor("MIT AND Zlib", templates), undefined);
  assert.equal(fallbackFor("Unicode-3.0", templates), undefined);
  assert.equal(fallbackFor(undefined, templates), undefined);
  assert.equal(
    fallbackFor("Apache-2.0 WITH LLVM-exception", templates),
    undefined,
  );
});

test("a shipped package with no licence text anywhere is missing, which fails the build", () => {
  const dir = folder({ "lost/Cargo.toml": "", "lost-npm/package.json": "{}" });
  const said = collect({
    crates: [
      {
        name: "lost",
        version: "1.0.0",
        licence: "Unicode-3.0",
        dir: join(dir, "lost"),
        authors: [],
      },
    ],
    npm: [
      {
        name: "lost-npm",
        version: "3.0.0",
        licence: undefined,
        dir: join(dir, "lost-npm"),
        authors: [],
      },
    ],
    templates: { MIT: "m" },
  });
  assert.deepEqual(said.missing, [
    "the crate lost 1.0.0 (Unicode-3.0) ships no licence file, and purlis has no standard text for its licence",
    "the npm package lost-npm 3.0.0 (no licence named) ships no licence file, and purlis has no standard text for its licence",
  ]);
});

test("the shipped npm packages are the lock's non-development entries, read from node_modules", () => {
  const app = folder({
    "node_modules/react/package.json": JSON.stringify({ author: "Meta" }),
    "node_modules/react/LICENSE": MIT_TEXT,
    "node_modules/@scope/pkg/package.json": JSON.stringify({
      author: { name: "Someone" },
    }),
    "node_modules/@scope/pkg/LICENCE.md": "scoped licence",
  });
  const lock = {
    packages: {
      "": { name: "app" },
      "node_modules/react": { version: "19.0.0", license: "MIT" },
      "node_modules/@scope/pkg": { version: "1.0.0", license: { type: "ISC" } },
      "node_modules/vite": { version: "7.0.0", license: "MIT", dev: true },
      "node_modules/linked": { link: true },
      "node_modules/fsevents": {
        version: "2.3.3",
        license: "MIT",
        optional: true,
      },
    },
  };
  const npm = shippedNpm(lock, app);
  assert.deepEqual(
    npm.map(({ name, version, licence, authors }) => [
      name,
      version,
      licence,
      authors,
    ]),
    [
      ["@scope/pkg", "1.0.0", "ISC", ["Someone"]],
      ["react", "19.0.0", "MIT", ["Meta"]],
    ],
  );
  const said = collect({ crates: [], npm, templates: {} });
  assert.deepEqual(said.missing, []);
  assert.deepEqual(said.texts, ["scoped licence", MIT_TEXT]);
});

test("a shipped npm package the install left out is missing, unless npm marks it optional", () => {
  const app = folder({});
  const lock = {
    packages: { "node_modules/gone": { version: "1.0.0", license: "MIT" } },
  };
  const said = collect({
    crates: [],
    npm: shippedNpm(lock, app),
    templates: { MIT: "m" },
  });
  assert.deepEqual(said.missing, [
    "the npm package gone 1.0.0 is not installed in node_modules",
  ]);
});

test("the notices file names each text's packages, then gives the text", () => {
  const text = render({
    packages: [
      {
        kind: "crate",
        name: "a",
        version: "1.0.0",
        licence: "MIT",
        texts: [0],
      },
      {
        kind: "crate",
        name: "b",
        version: "2.0.0",
        licence: "MIT",
        texts: [1],
        standard: ["MIT"],
        authors: ["Ada"],
      },
      {
        kind: "npm",
        name: "react",
        version: "19.0.0",
        licence: "MIT",
        texts: [0],
      },
    ],
    texts: ["shared text", "MIT standard text"],
  });
  assert.match(text, /^Third-party notices\n/);
  assert.match(text, /2 Rust crates and 1 npm package/);
  assert.match(
    text,
    /a 1\.0\.0 \(Rust crate, MIT\)\nreact 19\.0\.0 \(npm package, MIT\)\n-+\nshared text\n/,
  );
  assert.match(
    text,
    /b 2\.0\.0 \(Rust crate, MIT\): it ships no licence file, so this is the standard MIT text; its authors: Ada\n-+\nMIT standard text\n/,
  );
});
