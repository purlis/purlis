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
import type { ReactNode } from "react";

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

/** Controls, and what draws nothing or turns text round: built from code points, so no
 *  invisible character sits in this file. */
const CONTROL = new RegExp(
  `([${[
    [0x00, 0x1f],
    [0x7f, 0x9f],
    [0xad, 0xad],
    [0x061c, 0x061c],
    [0x180e, 0x180e],
    [0x200b, 0x200f],
    [0x2028, 0x202e],
    [0x2060, 0x206f],
    [0xfe00, 0xfe0f],
    [0xfeff, 0xfeff],
    [0xfff9, 0xfffb],
  ]
    .map(([from, to]) => `\\u{${from.toString(16)}}-\\u{${to.toString(16)}}`)
    .join("")}])`,
  "u",
);

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

/** **A chat's text, inert**: every control and invisible character drawn as itself, as a
 *  `control` token, on one flow. Nothing in it is obeyed or laid out as it says. */
export function Inert({ text }: { text: string }) {
  const parts = text.split(CONTROL);
  return (
    <>
      {parts.map((part, at) =>
        at % 2 === 0 ? (
          part
        ) : (
          <mark key={at} data-scope="secret-ask" data-part="control" data-testid="sa-control">
            {controlName(part.codePointAt(0) ?? 0)}
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
