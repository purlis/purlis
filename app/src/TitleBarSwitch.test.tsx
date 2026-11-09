// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The pinned strip's one Tab stop follows the keyboard** (#5912, WAI-ARIA APG toolbar): the
 * arrows move the stop with the focus, so Tab leaves the strip from the chip in front of the
 * person and Shift+Tab comes back to it, never to the chip the stop was on before.
 */
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it } from "vitest";
import { useState } from "react";
import { OUTSIDE } from "./actions";
import { workspaceMade } from "./actionResult";
import { TitleBarSwitch, type SwitchProject } from "./TitleBarSwitch";

const PLANE = "/home/dev/one";
const nothing = () => {};

function Bar({ notices = false }: { notices?: boolean }) {
  const [focused, setFocused] = useState("beta");
  const projects: SwitchProject[] = [
    {
      plane: PLANE,
      name: "one",
      waiting: 0,
      workspaces: {
        names: ["alpha", "beta", "gamma"],
        pins: ["alpha", "beta", "gamma"],
        focused,
        waiting: { [OUTSIDE]: 0 },
      },
    },
  ];
  return (
    <>
      <button type="button">Before</button>
      <TitleBarSwitch
        projects={projects}
        inFront={PLANE}
        railShown
        does={{
          go: (_plane, workspace) => setFocused(workspace),
          choose: () => {},
          pin: () => {},
          create: () => {},
        }}
        notices={
          notices
            ? {
                on: new Map([
                  [
                    "gamma",
                    [
                      {
                        id: 1,
                        kind: "info" as const,
                        said: workspaceMade("gamma", ["✓ Workspace 'gamma' ready (LOCAL)"], {
                          onAddVision: nothing,
                        }),
                      },
                    ],
                  ],
                ]),
                open: undefined,
                show: nothing,
                hide: nothing,
                dismiss: nothing,
                onSettings: nothing,
              }
            : undefined
        }
      />
      <button type="button">After</button>
    </>
  );
}

const chip = (name: string) => {
  const found = document.querySelector<HTMLElement>(
    `[data-part="workspace"][data-chip-of="${name}"]`,
  );
  if (found === null) throw new Error(`no chip of ${name}`);
  return found;
};
const strip = () => screen.getByRole("toolbar", { name: "Pinned workspaces" });
const stops = () =>
  [...strip().querySelectorAll<HTMLElement>("button")].filter((one) => one.tabIndex === 0);

afterEach(cleanup);

describe("the pinned strip's Tab stop (#5912)", () => {
  it("moves with the arrows, so Tab leaves from the chip arrowed to and Shift+Tab comes back to it", async () => {
    const user = userEvent.setup();
    render(<Bar />);
    // The stop starts on the chip of the workspace in front.
    expect(stops()).toEqual([chip("beta")]);
    chip("beta").focus();
    await user.keyboard("{ArrowRight}");
    await waitFor(() => expect(chip("gamma")).toHaveFocus());
    // One stop, and it is the chip the keyboard is on.
    expect(stops()).toEqual([chip("gamma")]);
    await user.tab();
    expect(screen.getByRole("button", { name: "After" })).toHaveFocus();
    await user.tab({ shift: true });
    expect(chip("gamma")).toHaveFocus();
    // Home walks to the first chip, and the stop goes with it.
    await user.keyboard("{Home}");
    await waitFor(() => expect(chip("alpha")).toHaveFocus());
    expect(stops()).toEqual([chip("alpha")]);
  });

  it("goes back to the chip in front once another workspace is chosen", async () => {
    const user = userEvent.setup();
    render(<Bar />);
    chip("beta").focus();
    await user.keyboard("{ArrowLeft}");
    await waitFor(() => expect(chip("alpha")).toHaveFocus());
    await user.keyboard("{ArrowRight}{ArrowRight}{Enter}");
    await waitFor(() => expect(chip("gamma")).toHaveAttribute("aria-current", "true"));
    expect(stops()).toEqual([chip("gamma")]);
  });

  it("rests on the ⌄ when the arrows reach it, and Shift+Tab comes back to it", async () => {
    const user = userEvent.setup();
    render(<Bar />);
    chip("beta").focus();
    await user.keyboard("{End}");
    const all = screen.getByRole("button", { name: "All workspaces" });
    await waitFor(() => expect(all).toHaveFocus());
    expect(stops()).toEqual([all]);
    await user.tab();
    await user.tab({ shift: true });
    expect(all).toHaveFocus();
  });

  it("rests on a chip's notice dot when the arrows land on it", async () => {
    const user = userEvent.setup();
    render(<Bar notices />);
    chip("beta").focus();
    await user.keyboard("{ArrowRight}");
    const dot = await screen.findByRole("button", { name: /^Show the notice on gamma/ });
    // The dot is before its chip in the strip's order, so one arrow from beta lands on it.
    await waitFor(() => expect(dot).toHaveFocus());
    expect(stops()).toEqual([dot]);
    await user.tab();
    await user.tab({ shift: true });
    expect(dot).toHaveFocus();
  });
});
