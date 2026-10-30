// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { PoolProfileRow, PoolSignInStep } from "../bindings";
import { askProfiles } from "../pools";
import { PoolProfiles } from "./PoolProfiles";
import { PROFILE_SIGN_IN, type ProfileSignInAsk } from "./links";

/**
 * **A Claude Code profile signs in on a pool's runner from its line, with no terminal** (#4042,
 * spec #4023 seam 4): Sign in opens the line in place into a strip; the core's steps (fed here as
 * the core says them) draw the page opened in the browser, the code field with the keyboard in
 * it, the check, and Signed in; each step is said by its number and to a screen reader.
 */

const PAGE = "https://claude.ai/oauth/authorize?code=true&state=recorded";

const a_profile = (name: string, kind = "claude"): PoolProfileRow => ({
  name,
  kind,
  harness: kind === "claude" ? "Claude Code" : "Codex",
  standing: "signed-out",
  not_kept: null,
  plans: [],
});

let asked: { cmd: string; args: Record<string, unknown> }[];
let profilesAsked: number;
/** The steps the core has said and the window has not yet asked for. */
let said: PoolSignInStep[];
let waiting: ((step: PoolSignInStep) => void) | undefined;

/** The core says `step`. */
function says(step: PoolSignInStep) {
  if (waiting) {
    const answer = waiting;
    waiting = undefined;
    answer(step);
  } else said.push(step);
}

beforeEach(() => {
  asked = [];
  profilesAsked = 0;
  said = [];
  waiting = undefined;
  mockIPC((cmd, args) => {
    const a = (args ?? {}) as Record<string, unknown>;
    asked.push({ cmd, args: a });
    if (cmd === "pool_profiles") {
      profilesAsked += 1;
      return {
        kept: [
          a_profile("work"),
          a_profile("fast", "codex"),
          {
            ...a_profile("open", "opencode"),
            harness: "opencode",
            plans: [
              { id: "openai", title: "ChatGPT" },
              { id: "github-copilot", title: "GitHub Copilot" },
            ],
          },
        ],
        addable: [{ name: "home", harness: "Codex" }],
      };
    }
    if (cmd === "pool_sign_in_start") return 7;
    if (cmd === "pool_sign_in_next") {
      const next = said.shift();
      if (next) return next;
      return new Promise<PoolSignInStep>((answer) => {
        waiting = answer;
      });
    }
    return null;
  });
});
afterEach(() => {
  cleanup();
  clearMocks();
});

const calls = (cmd: string) => asked.filter((one) => one.cmd === cmd).map((one) => one.args);

async function strip(plane?: string) {
  render(<PoolProfiles pool="builds" plane={plane} />);
  await userEvent.click(await screen.findByRole("button", { name: "Sign work in on pool builds" }));
  return screen.findByRole("group", { name: "Signing work in on builds" });
}

describe("Signing a profile in on a pool's runner (#4042)", () => {
  it("opens the page here, takes the pasted code and ends signed in", async () => {
    const opened = await strip();
    expect(calls("pool_sign_in_start")).toEqual([{ pool: "builds", name: "work", provider: null }]);
    expect(within(opened).getByText(/Step 1 of 3/)).toBeInTheDocument();

    says({ at: "open", url: PAGE });
    says({ at: "code" });
    const field = await within(opened).findByRole("textbox", { name: "Paste code" });
    expect(calls("plugin:opener|open_url")).toEqual([expect.objectContaining({ url: PAGE })]);
    expect(within(opened).getByText("Step 2 of 3: Paste the code the page shows")).toBeVisible();
    // The keyboard is in the field, and the step is said to a screen reader.
    expect(field).toHaveFocus();
    expect(within(opened).getByRole("status")).toHaveTextContent(
      "Step 2 of 3: Paste the code the page shows",
    );

    await userEvent.type(field, "abc123#recorded");
    await userEvent.click(within(opened).getByRole("button", { name: "Sign in" }));
    expect(calls("pool_sign_in_paste")).toEqual([{ flow: 7, code: "abc123#recorded" }]);

    says({ at: "checking" });
    expect(await within(opened).findByText(/Step 3 of 3/)).toBeInTheDocument();
    says({ at: "signed-in" });
    expect(await within(opened).findByText("Signed in")).toBeInTheDocument();
    expect(within(opened).getByRole("status")).toHaveTextContent("work is signed in on builds");

    // Closing it asks the runner again, so the line says where it stands now.
    const before = profilesAsked;
    await userEvent.click(within(opened).getByRole("button", { name: "Close" }));
    await waitFor(() => expect(profilesAsked).toBe(before + 1));
    expect(calls("pool_sign_in_cancel")).toEqual([]);
  });

  it("says a refused code in the harness's words and takes another", async () => {
    const opened = await strip();
    says({ at: "open", url: PAGE });
    says({ at: "code" });
    const field = await within(opened).findByRole("textbox", { name: "Paste code" });
    await userEvent.type(field, "wrong{Enter}");
    expect(calls("pool_sign_in_paste")).toEqual([{ flow: 7, code: "wrong" }]);

    says({ at: "refused", said: "Invalid code. That code is not from this sign-in." });

    expect(await within(opened).findByRole("alert")).toHaveTextContent(
      "Invalid code. That code is not from this sign-in.",
    );
    expect(within(opened).getByRole("textbox", { name: "Paste code" })).toBeVisible();
  });

  it("cancels the login on the runner when Cancel is pressed", async () => {
    const opened = await strip();
    says({ at: "open", url: PAGE });
    says({ at: "code" });
    await within(opened).findByRole("textbox", { name: "Paste code" });

    await userEvent.click(within(opened).getByRole("button", { name: "Cancel" }));

    expect(calls("pool_sign_in_cancel")).toEqual([{ flow: 7 }]);
    expect(screen.queryByRole("group", { name: "Signing work in on builds" })).toBeNull();
    expect(screen.getByRole("button", { name: "Sign work in on pool builds" })).toBeVisible();
  });

  it("keeps the sign-in when the runner's profiles are asked again meanwhile", async () => {
    const opened = await strip();
    expect(screen.queryByRole("button", { name: "Ask again" })).toBeNull();

    await askProfiles("builds");

    expect(screen.getByRole("group", { name: "Signing work in on builds" })).toBe(opened);
    expect(calls("pool_sign_in_start")).toHaveLength(1);
    expect(calls("pool_sign_in_cancel")).toEqual([]);
  });

  it("offers neither Add a profile… nor Remove while a sign-in runs on the pool (#4047)", async () => {
    render(<PoolProfiles pool="builds" plane={undefined} />);
    await userEvent.click(
      await screen.findByRole("button", { name: "Remove fast from pool builds" }),
    );
    await userEvent.click(screen.getByRole("button", { name: "Sign work in on pool builds" }));
    await screen.findByRole("group", { name: "Signing work in on builds" });

    expect(screen.queryByRole("button", { name: "Add a profile…" })).toBeNull();
    expect(screen.queryByRole("button", { name: /remove/i })).toBeNull();

    await userEvent.click(screen.getByRole("button", { name: "Cancel" }));

    // The question Remove asked went with the sign-in that was started over it.
    expect(screen.getByRole("button", { name: "Remove fast from pool builds" })).toBeVisible();
    expect(screen.getByRole("button", { name: "Add a profile…" })).toBeVisible();
  });

  it("says what the login printed when purlis cannot follow it, with a terminal the way out", async () => {
    const heard: ProfileSignInAsk[] = [];
    const hear = (event: Event) => heard.push((event as CustomEvent<ProfileSignInAsk>).detail);
    window.addEventListener(PROFILE_SIGN_IN, hear);
    try {
      const opened = await strip("/home/op/project");
      says({ at: "could-not-follow", said: "Choose your sign-in method: [1] [2]" });

      expect(await within(opened).findByRole("alert")).toHaveTextContent(
        "purlis could not follow this sign-in",
      );
      expect(within(opened).getByText("Choose your sign-in method: [1] [2]")).toBeVisible();
      await userEvent.click(within(opened).getByRole("button", { name: "Open in terminal" }));

      expect(heard).toEqual([
        { plane: "/home/op/project", profile: "work", provider: undefined, pool: "builds" },
      ]);
    } finally {
      window.removeEventListener(PROFILE_SIGN_IN, hear);
    }
  });

  it("says why a sign-in failed, and tries again", async () => {
    const opened = await strip();
    says({ at: "failed", why: "the code has expired" });

    expect(await within(opened).findByRole("alert")).toHaveTextContent(
      "work (Claude Code) did not sign in on builds: the code has expired",
    );
    await userEvent.click(within(opened).getByRole("button", { name: "Try again" }));
    await waitFor(() => expect(calls("pool_sign_in_start")).toHaveLength(2));
  });

  it("shows opencode's Copilot device code before any page opens, then signs in (#4045)", async () => {
    const writeText = vi.fn(() => Promise.resolve());
    Object.defineProperty(navigator, "clipboard", {
      configurable: true,
      value: { writeText },
    });
    render(<PoolProfiles pool="builds" plane="plane-1" />);
    await userEvent.click(
      await screen.findByRole("button", { name: "Sign open in to GitHub Copilot on pool builds" }),
    );
    const opened = await screen.findByRole("group", { name: "Signing open in on builds" });
    expect(calls("pool_sign_in_start")).toEqual([
      { pool: "builds", name: "open", provider: "github-copilot" },
    ]);

    const page = "https://github.com/login/device";
    says({ at: "device", url: page, code: "WDJB-MJHT" });

    expect(await within(opened).findByLabelText("The code to enter on the page")).toHaveTextContent(
      "WDJB-MJHT",
    );
    expect(within(opened).getByRole("status")).toHaveTextContent(
      "Step 2 of 3: Enter the code on the page",
    );
    expect(within(opened).getByText(page)).toBeVisible();
    // The code comes first: no browser opens until it is asked for.
    expect(calls("plugin:opener|open_url")).toEqual([]);

    await userEvent.click(within(opened).getByRole("button", { name: "Copy code" }));
    expect(writeText).toHaveBeenCalledWith("WDJB-MJHT");
    expect(await within(opened).findByText("The code is on the clipboard.")).toBeVisible();

    await userEvent.click(within(opened).getByRole("button", { name: "Open page" }));
    expect(calls("plugin:opener|open_url")).toEqual([expect.objectContaining({ url: page })]);

    says({ at: "checking" });
    says({ at: "signed-in" });
    expect(await within(opened).findByText("Signed in")).toBeInTheDocument();
  });

  it("names the command that does the same in a terminal, with its plan (#4046)", async () => {
    render(<PoolProfiles pool="builds" plane="plane-1" />);
    await userEvent.click(
      await screen.findByRole("button", { name: "Sign open in to GitHub Copilot on pool builds" }),
    );
    const opened = await screen.findByRole("group", { name: "Signing open in on builds" });

    expect(
      within(opened).getByText(
        "purlis profile sign-in open --pool builds --provider github-copilot",
      ),
    ).toBeVisible();
  });

  it("keeps opencode's other plan in a shell tab", async () => {
    const asks: ProfileSignInAsk[] = [];
    const hear = (event: Event) => asks.push((event as CustomEvent<ProfileSignInAsk>).detail);
    window.addEventListener(PROFILE_SIGN_IN, hear);
    render(<PoolProfiles pool="builds" plane="plane-1" />);
    await userEvent.click(
      await screen.findByRole("button", { name: "Sign open in to ChatGPT on pool builds" }),
    );
    window.removeEventListener(PROFILE_SIGN_IN, hear);

    expect(calls("pool_sign_in_start")).toEqual([]);
    expect(asks).toEqual([expect.objectContaining({ profile: "open", provider: "openai" })]);
  });

  it("keeps another harness's sign-in in a shell tab", async () => {
    const heard: ProfileSignInAsk[] = [];
    const hear = (event: Event) => heard.push((event as CustomEvent<ProfileSignInAsk>).detail);
    window.addEventListener(PROFILE_SIGN_IN, hear);
    try {
      render(<PoolProfiles pool="builds" plane="/home/op/project" />);
      await userEvent.click(
        await screen.findByRole("button", { name: "Sign fast in on pool builds" }),
      );

      expect(heard).toHaveLength(1);
      expect(calls("pool_sign_in_start")).toEqual([]);
    } finally {
      window.removeEventListener(PROFILE_SIGN_IN, hear);
    }
  });
});
