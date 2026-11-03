// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { createContext, type RefObject } from "react";
import type { TerminalRowId } from "./terminalRows";

/**
 * **A pane's terminal, as its head reaches it** (#4760, spec #4750 story 60): a right-click on a
 * pane head lists the pane's rows, and Copy and Paste are among them. They act on the terminal
 * the pane draws, which only `SessionPane` holds, so it says here, while it is drawn, whether
 * anything is selected and how to run one of its rows. None while the pane draws no terminal
 * (a chat still to start, a task that is away).
 */
export type PaneTerminal = {
  /** Whether anything is selected in it now: Copy is off without. */
  selected: () => boolean;
  /** Runs one of its own rows, as its own right-click does. */
  run: (id: TerminalRowId) => void;
};

/** Where a pane's terminal says it is: a ref its pane frame holds, or none outside one. */
export const PaneTerminalAt = createContext<RefObject<PaneTerminal | null> | null>(null);
