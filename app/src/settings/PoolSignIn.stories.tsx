// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **A profile's sign-in on a pool's runner, in place** (#4042, spec #4023, prototype Variant A):
 * the strip a profile's line opens into, at each step the core says, Claude Code's and, approved
 * in the browser, Codex's (#4044). Each is drawn in every cell of the theme matrix, light and dark
 * among them.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState, type FormEvent } from "react";
import { expect, fn } from "storybook/test";
import { expectFocusInSight, untilFocused } from "../stories.testkit";
import { SignInStrip, type StripStep } from "./PoolSignIn";

const PAGE = "https://claude.ai/oauth/authorize?code=true";

const TERMINAL_SHOWS =
  "Select login method:\r\n \x1b[36m❯\x1b[0m 1. Claude account with subscription\r\n   2. Anthropic Console account\r\n";

function Strip({
  step,
  onPaste,
  harness = "Claude Code",
}: {
  step: StripStep;
  onPaste: (code: string) => void;
  harness?: string;
}) {
  const [code, setCode] = useState("");
  return (
    <div className="ui-machine-entry" style={{ inlineSize: 640 }}>
      <span className="ui-machine-name">work</span>
      <SignInStrip
        pool="builds"
        profile="work"
        kind={harness === "Claude Code" ? "claude" : "codex"}
        harness={harness}
        approves={harness !== "Claude Code"}
        step={step}
        code={code}
        setCode={setCode}
        pasteRefused={undefined}
        onPaste={(event: FormEvent) => {
          event.preventDefault();
          onPaste(code);
        }}
        page={step.at === "starting" ? undefined : PAGE}
        onTryAgain={() => undefined}
        onEnd={() => undefined}
        onTerminal={() => undefined}
        terminal={
          step.at === "could-not-follow" || step.at === "device-in-terminal"
            ? { printer: (print) => print?.(TERMINAL_SHOWS), onType: () => undefined, attempt: 0 }
            : undefined
        }
        onDeviceCode={() => undefined}
      />
    </div>
  );
}

const meta = {
  component: Strip,
  args: { step: { at: "starting" }, onPaste: fn() },
} satisfies Meta<typeof Strip>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Step 1: the login starts on the runner, and its page opens in the browser here. */
export const Starting: Story = {};

/**
 * Step 2: the code field, with the keyboard in it. The code typed goes with Enter; Cancel and
 * Open in terminal are a Tab away.
 */
export const PasteCode: Story = {
  args: { step: { at: "code" } },
  play: async ({ args, canvas, userEvent }) => {
    const field = canvas.getByRole("textbox", { name: "Paste code" });
    await untilFocused(() => field, "the code field takes the keyboard when it appears");
    expectFocusInSight();
    await userEvent.keyboard("abc123#state{Enter}");
    await expect(args.onPaste).toHaveBeenCalledWith("abc123#state");
    await userEvent.tab();
    await untilFocused(
      () => canvas.getByRole("button", { name: "Sign in" }),
      "Tab moves to Sign in",
    );
    expectFocusInSight();
  },
};

/** Step 2 of a device code's sign-in (#4045): the code, large, and its page only on Open page. */
export const DeviceCode: Story = {
  args: { step: { at: "device", url: "https://github.com/login/device", code: "WDJB-MJHT" } },
  play: async ({ canvas }) => {
    await expect(canvas.getByLabelText("The code to enter on the page")).toHaveTextContent(
      "WDJB-MJHT",
    );
    await expect(canvas.getByRole("button", { name: "Open page" })).toBeVisible();
    await expect(canvas.getByRole("button", { name: "Copy code" })).toBeVisible();
  },
};

/** The harness refused the code, in its words; another may be pasted, from the field. */
export const CodeRefused: Story = {
  args: {
    step: { at: "refused", said: "Invalid code. That code is not from this sign-in." },
  },
  play: async ({ canvas }) => {
    await expect(canvas.getByRole("alert")).toHaveTextContent("Invalid code");
    await untilFocused(
      () => canvas.getByRole("textbox", { name: "Paste code" }),
      "the code field keeps the keyboard",
    );
    expectFocusInSight();
  },
};

/** Codex's step 1: nothing to paste, the person approves in the page that opened. */
export const Approve: Story = {
  args: { harness: "Codex", step: { at: "approve", url: PAGE } },
};

/** "Use a device code", pressed (#4106): the harness's device flow, in the strip's own terminal. */
export const DeviceInTerminal: Story = {
  args: { harness: "Codex", step: { at: "device-in-terminal" } },
  play: async ({ canvas }) => {
    await expect(
      canvas.getByRole("group", { name: "The runner's terminal on builds" }),
    ).toBeInTheDocument();
    await expect(canvas.getByRole("status")).toHaveTextContent(
      "Step 1 of 2: Enter the device code the terminal below shows",
    );
  },
};

/** The port for the browser's answer is taken on this Mac: it says so, and a device code is left. */
export const PortTaken: Story = {
  args: { harness: "Codex", step: { at: "port-taken", port: 1455 } },
  play: async ({ canvas }) => {
    await expect(canvas.getByRole("alert")).toHaveTextContent("Port 1455 is in use on this Mac");
    await expect(canvas.getByText("codex login --device-auth")).toBeInTheDocument();
    await expect(canvas.getByRole("button", { name: "Use a device code" })).toBeVisible();
  },
};

/** Step 3: the runner's own status check is asked. */
export const Checking: Story = { args: { step: { at: "checking" } } };

/** The runner's status check says it is signed in: Close is all that is left. */
export const SignedIn: Story = { args: { step: { at: "signed-in" } } };

/**
 * purlis could not follow the login: the runner's own terminal, in the strip, with the keyboard in
 * it. Tab leaves it for Cancel, so it is no trap.
 */
export const CouldNotFollow: Story = {
  args: {
    step: {
      at: "could-not-follow",
      said: "Choose your sign-in method:\n  1. Claude account\n  2. Console account",
    },
  },
  play: async ({ canvas, userEvent }) => {
    await expect(
      canvas.getByRole("group", { name: "The runner's terminal on builds" }),
    ).toBeVisible();
    await userEvent.tab();
    await untilFocused(
      () => canvas.getByRole("button", { name: "Cancel" }),
      "Tab leaves the terminal for Cancel",
    );
    expectFocusInSight();
  },
};

/** It did not sign in, and why; Open the page again starts another, a Tab away from the strip. */
export const Failed: Story = {
  args: { step: { at: "failed", why: "the code has expired", stopped: false } },
  play: async ({ canvas, userEvent }) => {
    await expect(canvas.getByRole("alert")).toHaveTextContent("the code has expired");
    await untilFocused(
      () => canvas.getByRole("group", { name: "Signing work in on builds" }),
      "the strip holds the keyboard once nothing is asked",
    );
    await userEvent.tab();
    await untilFocused(
      () => canvas.getByRole("button", { name: "Open the page again" }),
      "Tab moves to Open the page again",
    );
    expectFocusInSight();
  },
};

/** The sign-in could not start: the pool's runner or this machine said why. */
export const NotStarted: Story = {
  args: {
    step: {
      at: "not-started",
      why: "Pool builds: its runner runs a purlis before 1.43, so its chats start on the profiles it has of its own. Nothing was signed in.",
    },
  },
};

/** The login was ended; nothing changed on the runner. */
export const Cancelled: Story = { args: { step: { at: "cancelled" } } };
