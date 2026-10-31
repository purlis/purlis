// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useState } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, render, screen, waitFor, within } from "@testing-library/react";
import { userEvent } from "@testing-library/user-event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { PoolProfileRow, PoolSignInStep } from "../bindings";
import { askProfiles } from "../pools";
import { PoolProfiles } from "./PoolProfiles";
import { PROFILE_SIGN_IN, PROFILE_SIGN_OUT, type ProfileSignInAsk } from "./links";
import type { Printer } from "./SignInTerminal";

/** xterm draws on a page; here the strip's terminal is what it prints and one key typed. */
vi.mock("./SignInTerminal", () => ({
  SignInTerminal: function Stood({
    label,
    printer,
    onType,
    live,
  }: {
    label: string;
    printer: Printer;
    onType: (text: string) => void;
    live: boolean;
  }) {
    const [printed, setPrinted] = useState("");
    useEffect(() => {
      printer((text) => setPrinted((before) => before + text));
      return () => printer(undefined);
    }, [printer]);
    return (
      <div role="group" aria-label={label}>
        <pre>{printed}</pre>
        <button onClick={() => live && onType("1\r")}>Type 1</button>
      </div>
    );
  },
}));

/**
 * **A Claude Code profile signs in on a pool's runner from its line, with no terminal** (#4042,
 * spec #4023 seam 4): Sign in opens the line in place into a strip; the core's steps (fed here as
 * the core says them) draw the page opened in the browser, the code field with the keyboard in
 * it, the check, and Signed in; each step is said by its number and to a screen reader.
 */

const PAGE = "https://claude.ai/oauth/authorize?code=true&state=recorded";
const OPENAI = "https://auth.openai.com/oauth/authorize?state=recorded";

const a_profile = (name: string, kind = "claude"): PoolProfileRow => ({
  name,
  kind,
  harness: kind === "claude" ? "Claude Code" : "Codex",
  standing: signedIn ? "signed-in" : "signed-out",
  not_kept: null,
  plans: [],
});

let asked: { cmd: string; args: Record<string, unknown> }[];
let profilesAsked: number;
/** Whether the core says the profiles are signed in. */
let signedIn = false;
/** The steps the core has said and the window has not yet asked for. */
let said: PoolSignInStep[];
let waiting: ((step: PoolSignInStep) => void) | undefined;
/** What each typed call waits on, in turn, where a test holds them; an error refuses it. */
let typing: Promise<void>[];

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
  typing = [];
  mockIPC((cmd, args) => {
    const a = (args ?? {}) as Record<string, unknown>;
    asked.push({ cmd, args: a });
    if (cmd === "pool_profiles") {
      profilesAsked += 1;
      return {
        kept: [
          a_profile("work"),
          a_profile("fast", "codex"),
          { ...a_profile("gem", "gemini"), harness: "Gemini" },
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
    if (cmd === "pool_sign_in_type") return typing.shift() ?? null;
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
    expect(calls("pool_sign_in_start")).toEqual([
      { pool: "builds", name: "work", provider: null, terminal: null },
    ]);
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

  it("hands an unrecognised login over to the runner's terminal, then checks it (#4043)", async () => {
    const opened = await strip();

    says({ at: "could-not-follow", said: "Choose your sign-in method: [1] [2]" });

    const terminal = await within(opened).findByRole("group", {
      name: "The runner's terminal on builds",
    });
    expect(within(opened).getByRole("status")).toHaveTextContent(
      "purlis could not follow this sign-in: the runner's own terminal is open below",
    );
    expect(await screen.findByText("Needs you in the terminal")).toBeVisible();
    await waitFor(() =>
      expect(calls("pool_sign_in_start")).toEqual([
        { pool: "builds", name: "work", provider: null, terminal: null },
        { pool: "builds", name: "work", provider: null, terminal: { columns: 80, rows: 16 } },
      ]),
    );

    says({ at: "printed", text: "Which account? " });
    expect(await within(terminal).findByText("Which account?")).toBeVisible();
    await userEvent.click(within(terminal).getByRole("button", { name: "Type 1" }));
    expect(calls("pool_sign_in_type")).toEqual([{ flow: 7, text: "1\r" }]);

    says({ at: "checking" });
    says({ at: "signed-in" });
    expect(await within(opened).findByText("Signed in")).toBeInTheDocument();
  });

  it("offers the terminal's Cancel, which ends the login, and Open in terminal (#4043)", async () => {
    const heard: ProfileSignInAsk[] = [];
    const hear = (event: Event) => heard.push((event as CustomEvent<ProfileSignInAsk>).detail);
    window.addEventListener(PROFILE_SIGN_IN, hear);
    try {
      const opened = await strip("/home/op/project");
      says({ at: "could-not-follow", said: "Choose your sign-in method: [1] [2]" });
      await within(opened).findByRole("group", { name: "The runner's terminal on builds" });
      await waitFor(() => expect(calls("pool_sign_in_start")).toHaveLength(2));

      await userEvent.click(within(opened).getByRole("button", { name: "Open in terminal" }));

      expect(heard).toEqual([
        { plane: "/home/op/project", profile: "work", provider: undefined, pool: "builds" },
      ]);
      expect(calls("pool_sign_in_cancel")).toEqual([{ flow: 7 }]);
    } finally {
      window.removeEventListener(PROFILE_SIGN_IN, hear);
    }
  });

  it("ends the login in the terminal on Cancel, and Try again stays in the terminal (#4043)", async () => {
    const opened = await strip();
    says({ at: "could-not-follow", said: "Choose your sign-in method: [1] [2]" });
    await within(opened).findByRole("group", { name: "The runner's terminal on builds" });
    await waitFor(() => expect(calls("pool_sign_in_start")).toHaveLength(2));
    says({ at: "failed", why: "the runner said: no" });

    await userEvent.click(await within(opened).findByRole("button", { name: "Try again" }));

    await waitFor(() => expect(calls("pool_sign_in_start")).toHaveLength(3));
    expect(calls("pool_sign_in_start")[2]).toEqual(
      expect.objectContaining({ terminal: { columns: 80, rows: 16 } }),
    );
    // The terminal takes typing again, and the line says it needs the person.
    expect(await screen.findByText("Needs you in the terminal")).toBeVisible();
    await userEvent.click(within(opened).getByRole("button", { name: "Type 1" }));
    expect(calls("pool_sign_in_type")).toEqual([{ flow: 7, text: "1\r" }]);
    await userEvent.click(within(opened).getByRole("button", { name: "Cancel" }));
    expect(calls("pool_sign_in_cancel")).toEqual([{ flow: 7 }]);
  });

  it("passes what is typed on one call at a time, in order (#4123)", async () => {
    const opened = await strip();
    says({ at: "could-not-follow", said: "Choose your sign-in method: [1] [2]" });
    const terminal = await within(opened).findByRole("group", {
      name: "The runner's terminal on builds",
    });
    await waitFor(() => expect(calls("pool_sign_in_start")).toHaveLength(2));
    let letFirstGo = () => {};
    typing = [new Promise((go) => (letFirstGo = go))];
    const type = within(terminal).getByRole("button", { name: "Type 1" });

    await userEvent.click(type);
    await userEvent.click(type);
    await waitFor(() => expect(calls("pool_sign_in_type")).toHaveLength(1));
    // The second waits for the first to have been passed on.
    await act(() => new Promise((done) => setTimeout(done, 20)));
    expect(calls("pool_sign_in_type")).toHaveLength(1);

    letFirstGo();

    await waitFor(() => expect(calls("pool_sign_in_type")).toHaveLength(2));
  });

  it("carries on typing after a key refused, and says why (#4123)", async () => {
    const opened = await strip();
    says({ at: "could-not-follow", said: "Choose your sign-in method: [1] [2]" });
    const terminal = await within(opened).findByRole("group", {
      name: "The runner's terminal on builds",
    });
    await waitFor(() => expect(calls("pool_sign_in_start")).toHaveLength(2));
    const refused = Promise.reject(new Error("that is too much to type"));
    refused.catch(() => undefined);
    typing = [refused];
    const type = within(terminal).getByRole("button", { name: "Type 1" });

    await userEvent.click(type);
    expect(await within(opened).findByRole("alert")).toHaveTextContent("too much to type");

    await userEvent.click(type);
    await waitFor(() => expect(calls("pool_sign_in_type")).toHaveLength(2));
  });

  it("clears what it said of a refused key once the next goes through (#4123)", async () => {
    const opened = await strip();
    says({ at: "could-not-follow", said: "Choose your sign-in method: [1] [2]" });
    const terminal = await within(opened).findByRole("group", {
      name: "The runner's terminal on builds",
    });
    await waitFor(() => expect(calls("pool_sign_in_start")).toHaveLength(2));
    const refused = Promise.reject(new Error("that is too much to type"));
    refused.catch(() => undefined);
    typing = [refused];
    const type = within(terminal).getByRole("button", { name: "Type 1" });
    await userEvent.click(type);
    await within(opened).findByRole("alert");

    await userEvent.click(type);

    await waitFor(() => expect(within(opened).queryByRole("alert")).toBeNull());
  });

  it("a device code that failed may go back to the browser, on a screen afresh (#4106)", async () => {
    render(<PoolProfiles pool="builds" plane={undefined} />);
    await userEvent.click(
      await screen.findByRole("button", { name: "Sign fast in on pool builds" }),
    );
    const opened = await screen.findByRole("group", { name: "Signing fast in on builds" });
    says({ at: "port-taken", port: 1455 });
    await userEvent.click(await within(opened).findByRole("button", { name: "Use a device code" }));
    expect(
      await within(opened).findByText(/Enter the code only on a page of openai.com/),
    ).toBeVisible();
    const terminal = await within(opened).findByRole("group", {
      name: "The runner's terminal on builds",
    });
    await waitFor(() => expect(calls("pool_sign_in_start")).toHaveLength(2));
    says({ at: "printed", text: "Enter OLD-CODE" });
    await within(terminal).findByText("Enter OLD-CODE");
    says({ at: "failed", why: "the runner said: device auth is off" });

    await userEvent.click(await within(opened).findByRole("button", { name: "Try again" }));
    await waitFor(() => expect(calls("pool_sign_in_start")).toHaveLength(3));
    expect(within(opened).queryByText("Enter OLD-CODE")).toBeNull();

    says({ at: "failed", why: "the runner said: device auth is off" });
    await userEvent.click(await within(opened).findByRole("button", { name: "Try the browser" }));

    await waitFor(() => expect(calls("pool_sign_in_start")).toHaveLength(4));
    expect(calls("pool_sign_in_start")[3]).toEqual(expect.objectContaining({ terminal: null }));
    expect(
      within(opened).queryByRole("group", { name: "The runner's terminal on builds" }),
    ).toBeNull();
  });

  it("starts a Copilot sign-in purlis could not follow again in the terminal, with its plan (#4160)", async () => {
    render(<PoolProfiles pool="builds" plane={undefined} />);
    await userEvent.click(
      await screen.findByRole("button", { name: "Sign open in to GitHub Copilot on pool builds" }),
    );
    const opened = await screen.findByRole("group", { name: "Signing open in on builds" });

    says({ at: "could-not-follow", said: "Select GitHub deployment type" });

    await within(opened).findByRole("group", { name: "The runner's terminal on builds" });
    await waitFor(() =>
      expect(calls("pool_sign_in_start")).toEqual([
        { pool: "builds", name: "open", provider: "github-copilot", terminal: null },
        {
          pool: "builds",
          name: "open",
          provider: "github-copilot",
          terminal: { columns: 80, rows: 16 },
        },
      ]),
    );
  });

  it("offers no Cancel while it is checked, as the login has ended (#4043)", async () => {
    const opened = await strip("/home/op/project");
    says({ at: "checking" });

    expect(await within(opened).findByText(/Step 3 of 3/)).toBeInTheDocument();
    expect(within(opened).queryByRole("button", { name: "Cancel" })).toBeNull();
    // Closing it now would throw away a login that may have signed in.
    expect(within(opened).queryByRole("button", { name: "Open in terminal" })).toBeNull();
  });

  it("says why a sign-in failed and starts another by opening the page again (#4043)", async () => {
    const opened = await strip();
    says({ at: "open", url: PAGE });
    says({ at: "failed", why: "the code has expired" });

    expect(await within(opened).findByRole("alert")).toHaveTextContent(
      "work (Claude Code) did not sign in on builds: the code has expired",
    );
    await userEvent.click(within(opened).getByRole("button", { name: "Open the page again" }));
    await waitFor(() => expect(calls("pool_sign_in_start")).toHaveLength(2));
  });

  it("shows opencode's Copilot device code before any page opens, then signs in (#4045)", async () => {
    const writeText = vi.fn(() => Promise.resolve());
    const before = Object.getOwnPropertyDescriptor(navigator, "clipboard");
    Object.defineProperty(navigator, "clipboard", {
      configurable: true,
      value: { writeText },
    });
    try {
      render(<PoolProfiles pool="builds" plane="plane-1" />);
      await userEvent.click(
        await screen.findByRole("button", {
          name: "Sign open in to GitHub Copilot on pool builds",
        }),
      );
      const opened = await screen.findByRole("group", { name: "Signing open in on builds" });
      expect(calls("pool_sign_in_start")).toEqual([
        { pool: "builds", name: "open", provider: "github-copilot", terminal: null },
      ]);
      // No page is said to open as it starts: the code comes first.
      expect(
        within(opened).getByText("Step 1 of 3: Starting the sign-in on the runner"),
      ).toBeVisible();

      const page = "https://github.com/login/device";
      says({ at: "device", url: page, code: "WDJB-MJHT" });

      expect(
        await within(opened).findByLabelText("The code to enter on the page"),
      ).toHaveTextContent("WDJB-MJHT");
      expect(within(opened).getByText("Step 2 of 3: Enter the code on the page")).toBeVisible();
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
    } finally {
      if (before) Object.defineProperty(navigator, "clipboard", before);
      else Reflect.deleteProperty(navigator, "clipboard");
    }
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

  it("offers Try again where no page was opened, and the line stays failed (#4043)", async () => {
    const opened = await strip();
    says({ at: "failed", why: "the runner stopped answering" });
    await within(opened).findByRole("button", { name: "Try again" });

    await userEvent.click(within(opened).getByRole("button", { name: "Close" }));

    expect(await screen.findByText("Sign-in failed")).toBeVisible();
  });

  it("asks before Sign out, then runs it on the runner and asks where it stands (#4043)", async () => {
    signedIn = true;
    await act(() => askProfiles("builds"));
    try {
      render(<PoolProfiles pool="builds" plane={undefined} />);
      await userEvent.click(
        await screen.findByRole("button", { name: "Sign out work on pool builds" }),
      );

      expect(screen.getByText(/^Sign work out on pool builds\?/)).toBeVisible();
      await userEvent.click(
        screen.getByRole("button", { name: "Keep work signed in on pool builds" }),
      );
      expect(calls("pool_sign_out")).toEqual([]);

      await userEvent.click(screen.getByRole("button", { name: "Sign out work on pool builds" }));
      const before = profilesAsked;
      await userEvent.click(
        screen.getByRole("button", { name: "Yes, sign out work on pool builds" }),
      );

      expect(calls("pool_sign_out")).toEqual([{ pool: "builds", name: "work" }]);
      expect(await screen.findByText("work is signed out on pool builds.")).toBeVisible();
      await waitFor(() => expect(profilesAsked).toBe(before + 1));
      // Codex's own sign-out runs too; opencode's asks in a terminal (#4124).
      expect(screen.getByRole("button", { name: "Sign out fast on pool builds" })).toBeVisible();
      expect(screen.getByText("Gemini signs out only in its own terminal.")).toBeVisible();
    } finally {
      signedIn = false;
      await act(() => askProfiles("builds"));
    }
  });

  it("opens opencode's sign-out in a terminal, with a project open (#4124)", async () => {
    signedIn = true;
    await act(() => askProfiles("builds"));
    const typed: unknown[] = [];
    const heard = (event: Event) => typed.push((event as CustomEvent).detail);
    window.addEventListener(PROFILE_SIGN_OUT, heard);
    try {
      render(<PoolProfiles pool="builds" plane="/home/op/project" />);
      await userEvent.click(
        await screen.findByRole("button", { name: "Sign out open on pool builds in a terminal" }),
      );
      expect(typed).toEqual([{ plane: "/home/op/project", profile: "open", pool: "builds" }]);
    } finally {
      window.removeEventListener(PROFILE_SIGN_OUT, heard);
      signedIn = false;
      await act(() => askProfiles("builds"));
    }
  });

  it("signs Codex in by approving in the browser, with nothing to paste (#4044)", async () => {
    render(<PoolProfiles pool="builds" plane="/home/op/project" />);
    await userEvent.click(
      await screen.findByRole("button", { name: "Sign fast in on pool builds" }),
    );
    const opened = await screen.findByRole("group", { name: "Signing fast in on builds" });
    expect(calls("pool_sign_in_start")).toEqual([
      { pool: "builds", name: "fast", provider: null, terminal: null },
    ]);
    expect(within(opened).getByText(/Step 1 of 2/)).toBeInTheDocument();

    says({ at: "approve", url: OPENAI });

    expect(await within(opened).findByText("Step 1 of 2: Approve in your browser")).toBeVisible();
    // The CLI's --pool sign-in cannot carry the browser's answer, so it is not offered.
    expect(within(opened).queryByText(/From a terminal/)).toBeNull();
    expect(calls("plugin:opener|open_url")).toEqual([expect.objectContaining({ url: OPENAI })]);
    expect(within(opened).queryByRole("textbox")).not.toBeInTheDocument();

    says({ at: "checking" });
    expect(await within(opened).findByText(/Step 2 of 2/)).toBeInTheDocument();
    says({ at: "signed-in" });
    expect(await within(opened).findByText("Signed in")).toBeInTheDocument();
  });

  it("signs opencode in to ChatGPT in place too (#4044)", async () => {
    render(<PoolProfiles pool="builds" plane="/home/op/project" />);
    await userEvent.click(
      await screen.findByRole("button", { name: "Sign open in to ChatGPT on pool builds" }),
    );
    await screen.findByRole("group", { name: "Signing open in on builds" });

    expect(calls("pool_sign_in_start")).toEqual([
      { pool: "builds", name: "open", provider: "openai", terminal: null },
    ]);
  });

  it("says a taken port, and runs the device code in the strip, with no project (#4044, #4106)", async () => {
    const heard: ProfileSignInAsk[] = [];
    const hear = (event: Event) => heard.push((event as CustomEvent<ProfileSignInAsk>).detail);
    window.addEventListener(PROFILE_SIGN_IN, hear);
    try {
      render(<PoolProfiles pool="builds" plane={undefined} />);
      await userEvent.click(
        await screen.findByRole("button", { name: "Sign fast in on pool builds" }),
      );
      const opened = await screen.findByRole("group", { name: "Signing fast in on builds" });

      says({ at: "port-taken", port: 1455 });

      expect(await within(opened).findByRole("alert")).toHaveTextContent(
        "Port 1455 is in use on this Mac",
      );
      expect(within(opened).getByText("codex login --device-auth")).toBeVisible();
      expect(calls("plugin:opener|open_url")).toEqual([]);
      // The line says the sign-in failed: the person acts on the port (#4044).
      expect(await screen.findByText("Sign-in failed")).toBeVisible();
      await userEvent.click(within(opened).getByRole("button", { name: "Use a device code" }));

      // The same strip: the core starts the device flow in the runner's terminal, no shell tab.
      const terminal = await within(opened).findByRole("group", {
        name: "The runner's terminal on builds",
      });
      expect(heard).toEqual([]);
      expect(within(opened).getByRole("status")).toHaveTextContent(
        "Step 1 of 2: Enter the device code the terminal below shows",
      );
      expect(await screen.findByText("Needs you in the terminal")).toBeVisible();
      await waitFor(() =>
        expect(calls("pool_sign_in_start")).toEqual([
          { pool: "builds", name: "fast", provider: null, terminal: null },
          { pool: "builds", name: "fast", provider: null, terminal: { columns: 80, rows: 16 } },
        ]),
      );
      await userEvent.click(within(terminal).getByRole("button", { name: "Type 1" }));
      await waitFor(() => expect(calls("pool_sign_in_type")).toHaveLength(1));

      // Its status check ends the strip as any other sign-in does.
      says({ at: "checking" });
      says({ at: "signed-in" });
      expect(await within(opened).findByText("Signed in")).toBeInTheDocument();
    } finally {
      window.removeEventListener(PROFILE_SIGN_IN, hear);
    }
  });

  it("names opencode's ChatGPT device code by its plan, and tries again in the terminal (#4106)", async () => {
    render(<PoolProfiles pool="builds" plane={undefined} />);
    await userEvent.click(
      await screen.findByRole("button", { name: "Sign open in to ChatGPT on pool builds" }),
    );
    const opened = await screen.findByRole("group", { name: "Signing open in on builds" });
    says({ at: "port-taken", port: 1455 });

    await userEvent.click(await within(opened).findByRole("button", { name: "Use a device code" }));

    await waitFor(() =>
      expect(calls("pool_sign_in_start")[1]).toEqual({
        pool: "builds",
        name: "open",
        provider: "openai",
        terminal: { columns: 80, rows: 16 },
      }),
    );
    expect(within(opened).queryByText(/codex login/)).toBeNull();

    says({ at: "failed", why: "the runner said: no" });
    await userEvent.click(await within(opened).findByRole("button", { name: "Try again" }));

    await waitFor(() => expect(calls("pool_sign_in_start")).toHaveLength(3));
    expect(calls("pool_sign_in_start")[2]).toEqual(
      expect.objectContaining({ provider: "openai", terminal: { columns: 80, rows: 16 } }),
    );
    expect(within(opened).getByRole("status")).toHaveTextContent(/device code/);
  });

  it("keeps another harness's sign-in in a shell tab", async () => {
    const heard: ProfileSignInAsk[] = [];
    const hear = (event: Event) => heard.push((event as CustomEvent<ProfileSignInAsk>).detail);
    window.addEventListener(PROFILE_SIGN_IN, hear);
    try {
      render(<PoolProfiles pool="builds" plane="/home/op/project" />);
      await userEvent.click(
        await screen.findByRole("button", { name: "Sign gem in on pool builds" }),
      );

      expect(heard).toHaveLength(1);
      expect(calls("pool_sign_in_start")).toEqual([]);
    } finally {
      window.removeEventListener(PROFILE_SIGN_IN, hear);
    }
  });
});
