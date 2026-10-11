/// <reference types="node" />
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

/**
 * **One place for the Allow labels** (#1700): the words an ask's answers say are written once,
 * in the asks registry (`asking.rs`, `allow_said`), and every Notice that answers an ask, and
 * the Inbox, draws the label its ask's option carries. No string these files write may spell
 * one of them: a second spelling is how the Notice and the Inbox came to say "Allow for this
 * chat" and "Allow only for this chat" for one answer.
 */

const HERE = dirname(fileURLToPath(import.meta.url));

/** The files that draw an ask's answers. */
const DRAWING_ASKS = [
  "SandboxBlockNotice.tsx",
  "DispatchGrantNotice.tsx",
  "TaskBlocksNotice.tsx",
  "TaskPromptNotice.tsx",
  "Inbox.tsx",
];

/** The registry's labels, as `asking.rs` writes them. */
const LABELS = [
  "Allow for this chat",
  "Allow only for this chat",
  "Allow for me on this machine",
  "Allow for everyone in this project",
  "Keep blocked",
  "Never for this pair",
];

/** Every string literal of `source`, comments left out. */
function literals(source: string): string[] {
  const code = source.replace(/\/\*[\s\S]*?\*\//g, "").replace(/(^|[^:])\/\/.*$/gm, "$1");
  return [...code.matchAll(/"((?:[^"\\\n]|\\.)*)"|`((?:[^`\\]|\\.)*)`|>([^<>{}]+)</g)].map(
    (match) => match[1] ?? match[2] ?? match[3] ?? "",
  );
}

describe("an ask's answers are labelled once, in the registry", () => {
  it.each(DRAWING_ASKS)("%s spells no answer of its own", (file) => {
    const said = literals(readFileSync(join(HERE, file), "utf8"));
    const spelled = said.filter((one) => LABELS.some((label) => one.includes(label)));
    expect(spelled).toEqual([]);
  });

  it("would see a label spelled in a string or in markup", () => {
    expect(literals('const keep = { label: "Keep blocked" };')).toContain("Keep blocked");
    expect(literals("<button>Allow for this chat</button>")).toContain("Allow for this chat");
    expect(literals("// Keep blocked, said in a comment")).toEqual([]);
  });
});
