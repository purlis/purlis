// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { readFileSync, readdirSync } from "node:fs";
import { join, relative } from "node:path";
import { describe, expect, it } from "vitest";
import { CLONE_VOUCH } from "./CloneVouchNotice";
import { familyOf } from "./Notice";
import { NOTICE_KINDS } from "./noticeKinds";
import { PLACED_CONTROLS, placedControlsNamedIn } from "./placedControls";
import { WINDOW_CONTROLS, controlsNamedIn, goToSays } from "./windowControls";

/**
 * **Every core message that names a window control** (Q182, R-333; Q234): it links to the
 * control ({@link WINDOW_CONTROLS}), or it is held in {@link UNLINKED}, the exact list of those
 * that link nowhere yet, each with the reason. A new message that names a control fails this
 * test until it is one or the other, as `Notice.guard.test.ts` holds its `COPY_ONLY`.
 *
 * A message names a control where one of its string literals says "press X" or "presses X", or
 * "the window's X", or "use X" or "with X" followed in the same sentence by where it stands (in
 * the window, in Settings, in the title bar, in the Skill studio, in the Saving view, on its
 * notice). X is a label: a capital, and the words after it up to one that starts the rest of
 * the sentence. Read as text, comments and `#[cfg(test)] mod tests` left out.
 */
const ROOT = join(__dirname, "..", "..");

/** Every Rust source of the core and its crates, but their tests. */
function sources(dir: string): string[] {
  return readdirSync(dir, { withFileTypes: true }).flatMap((entry) => {
    const path = join(dir, entry.name);
    if (entry.isDirectory())
      return entry.name === "tests" || entry.name === "target" ? [] : sources(path);
    return entry.name.endsWith(".rs") && !entry.name.endsWith("tests.rs") ? [path] : [];
  });
}

/** A word that ends a label: the rest of the sentence starts there. */
const ENDS = new Set(
  (
    "again there is settles brings then or if from ends may do does asks shows draws so which " +
    "for button with when to beside at before"
  ).split(" "),
);
/** After "in" or "on", a word that says where the control stands, so the label ended before. */
const PLACES = new Set(["the", "a", "an", "its", "their", "this", "Settings"]);
/** Keys of a keyboard, which a terminal's prompt names: not the window's controls. */
const KEYS = new Set(["Return", "Enter", "Esc", "Tab", "Space"]);
const NAMES =
  /\b(?:[Pp]ress(?:es)?|[Uu]se|[Ww]ith|the window's(?: own)?) ([A-Z][^\s,.;:!?()"`]*(?: [^\s,.;:!?()"`]+){0,5})/g;
/** Where a control stands, said after "use X" or "with X". */
const STANDS =
  /^[^.;]*?\b(?:in|on) (?:the window|Settings|the title bar|the Skill studio|the Saving view|its notice)/;

/** The label each phrase of `literal` names a control by. */
function controlsSaid(literal: string): string[] {
  const named: string[] = [];
  for (const found of literal.matchAll(NAMES)) {
    const words = found[1].split(" ");
    const label = [words[0]];
    for (let at = 1; at < words.length; at++) {
      const word = words[at];
      if (ENDS.has(word)) break;
      const next = words[at + 1] ?? "";
      if ((word === "in" || word === "on") && (PLACES.has(next) || /^['{]/.test(next))) break;
      label.push(word);
    }
    if (KEYS.has(label[0])) continue;
    const pressed = /^(?:[Pp]ress|the window)/.test(found[0]);
    if (!pressed && !STANDS.test(literal.slice(found.index))) continue;
    named.push(label.join(" "));
  }
  return named;
}

/** Each source's string literals, its comments and its tests left out, joined across Rust's
 *  line continuations. */
function literalsOf(text: string): string[] {
  const code = text
    .split(/\n#\[cfg\(test\)\]\s*\nmod tests\b/)[0]
    .replace(/^\s*\/\/.*$/gm, "")
    .replace(/\\\n\s*/g, "");
  return [...code.matchAll(/"((?:[^"\\]|\\.)*)"/g)].map((found) => found[1]);
}

/** Read by a chat, whose words can carry no link: the refusal raises an ask (Q181). */
const CHAT = "read by a chat or its agent, which no link reaches: Q181 (the refusal raises an ask)";
/** Printed in a terminal by the `purlis` command. */
const CLI = "printed by the purlis command in a terminal, which no link reaches";
/** Said beside the control it names, in the same view. */
const BESIDE = "said in the view the control stands in, beside it";
/** Names a control on another program's page (a forge's), not the window's. */
const ELSEWHERE = "a control on a forge's own page, not the window's";
/** Names the one ask for the clones purlis has no record of, in Asks, whose row T1-42 draws (#6433). */
const TRUST_THESE =
  "names the Trust these clones ask in Asks, which links once T1-42 draws its row (#6433)";

/**
 * **The core messages that name a window control and link nowhere yet**, exactly: `<path>:
 * <label>`, each with why (Q234's table, from T1-43's review). Adding a line is a decision with
 * its reason; a message that links goes into `WINDOW_CONTROLS` instead.
 */
const UNLINKED: Readonly<Record<string, string>> = {
  "crates/purlis-cli/src/gradelink.rs: Grade": CHAT,
  "crates/purlis-cli/src/pool.rs: Pool setup": CLI,
  "crates/purlis-cli/src/signing.rs: Add SSH key": ELSEWHERE,
  "crates/purlis-cli/src/signing.rs: Add key": ELSEWHERE,
  "crates/purlis-cli/src/skill.rs: Extensions": CLI,
  "crates/purlis-core/src/browserproof.rs: Show what goes": CHAT,
  // The `chats` tool's description (#6235) names the window's Chats tree, a view, as what its
  // list matches; its reader is the chat that calls the tool.
  "crates/purlis-core/src/chattools.rs: Chats tree has them": CHAT,
  "crates/purlis-core/src/chattools.rs: Update": CHAT,
  "crates/purlis-core/src/dispatchplace.rs: Discard": CHAT,
  "crates/purlis-core/src/dispatchplace.rs: Merge": CHAT,
  "crates/purlis-core/src/doctor/clones.rs: Trust these clones": TRUST_THESE,
  "crates/purlis-core/src/doctor/fix/identity.rs: Doctor": CHAT,
  "crates/purlis-core/src/doctor/fix/move_into_repos.rs: Fix": BESIDE,
  "crates/purlis-core/src/doctor/fix/move_into_repos.rs: Trust these clones": TRUST_THESE,
  "crates/purlis-core/src/doctor/sandbox.rs: File report": BESIDE,
  // A merge refused for want of a press asked once more, or for a ruleset that changed, is shown
  // on the merge row and its pane, beside the Merge it names (Q269).
  "crates/purlis-core/src/mergehold.rs: Merge": BESIDE,
  "crates/purlis-core/src/personaverbs/select.rs: Allow": CHAT,
  // Followed only by `purlis` in a terminal (`follow_while`); the window's own sign-in draws
  // the button beside its own words (Q246).
  "crates/purlis-core/src/profilestore/sign_in/terminal.rs: Open in terminal": CLI,
  "crates/purlis-core/src/planegit.rs: Update": CHAT,
  "crates/purlis-core/src/planegit/update.rs: Update": CHAT,
  "crates/purlis-core/src/projectgit.rs: Save": CHAT,
  "crates/purlis-core/src/rootupdate.rs: Update": CHAT,
  "crates/purlis-core/src/sandbox/level_why.rs: Confirm": BESIDE,
  "crates/purlis-core/src/savefix.rs: Continue": CHAT,
  "crates/purlis-core/src/secrets/ask/run.rs: Allow once": CHAT,
  "crates/purlis-core/src/secrets/brokered.rs: Allow": CHAT,
  "crates/purlis-core/src/secrets/vaultcmd.rs: Allow": CHAT,
  "crates/purlis-core/src/settings/hosts.rs: Confirm": BESIDE,
  "crates/purlis-core/src/signingsetup/retry.rs: Sign and retry": CHAT,
};

describe("the window controls the core's messages name", () => {
  const named = [
    ...new Set(
      sources(join(ROOT, "crates")).flatMap((path) =>
        literalsOf(readFileSync(path, "utf8")).flatMap((literal) =>
          controlsSaid(literal).map((label) => `${relative(ROOT, path)}: ${label}`),
        ),
      ),
    ),
  ].sort();
  const linked = new Set([...WINDOW_CONTROLS, ...PLACED_CONTROLS].map((control) => control.words));

  it("finds the messages it holds the window to, the linked one among them", () => {
    expect(named).toContain("crates/purlis-core/src/forge/gitcredential.rs: Trust this clone");
  });

  it("links every control a core message names, or holds it in the exact list of those that link nowhere yet", () => {
    const unlinked = named.filter((one) => !linked.has(one.slice(one.lastIndexOf(": ") + 2)));
    expect(unlinked).toEqual(Object.keys(UNLINKED).sort());
  });

  it("would fail on a new message that names a control with no link", () => {
    expect(controlsSaid("Nothing was saved: press Keep both in the Saving view")).toEqual([
      "Keep both",
    ]);
    expect(controlsSaid("use Pool setup in the window, or run it")).toEqual(["Pool setup"]);
    expect(controlsSaid("Sign in with Open in terminal on 'x''s line in Settings")).toEqual([
      "Open in terminal",
    ]);
    // A key of the keyboard, and a capital with no place, are no control.
    expect(controlsSaid("press Return to go on")).toEqual([]);
    expect(controlsSaid("use Docker, or Podman")).toEqual([]);
  });

  it("goes to Trust this clone on a clone made outside purlis", () => {
    expect(WINDOW_CONTROLS.find((one) => one.words === "Trust this clone")?.family).toBe(
      CLONE_VOUCH,
    );
  });

  it("names each control's Notice by a kind the Notices view filters by", () => {
    for (const control of WINDOW_CONTROLS)
      expect(NOTICE_KINDS[familyOf(`${control.family}:x`)], control.words).toBeDefined();
  });

  it("links every message naming a control to it, by its words", () => {
    const message =
      "If you made this clone yourself, press Trust this clone on its notice in the window";
    expect(controlsNamedIn(message).map(goToSays)).toEqual(["Go to Trust this clone"]);
    expect(controlsNamedIn("Session saved")).toEqual([]);
  });

  /** The controls R-357 names, each where it stands, and a core message that names it. */
  const R_357: readonly [string, string][] = [
    ["Resolve", "Nothing was changed here: press Resolve in the Saving view to settle it"],
    ["Update the pin", "To resume it on 3.0.0, press Update the pin in Settings › You › Profiles"],
    [
      "Accept the folders",
      "open the project once in the purlis app and press Accept the folders in the Skill studio's Needs you",
    ],
    ["Remove", "or use Remove on the pool in Settings › You › This machine › Pools."],
  ];

  it("gives no Go to Open in terminal, which only a terminal prints (Q246)", () => {
    expect(
      placedControlsNamedIn("Sign in with Open in terminal on 'work''s line in Settings › Pools"),
    ).toEqual([]);
  });

  it("gives no Go to Resolve where a resolve is open, which draws Abort in its place", () => {
    expect(placedControlsNamedIn("Abort it and press Resolve again.")).toEqual([]);
    expect(placedControlsNamedIn("Nothing moved: Abort, then Resolve again.")).toEqual([]);
  });

  /**
   * **Where a placed control is named and no Go to is drawn**, exactly: `<path>: <label>`, with
   * why. Resolve again is said while a resolve is open, where the Saving view draws Abort and
   * not Resolve, so a Go to Resolve would find nothing.
   */
  const SAID_IN_PLACE: Readonly<Record<string, string>> = {
    "crates/purlis-core/src/planegit/resolve.rs: Resolve": "during a resolve: Abort stands there",
    "crates/purlis-core/src/planegit/resolvefolder.rs: Resolve":
      "during a resolve: Abort stands there",
  };

  it("gives every core message naming a control in a view or in Settings its Go to", () => {
    // A label linked is not enough: the message's words must be ones its Go to is found by.
    const missed = sources(join(ROOT, "crates")).flatMap((path) =>
      literalsOf(readFileSync(path, "utf8")).flatMap((literal) =>
        controlsSaid(literal)
          .filter((label) => PLACED_CONTROLS.some((one) => one.words === label))
          .filter((label) => !placedControlsNamedIn(literal).some((one) => one.words === label))
          .map((label) => `${relative(ROOT, path)}: ${label}`),
      ),
    );
    expect([...new Set(missed)].sort()).toEqual(Object.keys(SAID_IN_PLACE).sort());
  });

  it("links each control R-357 names, so no message names one of them with no link", () => {
    for (const [words, message] of R_357) {
      expect(linked.has(words), words).toBe(true);
      expect(
        placedControlsNamedIn(message).map((one) => one.words),
        message,
      ).toEqual([words]);
    }
    expect(
      Object.keys(UNLINKED).filter((one) => R_357.some(([words]) => one.endsWith(`: ${words}`))),
    ).toEqual([]);
  });
});

/** Names the control it needs in words the reader above does not take apart (a palette
 *  command, a view's button): the control is attached (R-328). */
const NAMED_IN_WORDS =
  "names the window's control it needs, in words the reader above does not parse";
/** Not a refusal: it says where an act is done, in a tool's description or a brief to a chat. */
const NOT_A_REFUSAL = "not a refusal: it says where an act is done, and owes no way out";
/** A refusal a chat reads whose way out is the window's, said with no control and no ask yet. */
const NO_ASK_YET =
  "a refusal whose way out is the window's, with no control and no ask yet: R-328 debt, its ask a ticket of its own";

/**
 * **The core messages that say "from the window" with no control the reader finds** (R-328),
 * exactly: `<path>` and the words the message starts with, each with why. A refusal whose way
 * out is a press in the window raises an ask carrying that control (`purlis_core::windowonly`),
 * and its message names the control; one that names none is held here, so a new one fails.
 */
const FROM_THE_WINDOW: ReadonlyArray<readonly [path: string, starts: string, why: string]> = [
  ["crates/purlis-cli/src/brokered.rs", "a chat does not force a branch folder's", NAMED_IN_WORDS],
  [
    "crates/purlis-cli/src/brokered.rs",
    "a sandboxed chat does not remove a branch",
    NAMED_IN_WORDS,
  ],
  ["crates/purlis-core/src/gitbroker.rs", "this chat stands in branch folder", NAMED_IN_WORDS],
  ["crates/purlis-core/src/planegit/resolvefolder.rs", "Resolve moves the project", NAMED_IN_WORDS],
  ["crates/purlis-core/src/chattools.rs", "Remove a finished branch folder", NOT_A_REFUSAL],
  ["crates/purlis-core/src/planegit.rs", "This save is left unsigned", NOT_A_REFUSAL],
  ["crates/purlis-core/src/renamelocal/keychain.rs", "it waits for you to finish", NOT_A_REFUSAL],
  ["crates/purlis-core/src/savefix.rs", "find why these checks failed", NOT_A_REFUSAL],
  ["crates/purlis-cli/src/brokered.rs", "a sandboxed chat changes only its own", NO_ASK_YET],
  ["crates/purlis-cli/src/brokered.rs", "a sandboxed chat does not remove a persona", NO_ASK_YET],
  ["crates/purlis-cli/src/brokered.rs", "a sandboxed chat does not remove a workspace", NO_ASK_YET],
  ["crates/purlis-cli/src/brokered.rs", "this chat {runs_as}", NO_ASK_YET],
  ["crates/purlis-cli/src/handoff.rs", "{HANDOFF_SAYS} no purlis app answered", NO_ASK_YET],
  // The forge milestones tool's answer for a project with no repo recorded (main's #6476): read by
  // the chat that called it, which records it with `purlis discover`; no ask for the clone yet.
  [
    "crates/purlis-core/src/catalogue/milestones.rs",
    "This project records no repo for {}",
    NO_ASK_YET,
  ],
  ["crates/purlis-core/src/dispatchguard.rs", "this call runs `purlis worktree", NO_ASK_YET],
  ["crates/purlis-core/src/forge/gitcredential.rs", "this repository's remote pushes", NO_ASK_YET],
  ["crates/purlis-core/src/hoststart.rs", "this chat's profile hands its program", NO_ASK_YET],
  ["crates/purlis-core/src/hoststart.rs", "this session host composes", NO_ASK_YET],
  ["crates/purlis-core/src/mergehold.rs", "{OPENS} `purlis change land`", NO_ASK_YET],
  ["crates/purlis-core/src/piececmd.rs", "{repo} · {piece} holds files git ignores", NO_ASK_YET],
  ["crates/purlis-core/src/piececmd.rs", "{why}. Nothing was removed", NO_ASK_YET],
  ["crates/purlis-core/src/planegit/bringin.rs", "{target} comes in from the window", NO_ASK_YET],
  ["crates/purlis-core/src/planegit/prsave.rs", "{request} merged, and {target} moves", NO_ASK_YET],
  [
    "crates/purlis-core/src/repocmd/clone/landing.rs",
    "{name}: {host} refused this clone",
    NO_ASK_YET,
  ],
];

/** The entries of {@link FROM_THE_WINDOW} that hold `one`, a `<path>: <message>`. */
const heldBy = (one: string) =>
  FROM_THE_WINDOW.filter(([path, starts]) => one.startsWith(`${path}: ${starts}`));

describe("no message says from the window without its control (R-328)", () => {
  const bare = [
    ...new Set(
      sources(join(ROOT, "crates")).flatMap((path) =>
        literalsOf(readFileSync(path, "utf8"))
          .filter((literal) => /\bfrom the window\b/.test(literal))
          .filter((literal) => controlsSaid(literal).length === 0)
          .filter((literal) => !WINDOW_CONTROLS.some((one) => literal.includes(one.words)))
          .map((literal) => `${relative(ROOT, path)}: ${literal}`),
      ),
    ),
  ].sort();

  it("finds the GitHub App token's refusal no longer among them, now that it raises Trust this clone", () => {
    expect(
      bare.filter((one) => one.startsWith("crates/purlis-core/src/forge/apptoken.rs")),
    ).toEqual([]);
  });

  it("holds every message that says from the window with no control in the exact list, each once", () => {
    expect(bare.filter((one) => heldBy(one).length !== 1)).toEqual([]);
    const unused = FROM_THE_WINDOW.filter((entry) =>
      bare.every((one) => !heldBy(one).includes(entry)),
    );
    expect(unused).toEqual([]);
  });
});
