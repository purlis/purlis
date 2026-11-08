// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { existsSync, mkdirSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { describe, expect, it } from "vitest";
import { forcedProbeFiles, forcedProbes, probeName, probed } from "./forcedProbes";

// What `specs/forced-colors.e2e.ts` reads, held here where it runs on every pull request: the
// spec itself runs only in the scenario run (#3481).
describe("the forced-colours probes", () => {
  it("are found on disk, the window's first, then each family's in its name's order", () => {
    const src = mkdtempSync(join(tmpdir(), "forced-probes-"));
    for (const path of [
      "theme/forced-colors.probe.ts",
      "ui/zeta/forced-colors.probe.ts",
      "ui/alpha/forced-colors.probe.ts",
      "ui/beta/theme.testkit.ts",
    ]) {
      mkdirSync(dirname(join(src, path)), { recursive: true });
      writeFileSync(join(src, path), "export default {};\n");
    }
    expect(forcedProbeFiles(src).map((file) => probeName(file, src))).toEqual([
      "window",
      "alpha",
      "zeta",
    ]);
  });

  it("are the window's and, in their own folders, those of the families the box drew", async () => {
    const names = (await forcedProbes()).map((one) => one.name);
    expect(names[0]).toBe("window");
    for (const family of ["activity", "badge", "button", "chat-mark", "choice", "status"])
      expect(names).toContain(family);
  });

  it("each test a forced-colours sheet beside them", async () => {
    for (const { file } of await forcedProbes())
      expect({ [file]: existsSync(join(dirname(file), "forced-colors.css")) }).toEqual({
        [file]: true,
      });
  });

  it("each read every control they draw, and only those, and import nothing at run time", async () => {
    for (const { name, file, probe } of await forcedProbes()) {
      const drawn = probed(probe.markup);
      expect({ [name]: new Set(drawn).size }).toEqual({ [name]: drawn.length });
      expect({ [name]: drawn.length }).not.toEqual({ [name]: 0 });
      expect({ [name]: Object.keys(probe.forced).sort() }).toEqual({ [name]: [...drawn].sort() });
      for (const p of [...Object.keys(probe.unforced ?? {}), ...(probe.focus ? [probe.focus] : [])])
        expect(drawn, name).toContain(p);
      // The spec imports the file as it is, in WebdriverIO's Node: a type is all it may import.
      const imports = readFileSync(file, "utf8").match(/^import (?!type ).*$/gm) ?? [];
      expect({ [name]: imports }).toEqual({ [name]: [] });
    }
  });

  it("name one focus at the most, as the window has one", async () => {
    const focused = (await forcedProbes()).filter((one) => one.probe.focus);
    expect(focused.length).toBeLessThanOrEqual(1);
  });
});
