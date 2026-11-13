// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { isScript, ratchet, sourceFiles, SRC, withoutComments } from "./guards.testkit";

/**
 * **No record list is drawn by hand** (#2911, #2919, #6301). A record list is a list of things a
 * store holds, each row ending in its actions, and it is the list or table family's
 * (`ui/list/List`, `ui/table/Table`, `ui/table/ControlTable`). You › This machine drew its own
 * before the families did: a `<ul className="ui-machine-list">` of `ui-machine-entry` rows. A
 * plain `<ul>` with a class of its own breaks no family's lint (`purlis/list-family` keeps a
 * `<ul>` of words), so this guard holds them instead.
 *
 * The last seven moved onto the families in #6301, so `LEFT` is empty and stays so: a list
 * drawn that way anywhere fails, and its class's rules are gone from `App.css`.
 *
 * Read as text, as `Notice.guard.test.ts` reads its live regions: a rendered window shows only
 * the lists its test set up.
 */

/** The class every hand-drawn record list wore. */
const HAND_DRAWN = "ui-machine-list";

/** Every hand-drawn record list left, one entry per list, by its file under `src`: none. */
const LEFT: Record<string, readonly string[]> = {};

/** How many each file may have: none, never raised. */
const CEILING: Record<string, number> = {};

/** The class as a whole word: `ui-machine-list`, not `ui-machine-listing`. */
const HAND_DRAWN_WORD = new RegExp(String.raw`(?<![\w-])${HAND_DRAWN}(?![\w-])`, "g");

/**
 * Each file's hand-drawn lists, labelled by its `LEFT` entries **by position only**: the
 * labels are for reading, and the count is what is held. When one of a file's two lists moves,
 * the stale line it asks for is the last, whichever list moved.
 */
function found(read: (file: string) => string): Map<string, string[]> {
  const files = sourceFiles((name) => isScript(name) && !name.includes(".stories."));
  const lists = new Map<string, string[]>();
  for (const file of files) {
    const count = withoutComments(read(file), "ts").match(HAND_DRAWN_WORD)?.length ?? 0;
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

  it("refuses one put back where the lists moved from", () => {
    const planted = (file: string) =>
      file === "settings/accounts.tsx"
        ? `${fromDisk(file)}\nexport const X = () => <ul className="${HAND_DRAWN}" />;`
        : fromDisk(file);
    expect(ratchet(found(planted), LEFT, CEILING, WORDS)).toEqual([
      `settings/accounts.tsx: ${WORDS.added}: a hand-drawn list (1)`,
    ]);
  });
});
