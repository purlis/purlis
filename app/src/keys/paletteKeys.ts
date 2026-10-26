// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The palette's keys** (`Palette.tsx`), as the key registry reads them (#2921, `registry.ts`):
 * the chord that opens it, where a chat or a tab keeps that chord, and the aimed file's two keys
 * in ⌘P's box. Here and not in the component so the registry can name them without drawing the
 * palette; `Palette.tsx` hands them on under their old names.
 */
import { CHAT_KEYBOARD, RENAMES_ON_F2 } from "../actions";

/** What the aimed file of ⌘P can have done to it from the keyboard (#1143). */
export type FileKey = "copy" | "reveal";

/**
 * Which of the aimed file's keys this keystroke is, if any (#1143): **⌥⌘C and ⌥⌘R on a Mac,
 * Shift+Alt+C and Shift+Alt+R everywhere else** — Copy Path and Reveal in Finder's keys in VS
 * Code's explorer, so the fingers that know an editor know these.
 *
 * They are read only in the palette's box, so no chat or tab ever loses them. ⌘C is the box's
 * own copy, which is why the Mac's has Option in it. Off a Mac, Ctrl is never part of it: Ctrl
 * with Alt is AltGr on Windows, which types a character. A letter is matched by what the key
 * types; on a Mac, Option makes C and R type `ç` and `®`, so a key that types no letter at all
 * is read by its place instead, and a key that types another letter is never this one.
 */
export function fileKeyOf(e: KeyboardEvent, mac: boolean): FileKey | undefined {
  if (!e.altKey || e.ctrlKey) return undefined;
  if (mac ? !e.metaKey || e.shiftKey : !e.shiftKey || e.metaKey) return undefined;
  const typed = e.key.length === 1 && /^[a-z]$/i.test(e.key) ? e.key.toLowerCase() : undefined;
  const letter = typed ?? (e.code === "KeyC" ? "c" : e.code === "KeyR" ? "r" : undefined);
  if (letter === "c") return "copy";
  if (letter === "r") return "reveal";
  return undefined;
}

/** How the palette spells that key, on this platform. */
export function fileKeySaid(does: FileKey, mac: boolean): string {
  const letter = does === "copy" ? "C" : "R";
  return mac ? `⌥⌘${letter}` : `Shift+Alt+${letter}`;
}

/**
 * Whether this keystroke opens the palette.
 *
 * `F2` is the tmux frame's own key, carried over so an operator's fingers do not have to be
 * retrained, and it needs no modifier — which matters because a modifier's name differs by
 * platform and the scenario tests drive a real window. `⌘K` and `Ctrl-K` are what a desktop
 * app is expected to answer; both are accepted on both platforms rather than sniffing one,
 * because the wrong guess is an app with no palette at all.
 */
export function opensIt(e: KeyboardEvent): boolean {
  if (e.key === "F2") return !e.ctrlKey && !e.metaKey && !e.altKey && !e.shiftKey;
  return (e.key === "k" || e.key === "K") && (e.metaKey || e.ctrlKey) && !e.altKey;
}

/**
 * Whether the chat keeps this keystroke, although the palette would otherwise open on it.
 *
 * **The question `opensIt` deliberately does not answer.** That one is about the KEY — is
 * this the palette's chord — and this one is about where it landed, because the same chord
 * belongs to two different programs depending on who has the keyboard. Two predicates and not
 * one condition, for the reason `actions` gives about `available` and `reason`: a surface that
 * says the wrong thing should be a defect in one of them rather than an ambiguity in both.
 *
 * **Only a `Ctrl` chord, and that is the whole of the rule** (charter-app#106). A terminal
 * encodes `Ctrl` with a letter as a C0 control byte — xterm.js 6.0.0 answers `Ctrl` plus a
 * key code in 65..90 with `String.fromCharCode(code - 64)`, so `Ctrl-K` is `\x0b`, which is
 * kill-to-end-of-line in readline and in every emacs-keys line editor. Swallowing it on the
 * window is taking an editing key out of the shell every chat starts in. `F2` is not here
 * because #47 already settled it the other way, with a way back; `⌘` is not here because
 * xterm.js sends nothing at all for a `⌘`-chord but `⌘A`, so there is nothing for the window
 * to be taking.
 *
 * **Where it landed, not where the focus is.** A capture listener on the window runs before
 * the focus has any say, and `e.target` is the element the keystroke was delivered to — which
 * inside a pane is xterm's own textarea, a descendant of the marked holder. An undispatched
 * event has no target and is therefore nobody's: that is the palette's, which is what makes
 * `opensIt` testable on a bare `KeyboardEvent`.
 */
export function theChatKeepsIt(e: KeyboardEvent): boolean {
  if (!e.ctrlKey || e.metaKey) return false;
  const on = e.target;
  return on instanceof Element && on.closest(`[${CHAT_KEYBOARD}]`) !== null;
}

/**
 * Whether an `F2` landed on a chat's tab, where it renames the tab rather than opening this
 * (charter-app#254, `actions.RENAMES_ON_F2`).
 *
 * The same question `theChatKeepsIt` asks — where did the keystroke land — about the one other
 * place `F2` means something of its own: the platform's rename key on a focused item. Nothing
 * is taken from the operator: `⌘K` opens the palette from the tab, and `F2` still does from
 * everywhere else, including the chat itself.
 */
export function theTabRenamesOnIt(e: KeyboardEvent): boolean {
  if (e.key !== "F2") return false;
  const on = e.target;
  return on instanceof Element && on.hasAttribute(RENAMES_ON_F2);
}
