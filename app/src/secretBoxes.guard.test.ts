// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/// <reference types="node" />
/**
 * **A secret's box is never given its words** (#2905): every `type="password"` box in the window
 * is the field family's own-words box, which keeps what is typed out of React's state and the
 * page's markup. `TextField`'s props make a `value` on one a type error; this names each box, so
 * a new one is a decision and none is written by hand.
 */
import { readdirSync, readFileSync, statSync } from "node:fs";
import { join, relative } from "node:path";
import { describe, expect, it } from "vitest";

const SRC = join(process.cwd(), "src");

/** Every secret's box in the window, by file: the vault's token and a secret's value, the vault
 *  sign-in's token and the forge sign-in's token. */
const SECRET_BOXES: Record<string, number> = {
  "VaultTab.tsx": 2,
  "VaultSignIn.tsx": 1,
  "ForgeTokenSignIn.tsx": 1,
};

function views(dir = SRC): string[] {
  return readdirSync(dir).flatMap((name) => {
    const path = join(dir, name);
    if (statSync(path).isDirectory()) return views(path);
    return name.endsWith(".tsx") && !/\.(test|stories)\.tsx$/.test(name) ? [path] : [];
  });
}

/** Each opening tag of a `<TextField>` or an `<input>` in `text`, as written. */
function boxes(text: string): string[] {
  const out: string[] = [];
  for (const hit of text.matchAll(/<(TextField|input)\b/g)) {
    let depth = 0;
    let at = hit.index + hit[0].length;
    for (; at < text.length; at++) {
      const char = text[at];
      if (char === "{") depth++;
      else if (char === "}") depth--;
      else if (char === ">" && depth === 0) break;
    }
    out.push(text.slice(hit.index, at + 1));
  }
  return out;
}

const secret = (tag: string) => /\stype=(?:"password"|\{[^}]*"password")/.test(tag);

describe("a secret's box", () => {
  it("is in exactly the files named, as many as named", () => {
    const found: Record<string, number> = {};
    for (const path of views()) {
      if (relative(SRC, path).startsWith("ui/")) continue;
      const count = boxes(readFileSync(path, "utf8")).filter(secret).length;
      if (count > 0) found[relative(SRC, path)] = count;
    }
    expect(found).toEqual(SECRET_BOXES);
  });

  it("is the family's box, never given a value", () => {
    for (const file of Object.keys(SECRET_BOXES)) {
      for (const tag of boxes(readFileSync(join(SRC, file), "utf8")).filter(secret)) {
        expect(tag.startsWith("<TextField"), `${file}: ${tag}`).toBe(true);
        expect(/\s(value|defaultValue)=/.test(tag), `${file}: ${tag}`).toBe(false);
      }
    }
  });

  it("finds a box given its words", () => {
    const planted = `<TextField type="password" value={token} onChange={setToken} />`;
    expect(boxes(planted).filter(secret)).toHaveLength(1);
    expect(/\s(value|defaultValue)=/.test(boxes(planted)[0])).toBe(true);
  });
});
