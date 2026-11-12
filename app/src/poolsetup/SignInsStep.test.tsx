// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { PoolProfiles, PoolSetupSizes, PoolSetupStep } from "../bindings";
import { STAYS_ON_THE_RUNNER, SignInsStep } from "./SignInsStep";

const STEP: PoolSetupStep = {
  id: "signins",
  name: "Sign-ins",
  state: "current",
  line: null,
  findings: [],
  next: { id: "choose-profiles", label: "Choose profiles", does: "", command: null, url: null },
  skippable: true,
  see: null,
  other: null,
  machine: null,
  cause: null,
  checks: [],
};

const row = (name: string, standing: string | null = "signed-out") => ({
  name,
  kind: "claude",
  harness: "Claude Code",
  standing,
  not_kept: null,
  limit: null,
  plans: [],
});

const PROFILES: PoolProfiles = {
  kept: [row("work", "signed-in")],
  addable: [
    { name: "personal", harness: "Claude Code" },
    { name: "oss", harness: "Codex" },
  ],
  fresh_claim: false,
};

const size = (
  name: string,
  mib: number,
  in_use = false,
  over: PoolSetupSizes["profiles"][0]["over"] = [],
) => ({
  name,
  mib,
  files: 100,
  unread: null,
  over,
  in_use,
});

const SIZES: PoolSetupSizes = {
  most_mib: 128,
  most_files: 8192,
  profiles: [size("work", 42, true), size("personal", 18, true), size("oss", 9)],
};

type Calls = { cmd: string; args: unknown }[];

function core(more: Record<string, (args: unknown) => unknown> = {}) {
  const calls: Calls = [];
  mockIPC((cmd, args) => {
    calls.push({ cmd, args });
    if (cmd in more) return more[cmd]?.(args);
    if (cmd === "pool_profiles") return PROFILES;
    if (cmd === "pool_setup_sizes") return SIZES;
    return null;
  });
  return calls;
}

let pools = 0;
/** A pool of its own per test: the window's pools are held across a test file. */
const draw = (step: PoolSetupStep = STEP, changed = vi.fn()) => {
  const pool = `pool-${++pools}`;
  render(<SignInsStep step={step} pool={pool} plane="p" changed={changed} />);
  return { pool, changed };
};

afterEach(() => {
  cleanup();
  clearMocks();
});

describe("SignInsStep", () => {
  it("ticks the profiles the pool keeps and those in use, and not the rest", async () => {
    core();
    draw();
    const list = await screen.findByRole("list", { name: "Your profiles" });
    const box = (name: string) =>
      within(list).getByRole("checkbox", { name: new RegExp(`^${name}`) }) as HTMLInputElement;
    await waitFor(() => expect(box("work").checked).toBe(true));
    expect(box("personal").checked).toBe(true);
    expect(box("oss").checked).toBe(false);
  });

  it("shows each profile's size against the pool's limit", async () => {
    core();
    draw();
    expect(await screen.findByText("42 of 128 MiB")).toBeTruthy();
    expect(screen.getByText("9 of 128 MiB")).toBeTruthy();
  });

  it("offers to raise a limit in place for a profile over it, and measures again", async () => {
    let sizes = {
      ...SIZES,
      profiles: [
        size("work", 212, true, [
          {
            key: "profileSize",
            said: "it needs 212 MiB, over 128 MiB",
            raise_to: 256,
            older_runner: null,
          },
        ]),
      ],
    };
    const calls = core({
      pool_setup_sizes: () => sizes,
      pool_limit_set: () => {
        sizes = { ...SIZES, most_mib: 256, profiles: [size("work", 212, true)] };
        return null;
      },
    });
    const { pool } = draw();
    expect(await screen.findByText("work is too big to send")).toBeTruthy();
    expect(screen.queryByRole("button", { name: /^Sign in/ })).toBeNull();
    await userEvent.click(screen.getByRole("button", { name: "Raise the limit to 256 MiB" }));
    await waitFor(() => expect(screen.queryByText("work is too big to send")).toBeNull());
    expect(calls.find((call) => call.cmd === "pool_limit_set")?.args).toEqual({
      pool,
      key: "profileSize",
      value: 256,
    });
    expect(screen.getByText("212 of 256 MiB")).toBeTruthy();
  });

  it("leaves a profile over its limit out, and carries on", async () => {
    core({
      pool_setup_sizes: () => ({
        ...SIZES,
        profiles: [
          size("work", 42, true),
          size("personal", 212, true, [
            { key: "profileSize", said: "too big", raise_to: 256, older_runner: null },
          ]),
        ],
      }),
    });
    draw();
    await userEvent.click(await screen.findByRole("button", { name: "Leave this profile out" }));
    expect(screen.queryByText("personal is too big to send")).toBeNull();
    expect(screen.getByRole("button", { name: "Continue" })).toBeTruthy();
  });

  it("records the ticked profiles and signs each in inside the step", async () => {
    let added = false;
    const calls = core({
      pool_profiles: () =>
        added
          ? {
              ...PROFILES,
              kept: [...PROFILES.kept, row("personal")],
              addable: [PROFILES.addable[1]],
            }
          : PROFILES,
      pool_profile_add: () => {
        added = true;
        return null;
      },
      pool_sign_in_start: () => 7,
      pool_sign_in_next: () => new Promise(() => undefined),
    });
    const { pool } = draw();
    await userEvent.click(await screen.findByRole("button", { name: "Sign in 1 profile" }));
    expect(await screen.findByRole("heading", { name: "Sign in personal" })).toBeTruthy();
    expect(calls.find((call) => call.cmd === "pool_profile_add")?.args).toMatchObject({
      pool,
      name: "personal",
    });
    expect(calls.find((call) => call.cmd === "pool_sign_in_start")?.args).toMatchObject({
      pool,
      name: "personal",
    });
    expect(calls.some((call) => call.cmd === "pool_profile_remove")).toBe(false);
  });

  it("offers no main action with nothing ticked, and says what to do", async () => {
    core({
      pool_setup_sizes: () => ({
        ...SIZES,
        profiles: SIZES.profiles.map((one) => ({ ...one, in_use: false })),
      }),
      pool_profiles: () => ({
        ...PROFILES,
        kept: [],
        addable: [...PROFILES.addable, { name: "work", harness: "Claude Code" }],
      }),
    });
    draw();
    expect(await screen.findByText(/Tick a profile for chats on/)).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Continue" })).toBeNull();
    expect(screen.getByRole("button", { name: "Skip for now" })).toBeTruthy();
  });

  it("wakes a sleeping runner on its own button", async () => {
    const calls = core({ pool_setup_wake: () => null });
    const { pool, changed } = draw({
      ...STEP,
      findings: ["The runner isn't running."],
      next: {
        id: "wake-runner",
        label: "Wake the runner",
        does: "Starts it.",
        command: null,
        url: null,
      },
    });
    await userEvent.click(await screen.findByRole("button", { name: "Wake the runner" }));
    await waitFor(() => expect(changed).toHaveBeenCalled());
    expect(calls.find((call) => call.cmd === "pool_setup_wake")?.args).toEqual({ pool });
  });

  it("is skipped on its own button, and that can be taken back", async () => {
    const calls = core();
    const { pool, changed } = draw();
    await userEvent.click(await screen.findByRole("button", { name: "Skip for now" }));
    await waitFor(() => expect(changed).toHaveBeenCalled());
    expect(calls.find((call) => call.cmd === "pool_setup_skip")?.args).toEqual({
      pool,
      step: "signins",
      skipped: true,
    });
    cleanup();
    const back = draw({ ...STEP, state: "skipped", next: null });
    expect(screen.getByText("You skipped this for now")).toBeTruthy();
    await userEvent.click(screen.getByRole("button", { name: "Choose profiles now" }));
    await waitFor(() => expect(back.changed).toHaveBeenCalled());
    expect(calls.filter((call) => call.cmd === "pool_setup_skip").pop()?.args).toMatchObject({
      skipped: false,
    });
  });

  it("says where a sign-in is kept on a pool whose runner keeps nothing past a claim", async () => {
    core({ pool_profiles: () => ({ ...PROFILES, fresh_claim: true }) });
    draw();
    expect(await screen.findByText(STAYS_ON_THE_RUNNER)).toBeTruthy();
  });

  it("says what the core found where the list can't help, with a way on", async () => {
    core();
    draw({
      ...STEP,
      findings: ["The runner of pool dev-box isn't running, so purlis can't read its sign-ins."],
      next: {
        id: "wake-runner",
        label: "Wake the runner",
        does: "Starts it.",
        command: null,
        url: null,
      },
    });
    expect(await screen.findByText(/isn't running/)).toBeTruthy();
    expect(screen.getByRole("button", { name: "Skip for now" })).toBeTruthy();
  });

  it("says a kept profile unticked is taken off by hand, with its own line (#5174)", async () => {
    const calls = core({
      pool_setup_sign_ins_by_hand: (args) => {
        const { takingOff } = args as { takingOff: string[] };
        return {
          note: "Run these yourself if you prefer.",
          text: takingOff.map((name) => `purlis pool remove-profile P ${name}`).join("\n"),
        };
      },
    });
    draw();
    await userEvent.click(await screen.findByRole("button", { name: "See what this does" }));
    const list = screen.getByRole("list", { name: "Your profiles" });
    await userEvent.click(within(list).getByRole("checkbox", { name: /^work/ }));
    expect(await screen.findByText(/purlis pool remove-profile P work/)).toBeTruthy();
    const asked = calls.filter((call) => call.cmd === "pool_setup_sign_ins_by_hand").at(-1);
    expect(asked?.args).toMatchObject({ takingOff: ["work"] });
  });

  it("shows no commands where the core spells none, only why (#5349)", async () => {
    core({
      pool_setup_sign_ins_by_hand: () => ({
        note: "Only the button takes this one off.",
        text: "",
      }),
    });
    draw();
    await userEvent.click(await screen.findByRole("button", { name: "See what this does" }));
    expect(await screen.findByText("Only the button takes this one off.")).toBeTruthy();
    expect(screen.queryByText("Commands")).toBeNull();
  });

  it("offers a new runner where only the runner's older purlis holds a profile (#5175)", async () => {
    let sizes: PoolSetupSizes = {
      ...SIZES,
      profiles: [
        size("work", 212, true, [
          {
            key: "profileSize",
            said: "it needs 212 MiB, and this pool's runner runs an older purlis",
            raise_to: null,
            older_runner: "move",
          },
        ]),
      ],
    };
    const calls = core({
      pool_setup_sizes: () => sizes,
      pool_setup_move_runner: () => {
        sizes = { ...SIZES, profiles: [size("work", 212, true)] };
        return null;
      },
    });
    const { pool, changed } = draw();
    expect(await screen.findByText(/runs an older purlis/)).toBeTruthy();
    expect(screen.queryByRole("button", { name: /^Raise the limit/ })).toBeNull();
    expect(screen.queryByText(/Raising the limit/)).toBeNull();
    await userEvent.click(screen.getByRole("button", { name: "Replace the runner…" }));
    expect(calls.some((call) => call.cmd === "pool_setup_move_runner")).toBe(false);
    await userEvent.click(await screen.findByRole("button", { name: "Replace the runner" }));
    await waitFor(() =>
      expect(calls.find((call) => call.cmd === "pool_setup_move_runner")?.args).toEqual({ pool }),
    );
    await waitFor(() => expect(screen.queryByText(/runs an older purlis/)).toBeNull());
    expect(changed).toHaveBeenCalled();
  });

  it("says what helps where purlis can't move the pool's image (#5175)", async () => {
    core({
      pool_setup_sizes: () => ({
        ...SIZES,
        profiles: [
          size("work", 212, true, [
            {
              key: "profileSize",
              said: "it runs an older purlis",
              raise_to: null,
              older_runner: "own-image",
            },
          ]),
        ],
      }),
    });
    draw();
    expect(await screen.findByText(/remove the pool, then add it again/)).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Replace the runner…" })).toBeNull();
  });

  it("shows by hand what its button records and signs in, for the profiles ticked (#4718)", async () => {
    const calls = core({
      pool_setup_sign_ins_by_hand: (args) => {
        const { adding, signingIn } = args as { adding: string[]; signingIn: string[] };
        return {
          note: "Run these yourself if you prefer.",
          text: [`purlis pool add-profile P ${adding.join(" ")}`, ...signingIn].join("\n"),
        };
      },
    });
    draw();
    await userEvent.click(await screen.findByRole("button", { name: "See what this does" }));
    expect(await screen.findByText(/purlis pool add-profile P personal/)).toBeTruthy();
    const list = screen.getByRole("list", { name: "Your profiles" });
    await userEvent.click(within(list).getByRole("checkbox", { name: /^oss/ }));
    expect(await screen.findByText(/purlis pool add-profile P personal oss/)).toBeTruthy();
    const asked = calls.filter((call) => call.cmd === "pool_setup_sign_ins_by_hand").at(-1);
    expect(asked?.args).toMatchObject({
      adding: ["personal", "oss"],
      signingIn: ["personal", "oss"],
    });
  });

  it("asks the runner again when the core reads the step anew, so work done by hand shows (#4718)", async () => {
    const calls = core();
    const pool = `pool-${++pools}`;
    const { rerender } = render(
      <SignInsStep step={STEP} pool={pool} plane="p" changed={vi.fn()} />,
    );
    await screen.findByRole("list", { name: "Your profiles" });
    const before = calls.filter((call) => call.cmd === "pool_profiles").length;
    rerender(
      <SignInsStep
        step={{ ...STEP, findings: ["The runner doesn't keep the profile personal yet."] }}
        pool={pool}
        plane="p"
        changed={vi.fn()}
      />,
    );
    await waitFor(() =>
      expect(calls.filter((call) => call.cmd === "pool_profiles").length).toBeGreaterThan(before),
    );
  });

  it("shows nothing by hand where its button would run nothing, not the core's older read (#4718)", async () => {
    core({ pool_setup_sign_ins_by_hand: () => null });
    draw({ ...STEP, see: { note: "n", text: "purlis profile sign-in oss --pool P" } });
    await screen.findByRole("list", { name: "Your profiles" });
    await waitFor(() =>
      expect(screen.queryByRole("button", { name: "See what this does" })).toBeNull(),
    );
  });
});
