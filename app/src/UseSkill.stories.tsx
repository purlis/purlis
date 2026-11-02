// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Use skill…** (#2775; spec #3679 S8, #3694): a quick picker of the chat's own skills, built-in
 * ones last, its box taking the keys as it opens; typing filters, Enter takes the first match,
 * and a skill that takes arguments asks for them in a second step. Nothing above the list: the
 * chat's budget, routing and pins are the Skills view's. Held to axe, walked by the keyboard and
 * pictured as a page, as a dialog is drawn over the window (`docs/component-stories.md`).
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { expect, screen, waitFor, within } from "storybook/test";
import type { ChatSkill, ChatSkills } from "./bindings";
import { UseSkill } from "./UseSkill";
import { expectFocusInSight, pictureNow, untilFocused } from "./stories.testkit";

/** A skill of the chat's set, as the core answers it. */
function skill(name: string, description: string, more: Partial<ChatSkill> = {}): ChatSkill {
  const args = more.arguments ?? [];
  return {
    name,
    level: "project",
    label: "project",
    description,
    manual: false,
    arguments: args,
    hint: null,
    hidden: null,
    ...more,
    takes_arguments: args.length > 0 || (more.hint !== undefined && more.hint !== null),
  };
}

const SKILLS: ChatSkills = {
  on: true,
  known: true,
  program: [],
  plugins_off: [],
  own: null,
  skills: [
    skill("purlis-secrets", "Uses a vault's secret without its value entering the chat.", {
      level: "built-in",
      label: "built-in",
    }),
    skill("deploy", "Deploys the service to staging, then production."),
    skill("review-pr", "Reviews a pull request for correctness and style.", {
      arguments: ["pr", "focus"],
      hint: "<pr> [focus]",
    }),
    skill("release-notes", "Writes the release notes from the merged pull requests.", {
      manual: true,
      level: "workspace",
      label: "workspace ops",
    }),
  ],
  budget: {
    skills: 3,
    chars: 1234,
    manual: 1,
    most_skills: 30,
    most_chars: 6000,
    hidden: 0,
    still_over: null,
    pinned_by: [],
    pins_together: null,
    warns: "both",
  },
  repos: [],
  not_supported: null,
  routing: { router: "acme", ms: 3000, fell_back: true, said: "acme did not answer." },
  missing: null,
};

/** The picker, and what was typed said under it. */
function Picker() {
  const [said, setSaid] = useState("Nothing typed");
  return (
    <>
      <UseSkill
        chat="steward 5"
        skills={SKILLS}
        busy={false}
        onUse={(name, values) => setSaid(`Typed ${[name, ...values].join(" ")}`)}
        onCancel={() => setSaid("Cancelled")}
      />
      <p role="status">{said}</p>
    </>
  );
}

const meta = {
  component: Picker,
  parameters: { screenshot: "page" },
} satisfies Meta<typeof Picker>;

export default meta;
type Story = StoryObj<typeof meta>;

/**
 * **A quick picker**: it opens with the keyboard in its box and its built-in skill last, with no
 * budget, routing or warning over the list though the chat's set is past its budget and its
 * router fell back. Typing filters; Enter takes the first match, here one that takes arguments,
 * whose second step asks for them; Back goes to the list. The picture is of the list as it
 * opens.
 */
export const QuickPicker: Story = {
  play: async ({ userEvent }) => {
    const dialog = await screen.findByTestId("use-skill");
    const box = within(dialog).getByRole("searchbox", { name: "Filter skills" });
    await untilFocused(() => box, "Use skill: its box has the keyboard as it opens");
    const names = within(dialog)
      .getAllByTestId("use-skill-row")
      .map((row) => row.querySelector('[data-part="label"]')?.textContent);
    await expect(names).toEqual(["deploy", "review-pr", "release-notes", "purlis-secrets"]);
    await expect(within(dialog).queryByRole("status")).toBeNull();
    await pictureNow();

    await userEvent.keyboard("review");
    await waitFor(() => expect(within(dialog).getAllByTestId("use-skill-row")).toHaveLength(1));
    await userEvent.keyboard("{Enter}");
    await waitFor(() =>
      expect(within(dialog).getByRole("heading", { name: "Use review-pr" })).toBeVisible(),
    );
    const first = within(dialog).getAllByRole("textbox")[0];
    await untilFocused(() => first, "Use skill: the second step's first box has the keyboard");
    expectFocusInSight();
    await userEvent.keyboard("42");
    await userEvent.click(within(dialog).getByRole("button", { name: "Type it" }));
    await waitFor(() => expect(screen.getByText(/^Typed review-pr 42/)).toBeInTheDocument());
  },
};
