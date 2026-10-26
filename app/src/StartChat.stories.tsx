// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Start a chat** (#2918, prototype B of #2798): the window's own dialog, two panes at a fixed
 * height — the persona list under its filter beside what the chat starts with — drawn with the
 * prototype's 28 personas and three harnesses. Held to axe, walked by the keyboard and compared
 * with its baseline in Chromium and WebKit (`docs/component-stories.md`). The picture is of the
 * dialog as it opens, before the play answers it (`pictureNow`).
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { expect, waitFor, within } from "storybook/test";
import type { ProfileRow, StartOptions } from "./bindings";
import { StartChat } from "./StartChat";
import { expectFocusInSight, pictureNow } from "./stories.testkit";

const PERSONAS = [
  "steward",
  "devops",
  "domain-authority-master",
  "elastic-logs-master",
  "email-investigation-master",
  "email-verification-master",
  "entri-master",
  "esp-integrations-master",
  "grafana-master",
  "inbox-placement-master",
  "integrations-master",
  "jira-git-flow-master",
  "keycloak-master",
  "limits-master",
  "marketplace-integrations-master",
  "messaging-integrations-master",
  "nestjs-developer",
  "organizations-master",
  "pending-actions-master",
  "performance-investigator",
  "platform-and-authz-master",
  "psa-integrations-master",
  "public-api-master",
  "reporting-master",
  "security-reviewer",
  "sending-domains-master",
  "support-triage",
  "ui-designer",
];

/** One of the built-in profiles, as the core answers it. */
function builtIn(
  name: string,
  kind: string,
  glyph: string,
  monogram: string,
  billed: string,
): ProfileRow {
  return {
    name,
    kind,
    shown: name,
    source: "built-in",
    is_default: kind === "claude",
    ready_to_type: true,
    harness: null,
    sandbox: null,
    approval: null,
    model: "the harness's default",
    billed_to: billed,
    icon: { glyph, own: null, monogram },
  };
}

const OPTIONS: StartOptions = {
  profiles: [
    builtIn("claude", "claude", "spark", "C", "whatever Claude Code is signed in with · claude"),
    builtIn("opencode", "opencode", "braces", "OC", "whatever opencode is signed in with"),
    builtIn("codex", "codex", "terminal", "CX", "whatever Codex is signed in with"),
  ],
  own_icons: null,
  refused: [],
  personas: PERSONAS,
  persona: "steward",
  persona_profiles: {},
  persona_descriptions: { steward: "Control Plane Steward" },
  ignore_fix: null,
  ignore_fix_id: null,
  declares_none: true,
};

/** The dialog as the window opens it, and what was answered once it is gone. */
function Window({ options = OPTIONS }: { options?: StartOptions }) {
  const [open, setOpen] = useState(true);
  const [said, setSaid] = useState("Nothing started");
  return (
    <>
      <p role="status" style={{ margin: 0 }}>
        {said}
      </p>
      {open && (
        <StartChat
          options={options}
          place="smart-ide"
          onStart={(profile, persona, footer, label) => {
            setSaid(
              `Started ${label ?? persona ?? "a chat"} as ${persona ?? "no persona"} on ${profile}` +
                (footer ? " with purlis's footer" : ""),
            );
            setOpen(false);
          }}
          onApprove={() => setSaid("Approved")}
          onCancel={() => {
            setSaid("Cancelled");
            setOpen(false);
          }}
        />
      )}
    </>
  );
}

const meta = {
  component: Window,
  parameters: { screenshot: "page" },
} satisfies Meta<typeof Window>;

export default meta;
type Story = StoryObj<typeof meta>;

/** The dialog's box: where it is and how big, which no pick changes. */
const boxOf = (dialog: HTMLElement) => {
  const { top, width, height } = dialog.getBoundingClientRect();
  return [top, width, height];
};

/**
 * **Prototype B**: the wide size at a fixed height, the list beside the details. Filtering,
 * picking a persona and a harness move and resize nothing, the footer and Start stay in view,
 * and ⏎ in the filter starts the chat on what is picked.
 */
export const PrototypeB: Story = {
  play: async ({ userEvent }) => {
    // A dialog is drawn over the page, outside the story's page.
    const page = within(document.body);
    const dialog = await page.findByRole("dialog", { name: "Start a chat" });
    await expect(dialog).toHaveAttribute("data-size", "wide");
    await expect(dialog).toHaveAccessibleDescription("In smart-ide · purlis's built-in profiles");
    // The keyboard starts on Cancel: a Return pressed by habit starts nothing.
    await waitFor(() => expect(page.getByRole("button", { name: "Cancel" })).toHaveFocus());
    await pictureNow();
    const box = boxOf(dialog);

    const filter = page.getByRole("searchbox", { name: "Filter personas" });
    await expect(filter).toHaveAttribute("placeholder", "Filter 28 personas");
    await userEvent.click(filter);
    await userEvent.type(filter, "master");
    // The pick the filter left out went to the first match (R13); the caret stayed (R168).
    await expect(filter).toHaveFocus();
    await expect(page.getByRole("option", { name: "domain-authority-master" })).toHaveAttribute(
      "aria-selected",
      "true",
    );
    await userEvent.keyboard("{ArrowDown}");
    await expect(page.getByRole("option", { name: "elastic-logs-master" })).toHaveAttribute(
      "aria-selected",
      "true",
    );
    await expect(boxOf(dialog)).toEqual(box);

    // A harness card picked: the dialog keeps its box and the summary follows (R33).
    await userEvent.click(page.getByRole("radio", { name: "opencode" }));
    await expect(page.getByTestId("start-chat-summary")).toHaveTextContent(
      "elastic-logs-master on",
    );
    await expect(page.getByTestId("start-chat-summary")).toHaveTextContent("opencode");
    await expect(boxOf(dialog)).toEqual(box);

    // The footer and its answers are in view whatever the panes hold.
    const start = page.getByRole("button", { name: "Start chat" });
    const shown = start.getBoundingClientRect();
    await expect(shown.bottom).toBeLessThanOrEqual(window.innerHeight);

    await userEvent.type(filter, "zz");
    await waitFor(() => expect(page.getByText("No persona is called “masterzz”.")).toBeVisible());
    await userEvent.clear(filter);
    await userEvent.type(filter, "keycloak");
    expectFocusInSight();
    await userEvent.keyboard("{Enter}");
    await waitFor(() =>
      expect(page.getByRole("status", { hidden: true })).toHaveTextContent(
        "Started keycloak-master as keycloak-master on opencode",
      ),
    );
  },
};

/** The prototype's harnesses in a project that runs every chat sandboxed, one asking approval. */
const GUARDED: StartOptions = {
  ...OPTIONS,
  profiles: [
    {
      ...OPTIONS.profiles[0],
      sandbox: {
        state: "sandboxed",
        said: "claude reaches only the hosts this project allows.",
        install: null,
        locked: null,
      },
    },
    { ...OPTIONS.profiles[1], shown: "opencode --port 0", approval: "new" },
    OPTIONS.profiles[2],
  ],
};

/**
 * **A sandboxed project with a profile to approve**: ticking the opt-out and picking the profile
 * that asks for approval change the main answer's words and look, and move and widen nothing —
 * not the dialog, not the answer (R163).
 */
export const SandboxAndApproval: Story = {
  args: { options: GUARDED },
  play: async ({ userEvent }) => {
    const page = within(document.body);
    const dialog = await page.findByRole("dialog", { name: "Start a chat" });
    await waitFor(() => expect(page.getByRole("button", { name: "Cancel" })).toHaveFocus());
    // Said in the details' Sandbox section; the dialog fades in as it opens, so it is waited for.
    await waitFor(() =>
      expect(page.getByText("claude reaches only the hosts this project allows.")).toBeVisible(),
    );
    await pictureNow();
    const box = boxOf(dialog);
    const answer = () => page.getByRole("button", { name: /^(Start|Approve)/ });
    const room = () => {
      const { top, width, height } = answer().getBoundingClientRect();
      return [top, width, height];
    };
    const before = room();

    await userEvent.click(page.getByRole("checkbox", { name: "Start without the sandbox" }));
    await expect(answer()).toHaveAccessibleName("Start without the sandbox");
    await expect(
      page.getByRole("textbox", { name: "Why, if you want it recorded" }),
    ).toBeInTheDocument();
    await expect(boxOf(dialog)).toEqual(box);
    await expect(room()).toEqual(before);

    await userEvent.click(page.getByRole("radio", { name: "opencode" }));
    await expect(answer()).toHaveAccessibleName("Approve and start");
    await expect(page.getByRole("alert")).toHaveTextContent("opencode --port 0");
    await expect(boxOf(dialog)).toEqual(box);
    await expect(room()).toEqual(before);
    expectFocusInSight();
  },
};
