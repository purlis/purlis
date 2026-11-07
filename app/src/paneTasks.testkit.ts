// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { waitFor } from "@testing-library/react";

/**
 * **A chat's tasks button, as a test finds it** (#5341): the state word on the head of the pane
 * showing chat `name`, described by its tasks' counts ("Tasks of steward 1: 2 working"), which
 * opens their menu. A tab draws none, and only the panes of the tab in front are drawn.
 */

/** What a tasks button is described by: its counts, in words. */
export function describedAs(button: Element): string {
  return document.getElementById(button.getAttribute("aria-describedby") ?? "")?.textContent ?? "";
}

/** Chat `name`'s tasks button, if a head draws one. */
export function tasksButton(name: string): HTMLElement | undefined {
  return [
    ...document.querySelectorAll<HTMLElement>('.pane-state-mark [aria-haspopup="menu"]'),
  ].find((one) => describedAs(one).startsWith(`Tasks of ${name}`));
}

/** Chat `name`'s tasks button, which is there. */
export function theTasksButton(name: string): HTMLElement {
  const found = tasksButton(name);
  if (found === undefined) throw new Error(`no head draws ${name}'s tasks`);
  return found;
}

/** Chat `name`'s tasks button, once a head draws it. */
export function findTasksButton(name: string): Promise<HTMLElement> {
  return waitFor(() => theTasksButton(name));
}
