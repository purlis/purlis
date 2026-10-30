// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **A profile's sign-in on a pool's runner, in place** (#4042, spec #4023, prototype Variant A):
 * the strip a Claude Code profile's line opens into, at each step the core says. Each is drawn in
 * every cell of the theme matrix, light and dark among them.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState, type FormEvent } from "react";
import { expect, fn } from "storybook/test";
import { expectFocusInSight, untilFocused } from "../stories.testkit";
import { SignInStrip, type StripStep } from "./PoolSignIn";

const PAGE = "https://claude.ai/oauth/authorize?code=true";

function Strip({ step, onPaste }: { step: StripStep; onPaste: (code: string) => void }) {
  const [code, setCode] = useState("");
  return (
    <div className="ui-machine-entry" style={{ inlineSize: 640 }}>
      <span className="ui-machine-name">work</span>
      <SignInStrip
        pool="builds"
        profile="work"
        harness="Claude Code"
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
    const field = canvas.getByRole("textbox", { name: "Code from the browser" });
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

/** The harness refused the code, in its words; another may be pasted. */
export const CodeRefused: Story = {
  args: {
    step: { at: "refused", said: "Invalid code. Please make sure the full code was copied." },
  },
};

/** Step 3: the runner's own status check is asked. */
export const Checking: Story = { args: { step: { at: "checking" } } };

export const SignedIn: Story = { args: { step: { at: "signed-in" } } };

/** purlis could not follow the login: what it printed, and a terminal as the way out. */
export const CouldNotFollow: Story = {
  args: {
    step: {
      at: "could-not-follow",
      said: "Choose your sign-in method:\n  1. Claude account\n  2. Console account",
    },
  },
};

export const Failed: Story = {
  args: { step: { at: "failed", why: "the code has expired" } },
};
