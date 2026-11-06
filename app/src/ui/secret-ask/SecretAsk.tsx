// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The Secret ask's body** (#5080, spec #5235, ADR 0112; the reviewed prototype of #5227): the
 * parts one notice for a secret is drawn from, whatever it holds (a key's value fields, a vault's
 * access, a program's Allow). A screen composes it from these and never writes
 * `data-scope="secret-ask"` itself (`purlis/secret-ask-family`).
 *
 * **The rules**, each held by construction:
 *
 * - **Who says what.** {@link SecretAskFacts} holds the app's own facts, under *From purlis*, and
 *   only them; {@link SecretAskChat} holds the chat's words, behind its rule and said to be the
 *   chat's. Every word of the chat's is drawn by {@link Inert}: a control or invisible character
 *   is drawn as its name, on one flow, so nothing the chat wrote moves a line, fakes the panel or
 *   turns text round.
 * - **A value box has an edge** ({@link SecretAskValue}): the part's own, on any card.
 * - **The action bar stays in reach** ({@link SecretAskFoot}): held at the body's foot as it
 *   scrolls, saying what the chat hears, with the answers at its end.
 * - **By its parts** (ADR 0103): `root`, `facts`, `facts-head`, `who`, `chat`, `command`,
 *   `control`, `value`, `scope-said`, `tool-line` and `foot` are a theme's to set; the rest lay
 *   it out.
 */
import type { FocusEvent, ReactNode } from "react";
import { capMarks } from "../../combiningMarks";
import { invisible } from "../../invisible";

/** The nearest ancestor of `at` that scrolls on the block axis, or the page's. */
function scrollerOf(at: Element): Element {
  for (let up = at.parentElement; up !== null; up = up.parentElement) {
    const { overflowY } = getComputedStyle(up);
    if ((overflowY === "auto" || overflowY === "scroll") && up.scrollHeight > up.clientHeight)
      return up;
  }
  return document.scrollingElement ?? document.documentElement;
}

/**
 * **A focused control is never under the action bar** (WCAG 2.4.11): the bar is held at the foot
 * of whatever scrolls the body, so a control the keyboard reaches under it is scrolled up clear of
 * it. Only ever a scroll; the keyboard is never moved.
 */
function clearOfTheBar(event: FocusEvent<HTMLDivElement>) {
  const foot = event.currentTarget.querySelector(
    ':scope > [data-scope="secret-ask"][data-part="foot"]',
  );
  const focused = event.target;
  if (foot === null || !(focused instanceof Element) || foot.contains(focused)) return;
  const covered = focused.getBoundingClientRect().bottom - foot.getBoundingClientRect().top;
  if (covered > 0) scrollerOf(event.currentTarget).scrollBy({ top: covered + 8 });
}

/** **The body**: one notice's parts, in order. Named for a reader by `label`. */
export function SecretAskBody({
  label,
  children,
  "data-testid": testId,
}: {
  label: string;
  children: ReactNode;
  "data-testid"?: string;
}) {
  return (
    <div
      data-scope="secret-ask"
      data-part="root"
      role="group"
      aria-label={label}
      data-testid={testId}
      onFocus={clearOfTheBar}
    >
      {children}
    </div>
  );
}

/** **The app's facts**, under *From purlis*: nothing of the chat's is ever a child of this. */
export function SecretAskFacts({ children }: { children: ReactNode }) {
  return (
    <section
      data-scope="secret-ask"
      data-part="facts"
      aria-label="From purlis"
      data-testid="sa-facts"
    >
      <span data-scope="secret-ask" data-part="facts-head">
        From purlis
      </span>
      {children}
    </section>
  );
}

/** **A line**: what it is at its start (`who`), and what it says. */
export function SecretAskLine({
  who,
  children,
  "data-testid": testId,
}: {
  who: ReactNode;
  children: ReactNode;
  "data-testid"?: string;
}) {
  return (
    <p data-scope="secret-ask" data-part="said" data-testid={testId}>
      <span data-scope="secret-ask" data-part="who">
        {who}
      </span>
      <span data-scope="secret-ask" data-part="fact">
        {children}
      </span>
    </p>
  );
}

/** **The chat's own words**, behind the rule: its command, and what it says it is for. */
export function SecretAskChat({ children }: { children: ReactNode }) {
  return (
    <div data-scope="secret-ask" data-part="chat" data-testid="sa-chat">
      {children}
    </div>
  );
}

/** **The chat's command**, as the app received it, plain and wrapped. */
export function SecretAskCommand({ who, text }: { who: ReactNode; text: string }) {
  return (
    <div data-scope="secret-ask" data-part="said">
      <span data-scope="secret-ask" data-part="who">
        {who}
      </span>
      <code data-scope="secret-ask" data-part="command" data-testid="sa-command">
        <Inert text={text} />
      </code>
    </div>
  );
}

/** **What the chat says**, quoted as its own. */
export function SecretAskQuote({ who, text }: { who: ReactNode; text: string }) {
  return (
    <p data-scope="secret-ask" data-part="said" data-testid="sa-chat-purpose">
      <span data-scope="secret-ask" data-part="who">
        {who}
      </span>
      <q data-scope="secret-ask" data-part="quote">
        <Inert text={text} />
      </q>
    </p>
  );
}

/** The names a control is drawn by where it has a short one; any other is `U+XXXX`. */
const CONTROL_NAMES: Readonly<Record<number, string>> = {
  0: "NUL",
  7: "BEL",
  8: "BS",
  9: "TAB",
  10: "LF",
  13: "CR",
  27: "ESC",
  127: "DEL",
};

/** How a control character is drawn: its short name, or its code point. */
export function controlName(code: number): string {
  return CONTROL_NAMES[code] ?? `U+${code.toString(16).toUpperCase().padStart(4, "0")}`;
}

/** A mark the core already wrote for a character no reader sees (`purlis_unicode::marked`). */
const CORE_MARK = /^⟦U\+([0-9A-F]{4,6})⟧/;

/** Whether `ch` is drawn as a token: a control, or what purlis's one rule says no reader sees
 *  (`invisible.ts`, SK-51). */
const drawnAsToken = (ch: string) => {
  const code = ch.codePointAt(0) ?? 0;
  return code < 0x20 || (code >= 0x7f && code <= 0x9f) || invisible(ch);
};

/** **`text` in the pieces it is drawn in**: plain runs, and each control or invisible character,
 *  or the core's mark for one, as its name. */
/** A piece of a chat's text as it is drawn. */
export type InertPiece = { plain: string } | { token: string };

export function inertPieces(text: string): InertPiece[] {
  const capped = capMarks(text).text;
  const pieces: InertPiece[] = [];
  let plain = "";
  const flush = () => {
    if (plain !== "") pieces.push({ plain });
    plain = "";
  };
  const points = [...capped];
  for (let at = 0; at < points.length; at++) {
    const ch = points[at];
    if (ch === "⟦") {
      const rest = points.slice(at, at + 10).join("");
      const mark = CORE_MARK.exec(rest);
      if (mark) {
        flush();
        pieces.push({ token: controlName(parseInt(mark[1], 16)) });
        at += [...mark[0]].length - 1;
        continue;
      }
    }
    if (drawnAsToken(ch)) {
      flush();
      pieces.push({ token: controlName(ch.codePointAt(0) ?? 0) });
      continue;
    }
    plain += ch;
  }
  flush();
  return pieces;
}

/** **A chat's text, inert**: every control and invisible character drawn as its name, as a
 *  `control` token, on one flow, and a run of stacked combining marks cut (`capMarks`), so
 *  nothing in it is obeyed, laid out as it says, or painted over what is around it. */
export function Inert({ text }: { text: string }) {
  const pieces = inertPieces(text);
  return (
    <>
      {pieces.map((piece, at) =>
        "plain" in piece ? (
          piece.plain
        ) : (
          <mark key={at} data-scope="secret-ask" data-part="control" data-testid="sa-control">
            {piece.token}
          </mark>
        ),
      )}
    </>
  );
}

/** **One key's field**: where it lands, how it is filled, and the box or the picker. */
export function SecretAskField({ children }: { children: ReactNode }) {
  return (
    <div data-scope="secret-ask" data-part="field" data-testid="sa-key-fill">
      {children}
    </div>
  );
}

/** **Where a field's value lands**: the vault and key, and what follows it on the line. */
export function SecretAskTarget({
  mark,
  path,
  children,
}: {
  mark: ReactNode;
  path: string;
  children?: ReactNode;
}) {
  return (
    <p data-scope="secret-ask" data-part="target">
      <span data-scope="secret-ask" data-part="mark" aria-hidden="true">
        {mark}
      </span>
      <code data-scope="secret-ask" data-part="key">
        {path}
      </code>
      {children}
    </p>
  );
}

/** **A name as the app knows it**: a key, a vault, a program's path, a host. */
export function SecretAskName({
  children,
  "data-testid": testId,
}: {
  children: ReactNode;
  "data-testid"?: string;
}) {
  return (
    <code data-scope="secret-ask" data-part="key" data-testid={testId}>
      {children}
    </code>
  );
}

/** **Around a box a value or an entry is given in**: the edge it takes on the card. */
export function SecretAskValue({ children }: { children: ReactNode }) {
  return (
    <div data-scope="secret-ask" data-part="value">
      {children}
    </div>
  );
}

/** **The Allow**: its question, its choice, and what the choice does. */
export function SecretAskAllow({ children }: { children: ReactNode }) {
  return (
    <div data-scope="secret-ask" data-part="allow" data-testid="sa-allow">
      {children}
    </div>
  );
}

/** **What the Allow chosen does**, said under it, and read as it changes. */
export function SecretAskScopeSaid({ children }: { children: ReactNode }) {
  return (
    <div
      data-scope="secret-ask"
      data-part="scope-said"
      data-testid="sa-scope-said"
      aria-live="polite"
    >
      {children}
    </div>
  );
}

/** **The line Always allow writes**, exactly. */
export function SecretAskToolLine({ text }: { text: string }) {
  return (
    <pre data-scope="secret-ask" data-part="tool-line" data-testid="sa-tool-line">
      {text}
    </pre>
  );
}

/** **One Secret need line Remember writes**: the key's line and its purpose to edit. */
export function SecretAskNeedRow({ children }: { children: ReactNode }) {
  return (
    <div data-scope="secret-ask" data-part="need-row">
      {children}
    </div>
  );
}

/** **The action bar**: what the chat hears, then the answers. */
export function SecretAskFoot({ hears, children }: { hears: ReactNode; children: ReactNode }) {
  return (
    <div data-scope="secret-ask" data-part="foot">
      <span data-scope="secret-ask" data-part="hears" data-testid="sa-hears">
        {hears}
      </span>
      <span data-scope="secret-ask" data-part="answers">
        {children}
      </span>
    </div>
  );
}
