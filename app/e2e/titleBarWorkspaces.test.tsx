// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { act, cleanup, render } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import { OUTSIDE } from "../src/actions";
import { TitleBarSwitch, type SwitchProject } from "../src/TitleBarSwitch";
import { Dialog } from "../src/ui/dialog/Dialog";
import { ALL_WORKSPACES, LISTED, closedOrAskedAgain, pressedWhenReady } from "./titleBarWorkspaces";

/**
 * **The ⌄'s press, as a spec makes it** (#5035): read in the page and pressed from there, a step
 * every 200 ms until the workspace popover's list is drawn. A modal dialog that a failed test
 * left up hides the ⌄ from the operator and the screen reader, and takes the keyboard back from
 * a popover opened under it, so the popover goes as it comes: a step that pressed again each time
 * it saw no list opened and lost it for the whole 20 s, and said only that the ⌄ opened nothing.
 */

const PLANE = "/home/dev/one";
const projects: SwitchProject[] = [
  {
    plane: PLANE,
    name: "one",
    waiting: 0,
    workspaces: {
      names: ["alpha"],
      pins: ["alpha"],
      focused: "alpha",
      waiting: { [OUTSIDE]: 0 },
    },
  },
];

/** The title bar, and Use skill's dialog over it while `dialog`. */
function TheWindow({ dialog }: { dialog: boolean }) {
  return (
    <>
      <header data-testid="title-bar">
        <TitleBarSwitch
          projects={projects}
          inFront={PLANE}
          railShown
          does={{ go: () => {}, choose: () => {}, pin: () => {}, create: () => {} }}
        />
      </header>
      {dialog && (
        <Dialog onClose={() => {}} title="Use skill">
          <p>In steward 2.</p>
        </Dialog>
      )}
    </>
  );
}

/** One step of the spec's poll, and the 200 ms it waits before the next. */
async function aStep(mayPress = true) {
  const seen = act(() => pressedWhenReady(LISTED, ALL_WORKSPACES, mayPress));
  await act(async () => {
    await new Promise((done) => setTimeout(done, 200));
  });
  return seen;
}

const expanded = () =>
  document.querySelector(ALL_WORKSPACES)?.getAttribute("aria-expanded") ?? null;

afterEach(() => {
  cleanup();
  // What the last press watched for is the page's, and each test is a page of its own.
  delete (window as Window & { e2eWorkspacesCame?: boolean }).e2eWorkspacesCame;
});

describe("the ⌄'s press from a spec (#5035)", () => {
  it("opens the popover at the first step and keeps it open at the next", async () => {
    render(<TheWindow dialog={false} />);
    expect((await aStep()).at).toBe("pressed");
    expect(document.querySelector(LISTED)).not.toBeNull();
    expect((await aStep()).at).toBe("listed");
    expect(expanded()).toBe("true");
  });

  it("presses nothing behind a modal dialog, and names the dialog in the way", async () => {
    render(<TheWindow dialog />);
    for (let step = 0; step < 3; step++)
      expect(await aStep()).toEqual({ at: "behind", role: "dialog", name: "Use skill" });
    expect(expanded()).toBe("false");
  });

  it("presses nothing once its presses are spent, and says the ⌄ is ready", async () => {
    render(<TheWindow dialog={false} />);
    expect(await aStep(false)).toEqual({ at: "ready", cameAndWent: false });
    expect(document.querySelector(LISTED)).toBeNull();
    expect(expanded()).toBe("false");
  });

  it("under a modal dialog, a press opens the popover and loses it at once", async () => {
    render(<TheWindow dialog />);
    const chevron = document.querySelector<HTMLElement>(ALL_WORKSPACES);
    act(() => chevron?.click());
    expect(document.querySelector(LISTED)).not.toBeNull();
    await act(async () => {
      await new Promise((done) => setTimeout(done, 200));
    });
    expect(document.querySelector(LISTED)).toBeNull();
    expect(expanded()).toBe("false");
  });

  it("says, after its press, that the popover came and went", async () => {
    const { rerender } = render(<TheWindow dialog={false} />);
    expect((await aStep()).at).toBe("pressed");
    // Something takes the popover away, as a dialog's hand-back of the keyboard does.
    rerender(<></>);
    rerender(<TheWindow dialog={false} />);
    expect(await aStep(false)).toEqual({ at: "ready", cameAndWent: true });
  });
});

/**
 * **The palette, up and unseen** (#4351): while it is up it takes every Escape on the window,
 * captured, before anything under it hears it (`Palette.tsx`). This one takes the first, as a
 * palette left up at opacity 0 did in a spec's `after`, and goes.
 */
function aPaletteTakesTheFirstEscape(): () => void {
  let up = true;
  const take = (e: KeyboardEvent) => {
    if (!up || e.key !== "Escape") return;
    e.stopPropagation();
    up = false;
  };
  window.addEventListener("keydown", take, true);
  return () => window.removeEventListener("keydown", take, true);
}

/** One step of the spec's close, and the 250 ms it waits before the next. */
async function aClosingStep(): Promise<boolean> {
  const gone = act(() => closedOrAskedAgain(LISTED));
  await act(async () => {
    await new Promise((done) => setTimeout(done, 250));
  });
  return gone;
}

describe("closing the workspace popover from a spec (#4351)", () => {
  let gone: () => void = () => {};
  afterEach(() => gone());

  it("asked once, leaves it up when the palette took that Escape", async () => {
    render(<TheWindow dialog={false} />);
    expect((await aStep()).at).toBe("pressed");
    gone = aPaletteTakesTheFirstEscape();
    expect(await aClosingStep()).toBe(false);
    expect(document.querySelector(LISTED)).not.toBeNull();
  });

  it("asks again until it has gone", async () => {
    render(<TheWindow dialog={false} />);
    expect((await aStep()).at).toBe("pressed");
    gone = aPaletteTakesTheFirstEscape();
    let closed = false;
    for (let step = 0; step < 4 && !closed; step++) closed = await aClosingStep();
    expect(closed).toBe(true);
    expect(document.querySelector(LISTED)).toBeNull();
  });

  it("closes it at the first ask where nothing else takes the Escape", async () => {
    render(<TheWindow dialog={false} />);
    expect((await aStep()).at).toBe("pressed");
    expect(await aClosingStep()).toBe(false);
    expect(document.querySelector(LISTED)).toBeNull();
    expect(await aClosingStep()).toBe(true);
  });
});
