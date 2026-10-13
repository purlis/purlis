import { readFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";

/**
 * No colour outside `app/src/theme` (the repo's rule, §8 of the iOS design): the phone's
 * sources are read whole and refused a hex value, a colour function or a named colour where a
 * value goes. The desktop's guard (`app/src/theme/literals.test.ts`) covers `app/`; this one
 * covers `mobile/`, the same way: Vite's glob says which files exist and Node says what is in
 * them (vitest hands a stylesheet back empty), so a file that comes back empty is a failure
 * and not a silent pass.
 */
const SOURCES: Record<string, string> = Object.fromEntries(
  Object.keys(import.meta.glob(["./**/*.css", "./**/*.ts", "./**/*.tsx", "../index.html"])).map(
    (path) => [path, readFileSync(join(process.cwd(), "src", path), "utf8")],
  ),
);

function withoutComments(text: string, kind: "css" | "ts"): string {
  let stripped = text.replace(/\/\*[\s\S]*?\*\//g, " ").replace(/<!--[\s\S]*?-->/g, " ");
  if (kind === "ts") stripped = stripped.replace(/(^|[^:])\/\/[^\n]*/g, "$1 ");
  return stripped;
}

const HEX = /(?<![\w&])#[0-9a-f]{3,8}\b/gi;
const FUNCTION = /\b(?:rgba?|hsla?|hwb|lab|lch|oklab|oklch|color)\(/g;
const NAMED =
  /(?<=[\w-]+\s*:\s*[^;{}]*)\b(?:white|black|red|green|blue|yellow|orange|grey|gray|silver|navy|teal|purple|pink|brown|cyan|magenta|lime|olive|maroon|aqua|fuchsia)\b/gi;

function at(path: string, text: string, index: number): string {
  return `${path}:${text.slice(0, index).split("\n").length}`;
}

describe("no colour outside the theme", () => {
  const covered = Object.entries(SOURCES)
    .filter(([path]) => !path.endsWith("literals.test.ts"))
    .sort(([a], [b]) => a.localeCompare(b));

  it("found the phone's sources", () => {
    expect(covered.length).toBeGreaterThan(10);
    for (const [path, text] of covered)
      expect(text.length, `${path} read as empty`).toBeGreaterThan(0);
  });

  it.each(covered)("%s names no colour", (path, text) => {
    const kind = path.endsWith(".css") || path.endsWith(".html") ? "css" : "ts";
    const stripped = withoutComments(text, kind);
    const complaints: string[] = [];
    for (const pattern of [HEX, FUNCTION]) {
      for (const hit of stripped.matchAll(pattern)) {
        complaints.push(`${at(path, stripped, hit.index)} has ${hit[0]}`);
      }
    }
    if (kind === "css") {
      for (const hit of stripped.matchAll(NAMED)) {
        complaints.push(`${at(path, stripped, hit.index)} names ${hit[0]}`);
      }
    }
    expect(complaints).toEqual([]);
  });
});
