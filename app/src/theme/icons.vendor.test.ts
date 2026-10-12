// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/// <reference types="node" />
import { readdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import purlisIcons from "./purlis-icons.json";
import { fromSvg } from "./icons.vendor";

/**
 * **purlis's own icons are the vendored files, converted** (FM-3, #1106): each symbol in
 * `purlis-icons.json` is what `fromSvg` makes of the Material Icon Theme file of the same
 * name under `app/icons/material-icon-theme/`, so the provenance and the licence of every
 * shape the window draws is one directory listing away.
 *
 * To add an icon: copy its SVG from the same release into that directory, map a name to it in
 * `purlis-icons.json`, and run this file with `PURLIS_ICONS_WRITE=1` to write its symbol.
 */

const VENDORED = join(process.cwd(), "icons/material-icon-theme");
const FILE = join(process.cwd(), "src/theme/purlis-icons.json");

const vendored = Object.fromEntries(
  readdirSync(VENDORED)
    .filter((name) => name.endsWith(".svg"))
    .sort()
    .map((name) => [
      name.slice(0, -".svg".length),
      fromSvg(name.slice(0, -".svg".length), readFileSync(join(VENDORED, name), "utf8")),
    ]),
);

if (process.env.PURLIS_ICONS_WRITE === "1") {
  writeFileSync(FILE, `${JSON.stringify({ ...purlisIcons, symbols: vendored }, null, 2)}\n`);
}

describe("purlis's own icon theme", () => {
  it("is every vendored file converted, and nothing else", () => {
    expect(purlisIcons.symbols).toEqual(vendored);
  });

  it("names only symbols it has, and uses every one it has", () => {
    const named = new Set<string>([
      purlisIcons.file,
      purlisIcons.folder,
      purlisIcons.folderOpen,
      ...Object.values(purlisIcons.extensions),
      ...Object.values(purlisIcons.filenames),
      ...Object.values(purlisIcons.folders),
      ...Object.values(purlisIcons.foldersOpen),
    ]);
    expect([...named].sort()).toEqual(Object.keys(vendored).sort());
  });

  it("ships the set's licence beside it", () => {
    const licence = readFileSync(join(VENDORED, "LICENSE"), "utf8");
    expect(licence).toMatch(/^The MIT License/);
    expect(licence).toContain("Copyright (c) 2025 Material Extensions");
  });
});
