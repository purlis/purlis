// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { ProposedChangeNotice } from "./ProposedChangeNotice";
import type { Shown } from "./bindings";

/**
 * A change a chat proposed to purlis's own configuration is held on its tab (#1975): the file
 * as purlis reads it and as the change makes it, and Apply and Dismiss. Only the person
 * applies it; Apply counts only once the change is on the screen.
 */

afterEach(() => {
  cleanup();
  clearMocks();
});

const PLANE = "/home/dev/plane";
const SESSION = 3;

const proposal = (more: Partial<Shown> = {}): Shown => ({
  session: SESSION,
  ask: "proposal:7",
  says: "Proposes a change to purlis.toml, which only you apply",
  options: [
    { id: "apply", label: "Apply", allows: true },
    { id: "dismiss", label: "Dismiss", allows: false },
  ],
  source: "proposed-change",
  chain: ["steward 3"],
  answer: { via: "proposed-change", id: 7 },
  writes: {
    kind: "whole",
    path: "purlis.toml",
    changes: [
      {
        says: "The file as it is, and as the change would make it",
        before: "[plane]\n",
        after: '[plane]\nmode = "pr"\n',
        hunks: [{ old_start: 1, old_lines: 1, new_start: 1, new_lines: 2 }],
      },
    ],
  },
  ...more,
});

function core() {
  const asked: { cmd: string; args: unknown }[] = [];
  mockIPC((cmd, args) => {
    asked.push({ cmd, args });
    if (cmd === "apply_proposed_change") return "Applied to purlis.toml.";
    return null;
  });
  return () => asked.filter((one) => one.cmd !== "asks_waiting");
}

describe("a change a chat proposed", () => {
  it("shows the change and applies it, once it is on the screen, as the person", async () => {
    const sent = core();
    render(<ProposedChangeNotice plane={PLANE} session={SESSION} asks={[proposal()]} />);
    const notice = screen.getByRole("status", { name: "Proposed change" });
    expect(notice.textContent).toContain(
      "Proposes a change to purlis.toml, which only you apply. This chat may not write this " +
        "file itself, so only you apply it",
    );
    expect(screen.getAllByRole("button").map((one) => one.textContent)).toEqual([
      "Apply",
      "Dismiss",
    ]);
    await screen.findByTestId("ask-writes");
    await userEvent.click(screen.getByRole("button", { name: "Apply" }));
    await waitFor(() =>
      expect(sent()).toEqual([{ cmd: "apply_proposed_change", args: { plane: PLANE, id: 7 } }]),
    );
  });

  it("offers Dismiss alone where the change cannot be applied, and dismisses it", async () => {
    const sent = core();
    render(
      <ProposedChangeNotice
        plane={PLANE}
        session={SESSION}
        asks={[
          proposal({
            says: "Proposed a change to purlis.toml, which changed since: it can no longer be applied",
            options: [{ id: "dismiss", label: "Dismiss", allows: false }],
            writes: null,
          }),
        ]}
      />,
    );
    expect(screen.getAllByRole("button").map((one) => one.textContent)).toEqual(["Dismiss"]);
    await userEvent.click(screen.getByRole("button", { name: "Dismiss" }));
    await waitFor(() =>
      expect(sent()).toEqual([{ cmd: "dismiss_proposed_change", args: { plane: PLANE, id: 7 } }]),
    );
  });

  it("draws nothing for another chat's proposal", () => {
    core();
    render(<ProposedChangeNotice plane={PLANE} session={4} asks={[proposal()]} />);
    expect(screen.queryByRole("status")).toBeNull();
  });
});
