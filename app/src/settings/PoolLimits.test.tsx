// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { PoolLimitRow, PoolLimitsPage } from "../bindings";
import { PoolLimits } from "./PoolLimits";

/**
 * **A pool's limits** (#4285): what a person sees and does on the page, over a faked core that
 * holds values and checks them as the real one does. Its limits are made up here: the page names
 * none, and draws what it is handed.
 */

const row = (over: Partial<PoolLimitRow> & { key: string }): PoolLimitRow => ({
  plain: `Limit ${over.key}`,
  unit: "MiB",
  default: 128,
  default_said: "128 MiB",
  set: null,
  applies: null,
  ...over,
});

let values: Record<string, number>;
let written: { key: unknown; value: unknown }[];

function theCore(refuseSave?: string) {
  mockIPC((cmd, args) => {
    const asked = (args ?? {}) as { key: string; typed: string; value: number | null };
    const withValue = (one: PoolLimitRow) => ({ ...one, set: values[one.key] ?? null });
    if (cmd === "pool_limits")
      return {
        groups: [
          {
            label: "Sign-in",
            help: "Profiles going to the runner.",
            applies: "Applies from the next attempt.",
            limits: [withValue(row({ key: "first" })), withValue(row({ key: "second" }))],
          },
          {
            label: "Runner",
            help: "What the runner is made with.",
            applies: null,
            limits: [
              withValue(
                row({ key: "shaping", applies: "Applies when this runner is next replaced." }),
              ),
            ],
          },
        ],
        advanced: [withValue(row({ key: "rare", applies: "Applies from the next attempt." }))],
      } satisfies PoolLimitsPage;
    if (cmd === "pool_limit_check") {
      if (!/^\d+$/.test(asked.typed.trim())) throw `${asked.key} is a whole number`;
      const value = Number(asked.typed);
      if (value < 1 || value > 1024) throw `${asked.key} is at most 1024 MiB`;
      return value;
    }
    if (cmd === "pool_limit_set") {
      if (refuseSave !== undefined) throw refuseSave;
      written.push({ key: asked.key, value: asked.value });
      if (asked.value === null) Reflect.deleteProperty(values, asked.key);
      else values[asked.key] = asked.value;
      return null;
    }
    return null;
  });
}

beforeEach(() => {
  values = {};
  written = [];
  theCore();
});
afterEach(() => {
  cleanup();
  clearMocks();
});

async function opened(pool: string) {
  const user = userEvent.setup();
  render(<PoolLimits pool={pool} />);
  await user.click(screen.getByRole("button", { name: "Limits" }));
  await screen.findByRole("region", { name: "Sign-in" });
  return user;
}
const box = (key: string) => screen.getByRole("textbox", { name: `Limit ${key} MiB` });

describe("a pool's limits page", () => {
  it("groups what the core lists, with Advanced folded, and says when each applies", async () => {
    const user = await opened("groups");
    const signIn = screen.getByRole("region", { name: "Sign-in" });
    expect(within(signIn).getByText(/Applies from the next attempt/)).toBeTruthy();
    expect(screen.getByRole("heading", { name: "Runner" })).toBeTruthy();
    expect(screen.getByText(/Applies when this runner is next replaced/)).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "Advanced" }));
    expect(box("rare")).toBeTruthy();
  });

  it("shows a limit left alone empty, following its default, with nothing to reset", async () => {
    await opened("unset");
    expect((box("first") as HTMLInputElement).value).toBe("");
    expect(box("first").getAttribute("placeholder")).toBe("128");
    expect(box("first").getAttribute("aria-describedby")).toBeTruthy();
    expect(screen.getAllByText(/Default 128 MiB/).length).toBeGreaterThan(0);
    expect(screen.queryByText("You set this")).toBeNull();
    const reset = screen.getByRole("button", { name: "Reset to 128 MiB: Limit first" });
    expect((reset as HTMLButtonElement).disabled).toBe(true);
  });

  it("refuses a bad value as it is typed, in the core's words, and saves nothing", async () => {
    const user = await opened("bad");
    await user.type(box("first"), "5000");
    expect(await screen.findByText("first is at most 1024 MiB")).toBeTruthy();
    expect(box("first").getAttribute("aria-invalid")).toBe("true");
    await user.tab();
    expect(written).toEqual([]);
    expect((box("first") as HTMLInputElement).value).toBe("5000");
  });

  it("saves a good value on leaving the box, and marks it as the person's", async () => {
    const user = await opened("good");
    await user.type(box("first"), "256");
    await user.tab();
    await screen.findByText("You set this");
    expect(written).toEqual([{ key: "first", value: 256 }]);
    expect((box("first") as HTMLInputElement).value).toBe("256");
    expect(screen.getByText("1 set by you")).toBeTruthy();
  });

  it("saves on Enter, the keyboard's way", async () => {
    const user = await opened("enter");
    await user.type(box("first"), "300{Enter}");
    await waitFor(() => expect(written).toEqual([{ key: "first", value: 300 }]));
  });

  it("returns a limit to following the default on Reset, and on emptying its box", async () => {
    values = { first: 256, second: 512 };
    const user = await opened("reset");
    await waitFor(() => expect((box("first") as HTMLInputElement).value).toBe("256"));
    await user.click(screen.getByRole("button", { name: "Reset to 128 MiB: Limit first" }));
    await waitFor(() => expect(written).toEqual([{ key: "first", value: null }]));
    await waitFor(() => expect((box("first") as HTMLInputElement).value).toBe(""));
    expect(document.activeElement).toBe(box("first"));

    await user.clear(box("second"));
    await user.tab();
    await waitFor(() => expect(written).toContainEqual({ key: "second", value: null }));
    await waitFor(() => expect(screen.queryByText("You set this")).toBeNull());
  });

  it("clears a refused value on a limit never set with Reset, writing nothing", async () => {
    const user = await opened("clear");
    await user.type(box("first"), "abc");
    await screen.findByText("first is a whole number");
    await user.click(screen.getByRole("button", { name: "Reset to 128 MiB: Limit first" }));
    expect((box("first") as HTMLInputElement).value).toBe("");
    expect(screen.queryByText("first is a whole number")).toBeNull();
    expect(written).toEqual([]);
  });

  it("says why a save was refused, and keeps what was typed", async () => {
    const user = await opened("refused");
    theCore("there is no pool named refused on this machine");
    await user.type(box("first"), "256");
    await user.tab();
    expect(await screen.findByText(/there is no pool named refused/)).toBeTruthy();
    expect((box("first") as HTMLInputElement).value).toBe("256");
  });

  it("says nothing of timeouts, quotas or config", async () => {
    await opened("words");
    const words = document.body.textContent ?? "";
    expect(words).not.toMatch(/timeout|quota|config/i);
  });
});
