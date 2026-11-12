// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { composeStories } from "@storybook/react-vite";
import { act, cleanup, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { useState } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { useDismissHold } from "./dismissHold";
import { Notice, NoticeList, type NoticesClear } from "./Notice";
import * as stories from "./NoticesFindable.stories";
import { ASK_WAITS_MS, WINDOW_CONTROLS, goToControl } from "./windowControls";

// Whether the focus is in sight needs a layout, which jsdom has none of: the browser run holds
// that. What jsdom can hold is the rest of it: the focus is somewhere, never on the page.
vi.mock(import("./stories.testkit"), async (actual) => ({
  ...(await actual()),
  expectFocusInSight: () => {
    const at = document.activeElement;
    if (at === null || at === document.body) throw new Error("nothing has the focus");
  },
}));

/**
 * **Notices are findable** (Q182, Q183): the Notices view's stories, each one's keyboard and
 * pointer path walked here in jsdom as `npm run test:stories` walks it in a browser, so each
 * requirement's test is seen to fail and pass where there is no browser to run it in.
 */
const composed = composeStories(stories);
type Composed = (typeof composed)[keyof typeof composed];

afterEach(() => {
  cleanup();
  vi.useRealTimers();
});

/** Draws `Story` and walks its `play`, with what a story's run hands it. */
async function walk(Story: Composed) {
  const { container } = render(<Story />);
  const context = {
    canvasElement: container,
    canvas: within(container),
    userEvent: userEvent.setup(),
  };
  await Story.play?.(context as unknown as Parameters<NonNullable<Composed["play"]>>[0]);
}

const PROJECT = "/home/dev/plane";

describe("the Notices view", () => {
  it("typing in the Notices search filters them by what they say (R-331)", async () => {
    await walk(composed.Searched);
  });

  it("Notices filter by kind and group with counts (R-332)", async () => {
    await walk(composed.ByKind);
  });

  it("a message that names a window control links to the one it names, focused (R-333)", async () => {
    await walk(composed.ToTheControl);
  });

  it("Go to a dismissed control's Notice shows Dismissed, on its Show again (R-333)", async () => {
    await walk(composed.ToADismissedControl);
  });

  it("Go to a control no Notice holds now shows every kind, never a blank view (R-333)", async () => {
    await walk(composed.ToAControlNoLongerThere);
  });

  it("another project's Notices view leaves a Go to alone (R-333)", async () => {
    await walk(composed.AnotherProjectsGoTo);
  });

  it("a dismissed Notice comes back from Dismissed (N), and the keyboard stays (R-334)", async () => {
    await walk(composed.BroughtBack);
  });

  it("Show again hands the keyboard to the row that took its place (R-334)", async () => {
    await walk(composed.ShownAgainOneOfTwo);
  });
});

/** A findable list of two clones' stale repos and a doctor finding, each with a Dismiss. */
function Three({ onClear }: { onClear: (clear: NoticesClear | undefined) => void }) {
  const notice = (cause: string, words: string) => (
    <Notice key={cause} cause={cause} label={cause} onDismiss={() => dismissed.push(cause)}>
      <p>{words}</p>
    </Notice>
  );
  return (
    <NoticeList
      onClear={onClear}
      findable={{ project: PROJECT, dismissed: new Set(), showAgain: () => undefined }}
    >
      {notice("stale-clone:web", "web is on spike/search, not main.")}
      {notice("stale-clone:api", "api is on spike/auth, not main.")}
      {notice("doctor-finding:hooks", "The project's hooks are not installed.")}
    </NoticeList>
  );
}
const dismissed: string[] = [];

describe("Dismiss all in the Notices view", () => {
  it("dismisses only the Notices the search finds (R-331)", async () => {
    dismissed.length = 0;
    let clear: NoticesClear | undefined;
    render(
      <Three
        onClear={(now) => {
          clear = now;
        }}
      />,
    );
    await waitFor(() => expect(clear?.count).toBe(3));
    await userEvent.type(screen.getByRole("searchbox", { name: "Search the notices" }), "spike");
    await waitFor(() => expect(clear?.count).toBe(2));
    act(() => clear?.take().out());
    expect(dismissed.sort()).toEqual(["stale-clone:api", "stale-clone:web"]);
  });
});

describe("a Go to no Notices view reaches", () => {
  it("is let go, so a Notices view drawn later does not jump to it (R-333)", async () => {
    vi.useFakeTimers();
    // A project of its own: the search's kept focus is another test's.
    const project = "/home/dev/later";
    act(() => goToControl(WINDOW_CONTROLS[0], project, "api was not saved"));
    act(() => vi.advanceTimersByTime(ASK_WAITS_MS + 1));
    vi.useRealTimers();
    render(<stories.default.component project={project} />);
    const filters = within(screen.getByRole("group", { name: "Show notices of a kind" }));
    expect(filters.getByRole("button", { name: "All · 4" })).toHaveAttribute(
      "aria-pressed",
      "true",
    );
    expect(document.activeElement).toBe(document.body);
  });
});

/** A findable list whose Dismiss waits for its Undo, as the Notices view's does. */
function Held() {
  const hold = useDismissHold();
  const [kept, setKept] = useState<ReadonlySet<string>>(new Set());
  const cause = "stale-clone:web";
  return (
    <NoticeList
      hold={hold}
      findable={{ project: PROJECT, dismissed: kept, showAgain: () => setKept(new Set()) }}
    >
      {!kept.has(cause) && (
        <Notice cause={cause} label="web in main" onDismiss={() => setKept(new Set([cause]))}>
          <p>web is on spike/search, not main.</p>
        </Notice>
      )}
    </NoticeList>
  );
}

describe("Dismissed (N) while a Dismiss waits for its Undo", () => {
  it("counts the Notice from the press, and Show again brings it back (Q234)", async () => {
    const user = userEvent.setup();
    render(<Held />);
    await user.click(await screen.findByRole("button", { name: "Dismiss" }));
    const filters = within(screen.getByRole("group", { name: "Show notices of a kind" }));
    await user.click(filters.getByRole("button", { name: "Dismissed (1)" }));
    await user.click(screen.getByRole("button", { name: "Show again" }));
    await user.click(filters.getByRole("button", { name: "All · 1" }));
    expect(await screen.findByText(/web is on spike/)).toBeVisible();
  });
});
