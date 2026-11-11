// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { act, cleanup, render } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import { OUTSIDE } from "../src/actions";
import { TitleBarSwitch, type SwitchProject } from "../src/TitleBarSwitch";
import { Dialog } from "../src/ui/dialog/Dialog";
import { ALL_WORKSPACES, LISTED, pressedWhenReady } from "./titleBarWorkspaces";

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

afterEach(cleanup);

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
      expect(await aStep()).toEqual({ at: "behind", dialog: "Use skill" });
    expect(expanded()).toBe("false");
  });

  it("presses nothing once its presses are spent, and says the ⌄ is ready", async () => {
    render(<TheWindow dialog={false} />);
    expect((await aStep(false)).at).toBe("ready");
    expect(document.querySelector(LISTED)).toBeNull();
    expect(expanded()).toBe("false");
  });
});
