// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, describe, expect, it } from "vitest";
import { cleanup, render, screen, waitFor } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { HIBERNATE_AFTER, hibernateAfterSetting } from "./hibernateAfter";

/**
 * **Settings › Project › Chats › Hibernate after** (SC-4h, #7637): the box shows
 * `[chats] hibernate_after` as the core reads it, a time left or entered is sent to the core as
 * typed, and what the core refuses is said under the box, which keeps the words.
 */

const PLANE = "/home/dev/plane";

afterEach(() => {
  cleanup();
  clearMocks();
});

function Row() {
  const setting = hibernateAfterSetting(PLANE);
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

const UNDER = "5m is under 10 minutes, the least it may be. Set 10m or more, or off.";

/** A core at `after`, refusing 5m as the core does, and what it was asked to set. */
function aCore(after: string) {
  const state = { after };
  const set: unknown[] = [];
  mockIPC((cmd, args) => {
    if (cmd === "hibernate_after") return state.after;
    if (cmd === "set_hibernate_after") {
      set.push(args);
      const asked = (args as { after: string }).after;
      if (asked === "5m") throw UNDER;
      state.after = asked;
      return state.after;
    }
    throw new Error(`unexpected ${cmd}`);
  });
  return { state, set };
}

async function theBox() {
  const box = await screen.findByRole("textbox", { name: "Hibernate after" });
  await waitFor(() => expect(box).not.toBeDisabled());
  return box;
}

describe("the Hibernate after row", () => {
  it("is this machine's, says what it does, its least and off, and never names the file", () => {
    const setting = hibernateAfterSetting(PLANE);
    expect(setting.id).toBe(HIBERNATE_AFTER);
    expect(setting.writes).toBe("local");
    expect(setting.help).toMatch(/at least 10 minutes/);
    expect(setting.help).toMatch(/or off to hibernate none of them/);
    expect(setting.help).toMatch(/Hibernate on a chat's row still works/);
    expect(setting.help).not.toMatch(/\.local\.toml/);
  });

  it("shows what the core reads", async () => {
    aCore("off");
    render(<Row />);
    expect(await theBox()).toHaveValue("off");
  });

  it("sends a time once Enter is pressed, and shows what the core answered", async () => {
    const core = aCore("30m");
    render(<Row />);
    const box = await theBox();
    await userEvent.clear(box);
    await userEvent.type(box, "45m{Enter}");
    await waitFor(() => expect(core.set).toEqual([{ plane: PLANE, after: "45m" }]));
    expect(core.state.after).toBe("45m");
    expect(box).toHaveValue("45m");
  });

  it("says why the core refused a value under the box, and keeps the words", async () => {
    const core = aCore("30m");
    render(<Row />);
    const box = await theBox();
    await userEvent.clear(box);
    await userEvent.type(box, "5m");
    await userEvent.tab();
    expect(await screen.findByText(UNDER)).toBeVisible();
    expect(box).toHaveValue("5m");
    expect(core.state.after).toBe("30m");
  });

  it("sends nothing when the box is left as it was", async () => {
    const core = aCore("30m");
    render(<Row />);
    const box = await theBox();
    await userEvent.click(box);
    await userEvent.tab();
    expect(core.set).toEqual([]);
  });
});
