// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { StrictMode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import App from "./App";
import { forgetRailMode } from "./railMode";
import { showNotices } from "./test-strips";

/**
 * **A project's or the app's result stands behind a dot on the status bar** (#3653, spec #3647;
 * ADR 0121 decision 1, #6004), on the real window with the core mocked: a refused pin pops no
 * card. It leaves a dot on the Notices item, with a "new" pip until it is seen, and is listed in
 * the Notices centre all along. The dot's press shows the same card a workspace's notice is on
 * its chip, hung above the item and pointing down at it; nothing in the page moves; a press
 * elsewhere hides it again; × and Esc dismiss it, in the Notices centre too.
 */

vi.mock("./SessionPane", () => ({
  SessionPane: ({ session }: { session: number }) => (
    <div data-testid="pane">session {session}</div>
  ),
}));

afterEach(() => {
  cleanup();
  clearMocks();
  forgetRailMode();
  vi.restoreAllMocks();
});

const PLANE = "/home/dev/one";
const REFUSED = "charter pins at most 12 workspaces in one project. Unpin one first.";

/** The chat in `alpha`, in front. */
const CHAT = {
  session: 1,
  name: "alpha.1",
  cwd: `${PLANE}/workspaces/alpha`,
  harness: null,
  in_front: true,
  resumed: null,
  fresh: null,
  profile: null,
  persona: null,
  unreported: null,
  card: null,
  guessed: null,
  pinned: false,
  label: null,
  from: null,
};

/** One project whose workspace `alpha` is in front and not pinned; the store refuses its pin.
 *  `seen`, the project's seen record (#6002), where a test reads what is New. */
function core({ seen }: { seen?: string[] } = {}) {
  mockIPC((cmd, args) => {
    if (cmd === "inbox_seen" && seen !== undefined) return seen;
    if (cmd === "see_inbox" && seen !== undefined) {
      seen.push(...((args as { keys: string[] }).keys ?? []));
      return seen;
    }
    if (cmd === "plane_at_launch") return { plane: PLANE, from: PLANE, why: null };
    if (cmd === "plane_sidebar")
      return {
        root: PLANE,
        workspaces: [
          {
            name: "alpha",
            path: `${PLANE}/workspaces/alpha`,
            vision: "",
            todos: [],
            chats: [CHAT],
          },
        ],
        personas: ["steward"],
        persona: "steward",
        unfiled: [],
      };
    if (cmd === "opened_chats") return [CHAT];
    if (cmd === "plane_pins")
      return { project: false, workspaces: [], missing: [], order: [], certain: true };
    // A refusal is a thrown value, as the core's own refusals arrive.
    if (cmd === "pin_workspace") throw REFUSED;
    if (cmd === "reachable_repos") return { repos: [], trouble: [] };
    if (cmd === "chat_states") return [];
    if (cmd === "chats_that_would_not_start") return [];
    if (cmd === "running_sessions") return [];
    if (cmd === "recent_planes") return { planes: [], dropped: [], forgetful: null };
    if (cmd === "extensions_on") return [];
    return null;
  });
}

const opened = () =>
  render(
    <StrictMode>
      <App />
    </StrictMode>,
  );

const bar = () => screen.getByTestId("title-bar");
const line = () => screen.getByTestId("status-line");
/** The card the Notices item's dot shows, named by the project. */
const findCard = () => screen.findByRole("region", { name: "Notices for one" });
const queryCard = () => screen.queryByRole("region", { name: "Notices for one" });
const findCardNow = queryCard;
/** The dot a hidden result leaves on the Notices item. */
const queryDot = () =>
  within(line()).queryByRole("button", { name: /^Show the notice on the status bar/ });
/** The Notices centre's Results, where every result is listed too. */
const results = async () => {
  // Listed in Notices since #5645, in the Panel under Asks.
  await showNotices();
  return screen.findByRole("region", { name: "Results" });
};

/** Refuses the pin of the dashed `alpha +` chip, as the person presses it. */
async function refusePin() {
  const chip = await within(bar()).findByRole("button", { name: "alpha" });
  await waitFor(() => expect(chip).toHaveAttribute("data-variant", "unpinned"));
  await userEvent.click(chip);
}

/** Presses the dot a result leaves on the Notices item, as a person does, and finds its card. */
async function openCard() {
  const dot = await waitFor(() => {
    const one = queryDot();
    if (one === null) throw new Error("no dot on the Notices item yet");
    return one;
  });
  await userEvent.click(dot);
  return findCard();
}

describe("a project's result on the status bar's Notices item (#3653, #6004)", () => {
  it("pops no card: it stands behind a dot, and is listed in the Notices centre", async () => {
    core();
    opened();
    await within(bar()).findByRole("button", { name: "alpha" });

    await refusePin();

    const dot = await waitFor(() => {
      const one = queryDot();
      if (one === null) throw new Error("no dot yet");
      return one;
    });
    expect(dot).toHaveAccessibleName(/most urgent: warning/);
    expect(queryCard()).toBeNull();
    expect(document.querySelector(".window-work > .action-result")).toBeNull();
    expect(line().contains(document.activeElement)).toBe(false);
    expect(within(await results()).getByText(REFUSED)).toBeInTheDocument();
  });

  it("wears a 'new' pip until its line in the Notices centre has been seen (#6004)", async () => {
    core({ seen: [] });
    opened();
    await within(bar()).findByRole("button", { name: "alpha" });
    await refusePin();
    await waitFor(() => expect(queryDot()).toHaveAccessibleName(/, something new$/));
    expect(line().querySelector('[data-scope="status-item"][data-part="pip"]')).not.toBeNull();

    // Its line under Results, in sight in the open Notices view: about a second later, seen.
    await results();
    await waitFor(() => expect(line().querySelector('[data-part="pip"]')).toBeNull(), {
      timeout: 4000,
    });
    expect(queryDot()).not.toHaveAccessibleName(/something new/);
  });

  it("shows the card a chip's notice is on its dot's press, pointing down at Notices, in no page's flow", async () => {
    core();
    opened();
    await within(bar()).findByRole("button", { name: "alpha" });
    await refusePin();

    const card = await openCard();
    expect(card).toHaveTextContent(REFUSED);
    // A popover of the family, the notice surface, above what it hangs from.
    const surface = card.closest('[data-scope="popover"][data-part="root"]');
    expect(surface).toHaveAttribute("data-variant", "notice");
    await waitFor(() => expect(surface).toHaveAttribute("data-placement", "top"));
    // Over the window, and in none of its flow.
    expect(card.closest(".window-work")).toBeNull();
    expect(line().contains(card)).toBe(false);
    // Asked for, it takes the keyboard, since the dot it was on goes as it comes.
    await waitFor(() => expect(card).toHaveFocus());
    expect(queryDot()).toBeNull();
  });

  it("hides on a press elsewhere and leaves its dot, which brings it back", async () => {
    core();
    opened();
    await refusePin();
    await openCard();

    await userEvent.click(document.body);
    await waitFor(() => expect(queryCard()).toBeNull());
    const dot = queryDot();
    expect(dot).not.toBeNull();
    // A Tab stop of its own: no strip's arrows walk the status bar.
    expect(dot).toHaveAttribute("tabindex", "0");
    // Hidden, not dismissed: still in the Notices centre.
    expect(within(await results()).getByText(REFUSED)).toBeInTheDocument();

    (queryDot() as HTMLElement).focus();
    await userEvent.keyboard("{Enter}");
    const card = await findCard();
    await waitFor(() => expect(card).toHaveFocus());
    expect(queryDot()).toBeNull();
  });

  it("keeps the keyboard in it across the palette, opened and closed (#3653's review)", async () => {
    core();
    opened();
    await refusePin();
    const card = await openCard();
    await waitFor(() => expect(card).toHaveFocus());

    // The palette is modal: it says the result on its own line, and gives the keyboard back
    // where it was as it closes, which is still drawn.
    await userEvent.keyboard("{Meta>}k{/Meta}");
    await screen.findByRole("dialog", { name: "Command palette" });
    await userEvent.keyboard("{Escape}");
    await waitFor(() =>
      expect(screen.queryByRole("dialog", { name: "Command palette" })).toBeNull(),
    );
    await waitFor(() => expect(findCardNow()?.contains(document.activeElement)).toBe(true));
  });

  it("is dismissed by its ×, in the Notices centre too", async () => {
    core();
    opened();
    await refusePin();
    const card = await openCard();

    await userEvent.click(within(card).getByRole("button", { name: "Dismiss" }));
    await waitFor(() => expect(queryCard()).toBeNull());
    expect(queryDot()).toBeNull();
    await waitFor(() =>
      expect(screen.queryByRole("region", { name: "Results" })?.textContent ?? "").not.toContain(
        REFUSED,
      ),
    );
  });

  it("is dismissed by Esc while the keyboard is in it, which goes back to Notices", async () => {
    core();
    opened();
    await refusePin();
    const card = await openCard();

    within(card).getByRole("button", { name: "Dismiss" }).focus();
    await userEvent.keyboard("{Escape}");
    await waitFor(() => expect(queryCard()).toBeNull());
    expect(queryDot()).toBeNull();
    await waitFor(() => expect(within(line()).getByTestId("status-alerts")).toHaveFocus());
  });

  it("goes from the status bar when the Notices centre dismisses it", async () => {
    core();
    opened();
    await refusePin();
    await waitFor(() => expect(queryDot()).not.toBeNull());

    await userEvent.click(within(await results()).getByRole("button", { name: "Dismiss" }));
    await waitFor(() => expect(queryDot()).toBeNull());
    expect(queryCard()).toBeNull();
  });
});
