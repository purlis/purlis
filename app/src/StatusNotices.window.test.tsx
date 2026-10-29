// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { StrictMode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import App from "./App";
import { forgetRailMode } from "./railMode";

/**
 * **A project's or the app's result pops from the status bar** (#3653, spec #3647), on the real
 * window with the core mocked: a refused pin is the same card a workspace's notice is on its
 * chip, hung above the status bar's Notices item and pointing down at it. Nothing in the page
 * moves; a press elsewhere hides it and leaves a dot on Notices, which brings it back; × and Esc
 * dismiss it, in the Notices centre too, where it is listed all along.
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

/** One project whose workspace `alpha` is in front and not pinned; the store refuses its pin. */
function core() {
  mockIPC((cmd) => {
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
/** The card that pops from the Notices item, named by the project. */
const findCard = () => screen.findByRole("region", { name: "Notices for one" });
const queryCard = () => screen.queryByRole("region", { name: "Notices for one" });
/** The dot a hidden result leaves on the Notices item. */
const queryDot = () =>
  within(line()).queryByRole("button", { name: "Show the notice on the status bar" });
/** The Notices centre's Results, where every result is listed too. */
const results = () => screen.findByRole("region", { name: "Results" });

/** What the page lays out, in order: each element of the work and of the status bar, by its
 *  tag, its class and its test id, but the Notices centre's Results, which lists the result as
 *  it should. A card in the page's flow would be one more. */
const laidOut = () =>
  [...document.querySelectorAll(".window-work *, [data-testid='status-line'] *")]
    .filter((one) => one.closest(".inbox-results, .action-result-kept") === null)
    .map(
      (one) =>
        `${one.tagName}.${one.getAttribute("class") ?? ""}#${one.getAttribute("data-testid") ?? ""}`,
    );

/** Refuses the pin of the dashed `alpha +` chip, as the person presses it. */
async function refusePin() {
  const chip = await within(bar()).findByRole("button", { name: "alpha" });
  await waitFor(() => expect(chip).toHaveAttribute("data-variant", "unpinned"));
  await userEvent.click(chip);
}

describe("a project's result pops from the status bar's Notices item (#3653)", () => {
  it("as the card a chip's notice is, pointing down at Notices, in no page's flow", async () => {
    core();
    opened();
    await within(bar()).findByRole("button", { name: "alpha" });
    await results().catch(() => undefined);
    const before = laidOut();

    await refusePin();

    const card = await findCard();
    expect(card).toHaveTextContent(REFUSED);
    // A popover of the family, the notice surface, above what it hangs from.
    const surface = card.closest('[data-scope="popover"][data-part="root"]');
    expect(surface).toHaveAttribute("data-variant", "notice");
    await waitFor(() => expect(surface).toHaveAttribute("data-placement", "top"));
    // Over the window, and in none of its flow: the work and the status bar lay out what they
    // did before, and the old notice at the top of the content is gone.
    expect(card.closest(".window-work")).toBeNull();
    expect(line().contains(card)).toBe(false);
    expect(document.querySelector(".window-work > .action-result")).toBeNull();
    expect(laidOut()).toEqual(before);
    // It takes no keyboard as it comes.
    expect(card.contains(document.activeElement)).toBe(false);
    // And it is listed in the Notices centre.
    expect(within(await results()).getByText(REFUSED)).toBeInTheDocument();
  });

  it("is reached by F6", async () => {
    core();
    opened();
    await refusePin();
    const card = await findCard();

    await userEvent.keyboard("{F6}");
    await waitFor(() => expect(card.contains(document.activeElement)).toBe(true));
  });

  it("hides on a press elsewhere and leaves a dot on Notices, which brings it back", async () => {
    core();
    opened();
    await refusePin();
    await findCard();
    expect(queryDot()).toBeNull();

    await userEvent.click(document.body);
    await waitFor(() => expect(queryCard()).toBeNull());
    const dot = queryDot();
    expect(dot).not.toBeNull();
    // A Tab stop of its own: no strip's arrows walk the status bar.
    expect(dot).toHaveAttribute("tabindex", "0");
    // Hidden, not dismissed: still in the Notices centre.
    expect(within(await results()).getByText(REFUSED)).toBeInTheDocument();

    (dot as HTMLElement).focus();
    await userEvent.keyboard("{Enter}");
    const card = await findCard();
    // Asked for, it takes the keyboard, since the dot it was on goes as it comes.
    await waitFor(() => expect(card).toHaveFocus());
    expect(queryDot()).toBeNull();
  });

  it("is dismissed by its ×, in the Notices centre too", async () => {
    core();
    opened();
    await refusePin();
    const card = await findCard();

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
    const card = await findCard();

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
    await findCard();

    await userEvent.click(within(await results()).getByRole("button", { name: "Dismiss" }));
    await waitFor(() => expect(queryCard()).toBeNull());
    expect(queryDot()).toBeNull();
  });
});
