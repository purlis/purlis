// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render, screen, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { QuitWarning, type Ending } from "./QuitWarning";

afterEach(cleanup);

const ONE: Ending = {
  key: "/ide.1",
  name: "ide.1",
  harness: "claude",
  cwd: null,
  state: "waiting",
};

/** A chat whose turn ended on work of its own still in flight (#5748). */
const ON_ITS_OWN_WORK: Ending = {
  ...ONE,
  ownWork: { helpers: 0, workflows: 0, monitors: 1, shells: 2 },
};

describe("the quit warning", () => {
  it("writes each answer's type, as every other dialog does (#1719)", () => {
    render(<QuitWarning chats={[ONE]} onQuit={() => {}} onCancel={() => {}} />);

    const dialog = screen.getByRole("alertdialog");
    for (const name of ["Cancel", "Quit purlis"])
      expect(within(dialog).getByRole("button", { name })).toHaveAttribute("type", "button");
  });

  it("says what of a chat's own work quitting ends (#5748)", () => {
    render(<QuitWarning chats={[ON_ITS_OWN_WORK]} onQuit={() => {}} onCancel={() => {}} />);

    const dialog = within(screen.getByRole("alertdialog"));
    expect(dialog.getByRole("alert").textContent).toBe(
      "ide.1 is waiting on its own work: 1 monitor and 2 background shells will end.",
    );
    expect(dialog.queryByText("No chat is mid-turn.")).toBeNull();
  });

  it("adds up the own work of several chats, and names a wake-up as ending too", () => {
    render(
      <QuitWarning
        chats={[
          ON_ITS_OWN_WORK,
          {
            ...ONE,
            key: "/ide.2",
            name: "ide.2",
            ownWork: { helpers: 1, workflows: 0, monitors: 0, shells: 1, wakes_at: 1_800_000_000 },
          },
        ]}
        onQuit={() => {}}
        onCancel={() => {}}
      />,
    );

    expect(within(screen.getByRole("alertdialog")).getByRole("alert").textContent).toBe(
      "2 chats are waiting on their own work: 1 helper, 1 monitor, 3 background shells and 1 wake-up will end.",
    );
  });

  it("still quits over a chat waiting on its own work when Quit purlis is pressed", async () => {
    let quit = 0;
    render(
      <QuitWarning chats={[ON_ITS_OWN_WORK]} onQuit={() => (quit += 1)} onCancel={() => {}} />,
    );

    await userEvent.click(
      within(screen.getByRole("alertdialog")).getByRole("button", { name: "Quit purlis" }),
    );

    expect(quit).toBe(1);
  });
});
