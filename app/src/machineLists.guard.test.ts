// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { ratchet, sourceFiles, SRC, withoutComments } from "./guards.testkit";

/**
 * **Settings' hand-drawn record lists only get fewer** (#2911, #2919). A record list is a list
 * of things a store holds, each row ending in its actions, and it is the list or table family's
 * (`ui/list/List`, `ui/table/Table`, `ui/table/ControlTable`). You › This machine drew its own
 * before the families did: a `<ul className="ui-machine-list">` of `ui-machine-entry` rows. A
 * plain `<ul>` with a class of its own breaks no family's lint (`purlis/list-family` keeps a
 * `<ul>` of words), so this guard holds them instead.
 *
 * `LEFT` names every list still drawn that way, by exact count per file. A new one fails. One
 * moved onto a family fails until its line is crossed off, so the list only gets shorter. When
 * it is empty, delete this file and the `.ui-machine-*` rules in `App.css`.
 *
 * Read as text, as `Notice.guard.test.ts` reads its live regions: a rendered window shows only
 * the lists its test set up.
 */

/** The class every hand-drawn record list wears. */
const HAND_DRAWN = "ui-machine-list";

/** Every hand-drawn record list left, one entry per list, by its file under `src`. */
const LEFT: Record<string, readonly string[]> = {
  "settings/accounts.tsx": ["the forge accounts signed in"],
  "settings/PoolProfiles.tsx": ["the profiles on one pool"],
  "settings/pools.tsx": ["the pools"],
  "settings/profiles.tsx": ["this machine's profiles"],
  "settings/signing.tsx": ["the forge accounts' signing keys", "the other devices' signing keys"],
  "settings/thisMachine.tsx": ["This machine's stores"],
};

/** How many each file may have: frozen at the lists as they were on 2026-11-12, never raised.
 *  Lower one as its file's lists move; a file that reaches 0 leaves both tables. */
const CEILING: Record<string, number> = {
  "settings/accounts.tsx": 1,
  "settings/PoolProfiles.tsx": 1,
  "settings/pools.tsx": 1,
  "settings/profiles.tsx": 1,
  "settings/signing.tsx": 2,
  "settings/thisMachine.tsx": 1,
};

/** Each file's hand-drawn lists, named in source order by its `LEFT` entries where it has them. */
function found(read: (file: string) => string): Map<string, string[]> {
  const files = sourceFiles(
    (name) => /\.tsx?$/.test(name) && !/\.(test|stories|testkit)\.tsx?$/.test(name),
  );
  const lists = new Map<string, string[]>();
  for (const file of files) {
    const count = withoutComments(read(file), "ts").split(HAND_DRAWN).length - 1;
    if (count === 0) continue;
    const names = LEFT[file] ?? [];
    lists.set(
      file,
      Array.from({ length: count }, (_, n) => names[n] ?? `a hand-drawn list (${n + 1})`),
    );
  }
  return lists;
}

const fromDisk = (file: string) => readFileSync(join(SRC, file), "utf8");

const WORDS = {
  added: `a record list drawn by hand (\`${HAND_DRAWN}\`): draw it from the list or table family (#2911)`,
};

describe("Settings' hand-drawn record lists (#2911, #2919)", () => {
  it("are only the ones left, and none is added", () => {
    expect(ratchet(found(fromDisk), LEFT, CEILING, WORDS)).toEqual([]);
  });

  it("refuses a new one in a file that had none", () => {
    const planted = (file: string) =>
      file === "settings/SettingsTab.tsx"
        ? `${fromDisk(file)}\nexport const X = () => <ul className="${HAND_DRAWN}" />;`
        : fromDisk(file);
    expect(ratchet(found(planted), LEFT, CEILING, WORDS)).toEqual([
      `settings/SettingsTab.tsx: ${WORDS.added}: a hand-drawn list (1)`,
    ]);
  });

  it("asks for a moved one's line to be crossed off", () => {
    const planted = (file: string) =>
      file === "settings/accounts.tsx"
        ? fromDisk(file).split(HAND_DRAWN).join("moved")
        : fromDisk(file);
    expect(ratchet(found(planted), LEFT, CEILING, WORDS)).toEqual([
      "settings/accounts.tsx: stale entry, delete it from the allowance: the forge accounts signed in",
    ]);
  });
});
