// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { PoolProfileRow, PoolSignInStep } from "../bindings";
import { ChooseProfiles } from "./PoolChooseProfiles";

/**
 * **Adding a pool ends by choosing the profiles that sign in on it** (#4048, spec #4023 Q4,
 * Q12, prototype Variant A): a strip lists this machine's profiles with the project's default
 * ticked; Sign in N profiles records the ticked ones on the pool and walks each in the same strip,
 * one after another; Not now records nothing and leaves each line offering Sign in.
 */

const a_profile = (name: string, kind = "claude", standing = "signed-out"): PoolProfileRow => ({
  name,
  kind,
  harness: kind === "claude" ? "Claude Code" : "Codex",
  standing,
  not_kept: null,
  plans: [],
});

/** This machine's profiles, as the core says them; the pool keeps `chosen`. */
const ALL = [
  a_profile("work"),
  a_profile("personal"),
  a_profile("oss", "gemini"),
  a_profile("oc", "opencode"),
];
let chosen: string[];
let signedIn: Set<string>;
let changes: unknown[];
let started: string[];
let flows: Map<number, string>;
let waiting: Map<number, (step: PoolSignInStep) => void>;

/** The core says `step` to the sign-in of `profile`, once the strip asks for its next step. */
async function says(profile: string, step: PoolSignInStep) {
  const flow = () => [...flows].find(([at, name]) => name === profile && waiting.has(at))?.[0];
  await waitFor(() => expect(flow()).toBeDefined());
  const at = flow() as number;
  const answer = waiting.get(at);
  waiting.delete(at);
  answer?.(step);
}

beforeEach(() => {
  chosen = ["work"];
  signedIn = new Set();
  changes = [];
  started = [];
  flows = new Map();
  waiting = new Map();
  mockIPC((cmd, args) => {
    const a = (args ?? {}) as Record<string, unknown>;
    if (cmd === "pool_profiles") {
      const standing = (one: PoolProfileRow) => ({
        ...one,
        standing: signedIn.has(one.name) ? "signed-in" : "signed-out",
      });
      return {
        kept: chosen.flatMap((name) => ALL.filter((one) => one.name === name)).map(standing),
        addable: ALL.filter((one) => !chosen.includes(one.name)).map((one) => ({
          name: one.name,
          harness: one.harness,
        })),
      };
    }
    if (cmd === "pool_profile_add") {
      changes.push(["add", a.pool, a.name]);
      chosen = [...chosen, a.name as string];
      return null;
    }
    if (cmd === "pool_profile_remove") {
      changes.push(["remove", a.pool, a.name]);
      chosen = chosen.filter((one) => one !== a.name);
      return null;
    }
    if (cmd === "pool_sign_in_start") {
      started.push(a.name as string);
      const flow = flows.size + 1;
      flows.set(flow, a.name as string);
      return flow;
    }
    if (cmd === "pool_sign_in_next")
      return new Promise<PoolSignInStep>((answer) => {
        waiting.set(a.flow as number, answer);
      });
    return null;
  });
});
afterEach(() => {
  cleanup();
  clearMocks();
});

async function strip() {
  render(<ChooseProfiles pool="builds" plane="/home/op/project" />);
  return screen.findByRole("group", { name: "Profiles that sign in on pool builds" });
}

describe("choosing the profiles a new pool signs in", () => {
  it("lists this machine's profiles with the project's default ticked", async () => {
    const group = await strip();
    expect(within(group).getByRole("checkbox", { name: /work/ })).toBeChecked();
    expect(within(group).getByRole("checkbox", { name: /personal/ })).not.toBeChecked();
    expect(within(group).getByRole("checkbox", { name: /oss/ })).not.toBeChecked();
    expect(within(group).getByText(/default/)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Sign in 1 profile" })).toBeInTheDocument();
  });

  it("records the ticked profiles on the pool and signs them in one after another", async () => {
    const group = await strip();
    await userEvent.click(within(group).getByRole("checkbox", { name: /personal/ }));
    await userEvent.click(screen.getByRole("button", { name: "Sign in 2 profiles" }));
    await waitFor(() => expect(changes).toEqual([["add", "builds", "personal"]]));

    // The first, in the same strip.
    expect(await screen.findByText("Profile 1 of 2: work")).toBeInTheDocument();
    await waitFor(() => expect(started).toEqual(["work"]));
    signedIn.add("work");
    await says("work", { at: "signed-in" });
    expect(await screen.findByText(/work is signed in on builds/)).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Close" }));

    // Then the next.
    expect(await screen.findByText("Profile 2 of 2: personal")).toBeInTheDocument();
    await waitFor(() => expect(started).toEqual(["work", "personal"]));
    signedIn.add("personal");
    await says("personal", { at: "signed-in" });
    await userEvent.click(await screen.findByRole("button", { name: "Close" }));

    // Then the pool's lines, as its row draws them.
    const list = await screen.findByRole("list", { name: "Profiles on pool builds" });
    expect(within(list).getAllByText("Signed in on pool builds.")).toHaveLength(2);
  });

  it("takes an unticked default off the pool", async () => {
    const group = await strip();
    await userEvent.click(within(group).getByRole("checkbox", { name: /work/ }));
    await userEvent.click(within(group).getByRole("checkbox", { name: /personal/ }));
    await userEvent.click(screen.getByRole("button", { name: "Sign in 1 profile" }));
    await waitFor(() =>
      expect(changes).toEqual([
        ["add", "builds", "personal"],
        ["remove", "builds", "work"],
      ]),
    );
    expect(await screen.findByText("Profile 1 of 1: personal")).toBeInTheDocument();
  });

  it("a profile whose sign-in is not driven here is recorded, and its line offers Sign in", async () => {
    const group = await strip();
    await userEvent.click(within(group).getByRole("checkbox", { name: /work/ }));
    await userEvent.click(within(group).getByRole("checkbox", { name: /oss/ }));
    await userEvent.click(screen.getByRole("button", { name: "Sign in 1 profile" }));
    const list = await screen.findByRole("list", { name: "Profiles on pool builds" });
    expect(within(list).getByText("oss")).toBeInTheDocument();
    expect(started).toEqual([]);
  });

  it("leaves an opencode profile to its line, which asks its plan first (#4181)", async () => {
    const group = await strip();
    await userEvent.click(within(group).getByRole("checkbox", { name: /work/ }));
    await userEvent.click(within(group).getByRole("checkbox", { name: /oc/ }));
    await userEvent.click(screen.getByRole("button", { name: "Sign in 1 profile" }));
    const list = await screen.findByRole("list", { name: "Profiles on pool builds" });
    expect(within(list).getByText("oc")).toBeInTheDocument();
    expect(started).toEqual([]);
  });

  it("Not now records nothing and leaves each line offering Sign in", async () => {
    await strip();
    await userEvent.click(screen.getByRole("button", { name: "Not now" }));
    const list = await screen.findByRole("list", { name: "Profiles on pool builds" });
    expect(
      within(list).getByRole("button", { name: "Sign work in on pool builds" }),
    ).toBeInTheDocument();
    expect(changes).toEqual([]);
    expect(started).toEqual([]);
  });

  it("offers nothing to sign in where nothing is ticked", async () => {
    const group = await strip();
    await userEvent.click(within(group).getByRole("checkbox", { name: /work/ }));
    expect(screen.getByRole("button", { name: "Sign in 0 profiles" })).toBeDisabled();
  });

  it("says where the core refused a change, and goes on to the pool's lines", async () => {
    mockIPC((cmd) => {
      if (cmd === "pool_profiles")
        return {
          kept: [a_profile("work")],
          addable: [{ name: "personal", harness: "Claude Code" }],
        };
      if (cmd === "pool_profile_add") throw "pool builds keeps the profile a new chat gets";
      return null;
    });
    const group = await strip();
    await userEvent.click(within(group).getByRole("checkbox", { name: /personal/ }));
    await userEvent.click(within(group).getByRole("checkbox", { name: /work/ }));
    await userEvent.click(screen.getByRole("button", { name: "Sign in 1 profile" }));
    expect(await screen.findByRole("status")).toHaveTextContent(
      /keeps the profile a new chat gets/,
    );
    expect(
      await screen.findByRole("list", { name: "Profiles on pool builds" }),
    ).toBeInTheDocument();
  });
});
