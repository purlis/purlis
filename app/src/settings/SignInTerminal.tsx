// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useLayoutEffect, useRef } from "react";
import { Terminal } from "@xterm/xterm";
import type { PoolSignInTerminal } from "../bindings";
import { onAMac } from "../mac";
import { textSizes } from "../textSize";
import {
  inForce,
  onDrawn,
  onTerminalFont,
  terminalFont,
  xtermLook,
  xtermTheme,
} from "../theme/theme";
import { drawTerminalFont } from "../theme/terminalFont";

/** How a strip prints in its terminal: told the way to print, and `undefined` once it is gone. */
export type Printer = (print: ((text: string) => void) | undefined) => void;

/**
 * **The runner's live terminal, inline in a sign-in's strip** (#4043, spec #4023 story 9): where
 * purlis cannot follow a login, or the person asked for a device code (#4106), they answer it here. A terminal of a fixed size, drawn from
 * the theme's terminal tokens: the runner's `purlis` made it, and keeps its screen.
 *
 * - **Never a keyboard trap** (WCAG 2.1.2): Tab and Shift+Tab leave it, as no login asks for them,
 *   so Cancel is always reachable.
 * - **It answers none of the questions a program asks its terminal** (its kind, its cursor, its
 *   modes, its settings, its colours): the terminal on the runner already did, and a second
 *   answer would be typed into the login. Nor does it tell where the keyboard went, were it asked
 *   to. A click is told to a login that asks for clicks, as a terminal tells it.
 * - **Copy from it, paste into it**: ⌘C (Ctrl+C off a Mac) with something selected copies it, and
 *   Ctrl+C with nothing selected interrupts the login. A paste goes in as typed text.
 * - **No link in it opens anything.** What a runner prints is less trusted than the window, and a
 *   link's words can name another page than it opens: a page is opened only from the strip's own
 *   steps, where purlis checked it is the harness's own. A page printed here is copied by hand.
 * - **The keyboard comes to it only from the strip**, never from another field the person is in.
 * - **A theme drawn while it is up is its theme too** (#4122), its look and its font as well, as a
 *   chat's terminal follows one. Its size stays: the runner's terminal keeps the screen.
 */
export function SignInTerminal({
  label,
  size,
  printer,
  onType,
  live,
}: {
  /** What the screen is, as a region. */
  label: string;
  size: PoolSignInTerminal;
  printer: Printer;
  onType: (text: string) => void;
  /** Whether a login still runs in it: what is typed goes nowhere otherwise. */
  live: boolean;
}) {
  const holder = useRef<HTMLDivElement>(null);
  const typed = useRef(onType);
  const running = useRef(live);
  useLayoutEffect(() => {
    typed.current = onType;
    running.current = live;
  });
  useLayoutEffect(() => {
    const where = holder.current;
    if (!where) return;
    const look = inForce();
    const pane = new Terminal({
      fontSize: textSizes().terminal,
      fontFamily: terminalFont(),
      theme: xtermTheme(look),
      ...xtermLook(look),
      cols: size.columns,
      rows: size.rows,
      scrollback: 1_000,
      screenReaderMode: true,
      // The purlis on the runner says its own lines with a bare line break.
      convertEol: true,
    });
    const { parser } = pane;
    const swallowed = [
      // Its kind, its cursor and its state.
      parser.registerCsiHandler({ final: "c" }, () => true),
      parser.registerCsiHandler({ prefix: ">", final: "c" }, () => true),
      parser.registerCsiHandler({ final: "n" }, () => true),
      parser.registerCsiHandler({ prefix: "?", final: "n" }, () => true),
      // Its modes and its settings.
      parser.registerCsiHandler({ intermediates: "$", final: "p" }, () => true),
      parser.registerCsiHandler({ prefix: "?", intermediates: "$", final: "p" }, () => true),
      parser.registerDcsHandler({ intermediates: "$", final: "q" }, () => true),
      // Its colours, and every link: none opens from here.
      ...[4, 8, 10, 11, 12].map((osc) => parser.registerOscHandler(osc, () => true)),
    ];
    pane.attachCustomKeyEventHandler((event) => {
      if (event.key === "Tab") return false;
      const chord = onAMac() ? event.metaKey : event.ctrlKey;
      if (chord && event.key.toLowerCase() === "c" && pane.hasSelection()) {
        if (event.type === "keydown") {
          void navigator.clipboard.writeText(pane.getSelection()).catch(() => undefined);
        }
        return false;
      }
      return true;
    });
    const typing = pane.onData((text) => {
      // Told only of the keyboard coming and going, which no one typed.
      if (text === "\x1b[I" || text === "\x1b[O") return;
      if (running.current) typed.current(text);
    });
    pane.open(where);
    // Only where the keyboard is nowhere, or in the strip: never out of a field the person is in.
    const at = document.activeElement;
    const strip = where.closest(".pool-sign-in");
    if (at === null || at === document.body || (strip !== null && strip.contains(at))) {
      pane.focus();
    }
    printer((text) => pane.write(text));
    const unfollow = onDrawn((theme) => {
      pane.options.theme = xtermTheme(theme);
      Object.assign(pane.options, xtermLook(theme));
    });
    const unfont = onTerminalFont((family, arrived) => {
      drawTerminalFont(pane, family, arrived);
    });
    return () => {
      unfollow();
      unfont();
      printer(undefined);
      typing.dispose();
      swallowed.forEach((one) => one.dispose());
      pane.dispose();
    };
  }, [printer, size.columns, size.rows]);
  return <div ref={holder} className="pool-sign-in-terminal" role="group" aria-label={label} />;
}
