// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { LiveSetting } from "./groups";
import { dispatchGroup } from "./dispatch";
import { TASK_SCREENS } from "./taskScreens";
import {
  TASK_SCREENS_KEPT_FOR_DAYS,
  TASK_SCREENS_MOST_KEPT,
  TASK_SCREENS_MOST_KEPT_MIB,
} from "../bindings";

/**
 * **Settings › Project › Dispatch › Keep task screens** (#6306): the switch shows
 * `[task_screens] keep` as the core reads it, turning it on writes it at once, and turning it
 * off asks first, then deletes every kept screen.
 */

const PLANE = "/home/dev/plane";

afterEach(() => {
  cleanup();
  clearMocks();
});

function Row() {
  const setting = dispatchGroup(PLANE).settings.find(
    (one) => one.id === TASK_SCREENS,
  ) as LiveSetting;
  const { control, error } = setting.useControl();
  return (
    <>
      {control({ id: setting.id, label: setting.label, describedBy: undefined })}
      {error?.map((said) => (
        <p key={said}>{said}</p>
      ))}
    </>
  );
}

/** A core keeping `kept` screens while `keep`, and what it was asked to set. */
function aCore(keep: boolean, kept: number) {
  const state = { keep, kept };
  const set: unknown[] = [];
  mockIPC((cmd, args) => {
    if (cmd === "task_screens") return { ...state };
    if (cmd === "set_task_screens") {
      set.push(args);
      state.keep = (args as { keep: boolean }).keep;
      if (!state.keep) state.kept = 0;
      return { ...state };
    }
    throw new Error(`unexpected ${cmd}`);
  });
  return { state, set };
}

describe("the Keep task screens row", () => {
  it("says what is kept, where, and for how long, and that off deletes", () => {
    const setting = dispatchGroup(PLANE).settings.find((one) => one.id === TASK_SCREENS);
    expect(setting?.label).toBe("Keep task screens");
    expect(setting?.help).toMatch(/last screen/);
    expect(setting?.help).toMatch(/dispatched task/);
    expect(setting?.help).toMatch(/purlis\.local\.toml/);
    expect(setting?.help).toMatch(/deletes every kept screen/);
    expect((setting as LiveSetting).writes).toBe("local");
  });

  it("says when a screen goes and the store's caps, in the core's numbers", () => {
    // #6451's review: 30 days from when it was kept, swept as the project opens, and 256 MiB.
    const help = dispatchGroup(PLANE).settings.find((one) => one.id === TASK_SCREENS)?.help;
    expect(help).toContain(`${TASK_SCREENS_KEPT_FOR_DAYS} days after it was kept`);
    expect(help).toMatch(/when the project opens/i);
    expect(help).toContain(
      `at most ${TASK_SCREENS_MOST_KEPT} screens and ${TASK_SCREENS_MOST_KEPT_MIB} MiB`,
    );
    expect(help).toContain(`${TASK_SCREENS_MOST_KEPT_MIB} MiB`);
    expect([
      TASK_SCREENS_MOST_KEPT,
      TASK_SCREENS_KEPT_FOR_DAYS,
      TASK_SCREENS_MOST_KEPT_MIB,
    ]).toEqual([200, 30, 256]);
  });

  it("shows the setting the core reads, on by default", async () => {
    aCore(true, 3);
    render(<Row />);
    await waitFor(() =>
      expect(screen.getByRole("switch", { name: "Keep task screens" })).toBeChecked(),
    );
  });

  it("turns keeping on at once, asking nothing", async () => {
    const { set } = aCore(false, 0);
    render(<Row />);
    const toggle = screen.getByRole("switch", { name: "Keep task screens" });
    await waitFor(() => expect(toggle).not.toBeChecked());

    await userEvent.click(toggle);

    await waitFor(() => expect(toggle).toBeChecked());
    expect(set).toEqual([{ plane: PLANE, keep: true }]);
    expect(screen.queryByRole("alertdialog")).toBeNull();
  });

  it("asks before turning it off, and deletes the kept screens only on the yes", async () => {
    const { set } = aCore(true, 3);
    render(<Row />);
    const toggle = screen.getByRole("switch", { name: "Keep task screens" });
    await waitFor(() => expect(toggle).toBeChecked());

    await userEvent.click(toggle);

    const ask = await screen.findByRole("alertdialog");
    expect(ask).toHaveTextContent("3 kept screens");
    expect(set).toEqual([]);
    expect(toggle).toBeChecked();
    await userEvent.click(within(ask).getByRole("button", { name: "Delete kept screens" }));

    await waitFor(() => expect(toggle).not.toBeChecked());
    expect(set).toEqual([{ plane: PLANE, keep: false }]);
    await waitFor(() => expect(screen.queryByRole("alertdialog")).toBeNull());
  });

  it("keeps everything when the ask is cancelled", async () => {
    const { set } = aCore(true, 1);
    render(<Row />);
    const toggle = screen.getByRole("switch", { name: "Keep task screens" });
    await waitFor(() => expect(toggle).toBeChecked());

    await userEvent.click(toggle);
    const ask = await screen.findByRole("alertdialog");
    expect(ask).toHaveTextContent("1 kept screen");
    await userEvent.keyboard("{Escape}");

    await waitFor(() => expect(screen.queryByRole("alertdialog")).toBeNull());
    expect(toggle).toBeChecked();
    expect(set).toEqual([]);
  });

  it("says the core's refusal beside it", async () => {
    mockIPC((cmd) => {
      if (cmd === "task_screens") return { keep: false, kept: 0 };
      if (cmd === "set_task_screens")
        throw new Error("purlis.local.toml is not valid TOML (x), so nothing was changed.");
      throw new Error(`unexpected ${cmd}`);
    });
    render(<Row />);
    const toggle = screen.getByRole("switch", { name: "Keep task screens" });
    await waitFor(() => expect(toggle).not.toBeChecked());

    await userEvent.click(toggle);

    expect(await screen.findByText(/so nothing was changed/)).toBeInTheDocument();
    expect(toggle).not.toBeChecked();
  });

  it("says which kept screens stayed when off could not delete them, and shows it is off", async () => {
    // #6451's review: off is written, then the deletes fail; the row says so and counts them.
    const stayed =
      "1 kept screen could not be deleted (Permission denied). Check that purlis may change " +
      "/home/dev/plane/.purlis/app/chat-screens, then turn it off again.";
    const state = { keep: true, kept: 1 };
    mockIPC((cmd) => {
      if (cmd === "task_screens") return { ...state };
      if (cmd === "set_task_screens") {
        state.keep = false;
        return Promise.reject(stayed);
      }
      throw new Error(`unexpected ${cmd}`);
    });
    render(<Row />);
    const toggle = screen.getByRole("switch", { name: "Keep task screens" });
    await waitFor(() => expect(toggle).toBeChecked());

    await userEvent.click(toggle);
    const ask = await screen.findByRole("alertdialog");
    await userEvent.click(within(ask).getByRole("button", { name: "Delete kept screens" }));

    expect(await screen.findByText(/1 kept screen could not be deleted/)).toBeInTheDocument();
    await waitFor(() => expect(toggle).not.toBeChecked());
  });

  it("reads the count again as it asks, not as the row was drawn", async () => {
    // #6451's review: screens kept while the page was open are counted in the ask.
    const { state } = aCore(true, 1);
    render(<Row />);
    const toggle = screen.getByRole("switch", { name: "Keep task screens" });
    await waitFor(() => expect(toggle).toBeChecked());
    state.kept = 4;

    await userEvent.click(toggle);

    expect(await screen.findByRole("alertdialog")).toHaveTextContent("4 kept screens");
  });

  it("with nothing kept, asks to turn it off and names no delete", async () => {
    const { set } = aCore(true, 0);
    render(<Row />);
    const toggle = screen.getByRole("switch", { name: "Keep task screens" });
    await waitFor(() => expect(toggle).toBeChecked());

    await userEvent.click(toggle);

    const ask = await screen.findByRole("alertdialog");
    expect(ask).toHaveTextContent("No screen is kept now.");
    expect(within(ask).queryByRole("button", { name: "Delete kept screens" })).toBeNull();
    await userEvent.click(within(ask).getByRole("button", { name: "Turn off" }));
    await waitFor(() => expect(toggle).not.toBeChecked());
    expect(set).toEqual([{ plane: PLANE, keep: false }]);
  });

  it("says why when the setting could not be read at all", async () => {
    mockIPC((cmd) => {
      if (cmd === "task_screens") throw new Error("the window lost its project");
      throw new Error(`unexpected ${cmd}`);
    });
    render(<Row />);

    expect(await screen.findByText(/the window lost its project/)).toBeInTheDocument();
  });
});
