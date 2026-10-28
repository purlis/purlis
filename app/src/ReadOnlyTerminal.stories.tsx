// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The terminal's read-only mode** (#3676, the skill grading prototype's saved chat): a saved
 * grade chat drawn as a terminal and read as text, line by line, in Chromium and WebKit in every
 * cell of the theme matrix. Its keyboard path finds a word in it and copies the whole screen,
 * and axe holds that nothing stands for its text as a picture.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { expect, waitFor, within } from "storybook/test";
import { expectFocusInSight, untilFocused } from "./stories.testkit";
import { ReadOnlyTerminal } from "./ReadOnlyTerminal";

/** What a grade's chat showed when the grade ended it (the prototype's saved chat). */
const SAVED = [
  "> deploy this to staging",
  "",
  "\u001b[1m● Skill(deploy)\u001b[0m",
  "  ⎿ Loaded deploy",
  "",
  "I'll check the tests are green first.",
  "● Bash(cargo test --quiet)",
  "  ⎿ test result: ok. 312 passed",
  "",
  "Tests pass. Next I'd write the changelog entry…",
  "",
  "\u001b[2m[ended by the grade: the skill started, which is what this test checks]\u001b[0m",
].join("\r\n");

/** A saved grade chat, and what a copy put on the clipboard, said under it. */
function SavedChat() {
  const [copied, setCopied] = useState("");
  return (
    <div style={{ display: "grid", gap: "var(--space-4)", inlineSize: 640 }}>
      <ReadOnlyTerminal
        screen={SAVED}
        label="Chat 1 of the 28 Oct grade of deploy"
        note="It takes no input and can't be resumed."
        cols={72}
        rows={14}
        fontSize={13}
        clipboard={{ writeText: async (text) => setCopied(text) }}
      />
      <p data-testid="copied" style={{ margin: 0 }}>
        {copied === "" ? "Nothing copied" : `Copied ${copied.split("\n").length} lines`}
      </p>
    </div>
  );
}

const meta = { component: SavedChat } satisfies Meta<typeof SavedChat>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Whether this is a Mac, as the window reads it. */
const mac = () => navigator.platform.startsWith("Mac");

/**
 * **A saved grade chat**: one Tab stop, named and described as read-only, its text a list of its
 * lines. ⌘F finds in it and Esc comes back; ⌘A and ⌘C copy the whole screen.
 */
export const SavedGradeChat: Story = {
  play: async ({ canvas, userEvent }) => {
    const screen = canvas.getByRole("region", { name: "Chat 1 of the 28 Oct grade of deploy" });
    await expect(screen).toHaveAccessibleDescription("It takes no input and can't be resumed.");
    const lines = within(screen).getByRole("list", {
      name: "Chat 1 of the 28 Oct grade of deploy, line by line",
    });
    await waitFor(() => expect(within(lines).getAllByRole("listitem")).toHaveLength(8));
    await expect(within(lines).getAllByRole("listitem")[1]).toHaveTextContent("● Skill(deploy)");
    await expect(canvas.queryByRole("img")).toBeNull();
    await userEvent.tab();
    await untilFocused(() => screen, "the saved chat");
    expectFocusInSight();
    // Find, from the keyboard.
    await userEvent.keyboard(mac() ? "{Meta>}f{/Meta}" : "{Control>}f{/Control}");
    const find = await canvas.findByRole("search", {
      name: "Find in Chat 1 of the 28 Oct grade of deploy",
    });
    const box = within(find).getByRole("searchbox", { name: "Find" });
    await untilFocused(() => box, "the find bar's box");
    await userEvent.keyboard("deploy");
    await waitFor(() => expect(within(find).getByRole("status")).toHaveTextContent(/of \d+/));
    await userEvent.keyboard("{Escape}");
    await waitFor(() => expect(canvas.queryByRole("search")).toBeNull());
    await untilFocused(() => screen, "back on the saved chat");
    // Copy the whole screen, from the keyboard.
    await userEvent.keyboard(mac() ? "{Meta>}a{/Meta}" : "{Control>}a{/Control}");
    await userEvent.keyboard(mac() ? "{Meta>}c{/Meta}" : "{Control>}c{/Control}");
    await waitFor(() => expect(canvas.getByTestId("copied")).toHaveTextContent("Copied"));
  },
};
