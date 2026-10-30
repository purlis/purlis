// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useLayoutEffect, useRef } from "react";
import { Terminal } from "@xterm/xterm";
import { openUrl } from "@tauri-apps/plugin-opener";
import type { PoolSignInTerminal } from "../bindings";
import { onAMac } from "../mac";
import { textSizes } from "../textSize";
import { inForce, terminalFont, xtermLook, xtermTheme } from "../theme/theme";

/** How a strip prints in its terminal: told the way to print, and `undefined` once it is gone. */
export type Printer = (print: ((text: string) => void) | undefined) => void;

/**
 * **The runner's live terminal, inline in a sign-in's strip** (#4043, spec #4023 story 9): where
 * purlis cannot follow a login, the person answers it here. A terminal of a fixed size, drawn from
 * the theme's terminal tokens: the runner's `purlis` made it, and keeps its screen.
 *
 * - **Never a keyboard trap** (WCAG 2.1.2): Tab and Shift+Tab leave it, as no login asks for them,
 *   so Cancel is always reachable.
 * - **It answers nothing a terminal is asked**: the terminal on the runner already did, and a
 *   second answer would be typed into the login.
 * - **Copy from it, paste into it**: ⌘C (Ctrl+C off a Mac) with something selected copies it, and
 *   Ctrl+C with nothing selected interrupts the login. A paste goes in as typed text.
 * - **A link opens only on https**, in the browser here, and only when it is pressed.
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
      linkHandler: {
        activate: (_, uri) => {
          if (uri.startsWith("https://")) void openUrl(uri).catch(() => undefined);
        },
      },
    });
    const swallowed = [
      pane.parser.registerCsiHandler({ final: "c" }, () => true),
      pane.parser.registerCsiHandler({ prefix: ">", final: "c" }, () => true),
      pane.parser.registerCsiHandler({ final: "n" }, () => true),
      pane.parser.registerCsiHandler({ prefix: "?", final: "n" }, () => true),
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
      if (running.current) typed.current(text);
    });
    pane.open(where);
    pane.focus();
    printer((text) => pane.write(text));
    return () => {
      printer(undefined);
      typing.dispose();
      swallowed.forEach((one) => one.dispose());
      pane.dispose();
    };
  }, [printer, size.columns, size.rows]);
  return <div ref={holder} className="pool-sign-in-terminal" role="group" aria-label={label} />;
}
