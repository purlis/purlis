// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { Inbox, NOTHING_WAITS } from "./Inbox";
import { forgetInbox } from "./inboxRules";
import { forgetAsksScope } from "./asksScope";
import { forgetProjectViews } from "./projectViews";
import type { HeldActShown, Shown } from "./bindings";
import { askedSaid } from "./HeldActAsks";
import { TAG } from "./HeldActAsks.fixtures";

/**
 * **A held forge act in Asks agrees with what Asks counts** (#7685 review, F3): its card is the
 * ask's one place in Asks, so where it is the only thing waiting Asks never says nothing waits,
 * and the scope chips narrow its cards as they narrow every other ask.
 */

const PLANE = "/home/dev/plane";

/** Chat 3's tag, and chat 5's: chat 5 is no chat 3 dispatched, filed in another workspace. */
const OF_3: HeldActShown = TAG;
const OF_5: HeldActShown = { ...TAG, ask: 7, session: 5, digest: "digest-of-5" };

/** The registry's row of a held act, as `crate::asking::held_act` lists it. */
const listedOf = (act: HeldActShown): Shown => ({
  session: act.session,
  ask: `held-act:${act.ask}`,
  says: `Approve a ${act.kind} in ${act.repo}? It is made with your sign-in.`,
  options: [],
  source: "held-act",
  chain: [`steward ${act.session}`],
  answer: { via: "held-act", ask: act.ask },
});

const FILED: Record<number, string> = { 3: "alpha", 5: "beta" };
const PARENT: Record<number, number> = { 3: 1 };
const around = {
  here: "alpha",
  workspaceOf: (session: number) => FILED[session],
  workspaceNamed: (key: string) => key,
  chat: { session: 3, name: "steward 3", tasks: 0 },
  askedBy: (session: number) => PARENT[session],
};

beforeEach(() => {
  mockIPC((cmd) => (cmd === "held_acts_waiting" ? [OF_3, OF_5] : null));
});

afterEach(() => {
  cleanup();
  clearMocks();
  forgetInbox();
  forgetProjectViews();
  forgetAsksScope(PLANE);
});

function draw(asks: readonly Shown[]) {
  render(<Inbox plane={PLANE} asks={asks} onGo={vi.fn()} onLeave={vi.fn()} {...around} />);
}

describe("a held forge act in Asks", () => {
  it("is drawn on its card, and Asks never says nothing waits while it is the only ask", async () => {
    mockIPC((cmd) => (cmd === "held_acts_waiting" ? [OF_3] : null));
    draw([listedOf(OF_3)]);

    expect(await screen.findByRole("listitem", { name: askedSaid(OF_3) })).toBeTruthy();
    expect(screen.queryByText(NOTHING_WAITS)).toBeNull();
    expect(screen.queryByText(/Nothing is waiting on you in/)).toBeNull();
  });

  it("is narrowed by Chat as every ask is: another chat's card is not drawn there", async () => {
    draw([listedOf(OF_3), listedOf(OF_5)]);

    expect(await screen.findByRole("listitem", { name: askedSaid(OF_3) })).toBeTruthy();
    expect(screen.queryByRole("listitem", { name: askedSaid(OF_5) })).toBeNull();
  });
});
