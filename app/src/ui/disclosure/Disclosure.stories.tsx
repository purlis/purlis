// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The disclosure family** (#3675): Add source's More in its dialog, folded and open, a field
 * in it that refuses opening it, and a review's file to read under its risk mark. Each is walked
 * by the keyboard, held to axe in Chromium and WebKit in every cell of the theme matrix, and
 * compared with its baseline (`docs/component-stories.md`).
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { expect, waitFor, within } from "storybook/test";
import { expectFocusInSight, pictureNow, untilFocused } from "../../stories.testkit";
import { Badge } from "../badge/Badge";
import { Dialog } from "../dialog/Dialog";
import { TextField } from "../field/Field";
import { Select } from "../select/Select";
import { Disclosure } from "./Disclosure";

/** A folder in the repository: a path inside it, never one from the root of the machine. */
const relative = (value: string) =>
  value.startsWith("/") ? "A folder in the repository, without a leading /." : undefined;

/** **Add source…** (#3632): the repository, and under More its branch, folder and sign-in. */
function AddSource({ open = false, folder: start = "" }: { open?: boolean; folder?: string }) {
  const [url, setUrl] = useState("https://github.com/anthropics/skills");
  const [branch, setBranch] = useState("");
  const [folder, setFolder] = useState(start);
  const [vault, setVault] = useState("");
  const [said, setSaid] = useState("Not added yet");
  return (
    <>
      <p role="status">{said}</p>
      <Dialog
        onClose={() => setSaid("Cancelled")}
        title="Add a source"
        description="Nothing is installed until you review a skill from it."
        main={{ children: "Add source", onPress: () => setSaid("Added") }}
      >
        <div style={{ display: "grid", gap: "var(--space-4)" }}>
          <TextField label="Repository" value={url} onChange={setUrl} />
          <Disclosure label="More: branch, folder, sign-in" defaultExpanded={open}>
            <TextField label="Branch" placeholder="main" value={branch} onChange={setBranch} />
            <TextField
              label="Folder in the repository"
              placeholder="skills/"
              value={folder}
              onChange={setFolder}
              validate={relative}
            />
            <Select
              aria-label="Signs in with vault"
              value={vault}
              onValueChange={setVault}
              options={[
                { value: "", label: "None" },
                { value: "forge-token", label: "forge-token" },
              ]}
            />
          </Disclosure>
        </div>
      </Dialog>
    </>
  );
}

const meta = {
  component: AddSource,
  parameters: { screenshot: "page" },
} satisfies Meta<typeof AddSource>;

export default meta;
type Story = StoryObj<typeof meta>;

/** What the page holds now: a dialog is drawn over the story's page, outside its canvas. */
const page = () => within(document.body);

/**
 * **More, folded**: a quiet trigger after the repository, its fields out of the reading order.
 * Return on it opens the panel under it, Tab goes into it, and Space folds it again from the
 * trigger, the keyboard staying there.
 */
export const MoreFolded: Story = {
  play: async ({ userEvent }) => {
    const more = await page().findByRole("button", { name: "More: branch, folder, sign-in" });
    await expect(more).toHaveAttribute("aria-expanded", "false");
    await expect(page().queryByRole("textbox", { name: "Branch" })).toBeNull();
    const repository = page().getByRole("textbox", { name: "Repository" });
    await untilFocused(() => repository, "the dialog starts in Repository");
    await pictureNow();
    const before = more.getBoundingClientRect();
    await userEvent.tab();
    await untilFocused(() => more, "More");
    expectFocusInSight();
    await userEvent.keyboard("{Enter}");
    await waitFor(() => expect(more).toHaveAttribute("aria-expanded", "true"));
    // Opening moves only what is under it.
    const after = more.getBoundingClientRect();
    await expect([after.top, after.left, after.height]).toEqual([
      before.top,
      before.left,
      before.height,
    ]);
    const panel = page().getByRole("group", { name: "More: branch, folder, sign-in" });
    await expect(panel).toBeVisible();
    await userEvent.tab();
    await untilFocused(() => page().getByRole("textbox", { name: "Branch" }), "Branch");
    expectFocusInSight();
    await userEvent.tab({ shift: true });
    await untilFocused(() => more, "back on More");
    await userEvent.keyboard(" ");
    await waitFor(() => expect(more).toHaveAttribute("aria-expanded", "false"));
    await untilFocused(() => more, "More keeps the keyboard as it folds");
  },
};

/** **More, open**: drawn open, its three fields in the reading order after its trigger. */
export const MoreOpen: Story = {
  args: { open: true },
  play: async ({ userEvent }) => {
    const more = await page().findByRole("button", { name: "More: branch, folder, sign-in" });
    await expect(more).toHaveAttribute("aria-expanded", "true");
    await expect(more).toHaveAttribute("aria-controls", page().getByRole("group").id);
    await untilFocused(
      () => page().getByRole("textbox", { name: "Repository" }),
      "the dialog starts in Repository",
    );
    await pictureNow();
    await userEvent.tab();
    await userEvent.tab();
    await untilFocused(() => page().getByRole("textbox", { name: "Branch" }), "Branch");
    await userEvent.tab();
    await untilFocused(
      () => page().getByRole("textbox", { name: "Folder in the repository" }),
      "Folder",
    );
    expectFocusInSight();
  },
};

/**
 * **A refused field inside a folded More** (#3675, #3291): Add source with a folder from the
 * machine's root opens More, puts the keyboard in the folder's box, and says why once: one alert,
 * the field's own error line.
 */
export const RefusedInsideMore: Story = {
  args: { folder: "/skills" },
  play: async ({ userEvent }) => {
    const more = await page().findByRole("button", { name: "More: branch, folder, sign-in" });
    await expect(more).toHaveAttribute("aria-expanded", "false");
    await untilFocused(
      () => page().getByRole("textbox", { name: "Repository" }),
      "the dialog starts in Repository",
    );
    await userEvent.keyboard("{Enter}");
    const folder = await page().findByRole("textbox", { name: "Folder in the repository" });
    await waitFor(() => expect(more).toHaveAttribute("aria-expanded", "true"));
    await untilFocused(() => folder, "the refused folder");
    expectFocusInSight();
    const alerts = page().getAllByRole("alert");
    await expect(alerts).toHaveLength(1);
    await expect(alerts[0]).toHaveTextContent("A folder in the repository, without a leading /.");
    await expect(folder).toHaveAccessibleDescription(
      "A folder in the repository, without a leading /.",
    );
    await expect(page().getByRole("status")).toHaveTextContent("Not added yet");
  },
};

/** **A review's file** (#3632): its risk mark beside it, its text under it, read by opening it. */
function ReviewFiles() {
  return (
    <div style={{ display: "grid", gap: "var(--space-4)", inlineSize: 520 }}>
      <Disclosure label="Show SKILL.md" trail={<Badge variant="ok">ok</Badge>}>
        <pre style={{ margin: 0, whiteSpace: "pre-wrap" }}>
          {"---\nname: systematic-debugging\ndescription: Finds the cause of a bug first.\n---"}
        </pre>
      </Disclosure>
      <Disclosure label="Show scripts/repro.sh" trail={<Badge variant="waiting">look</Badge>}>
        <pre style={{ margin: 0, whiteSpace: "pre-wrap" }}>
          {'#!/bin/sh\nset -eu\ncargo test --quiet "$1"'}
        </pre>
      </Disclosure>
    </div>
  );
}

/**
 * **A file to read, under its risk mark**: each file's trigger is one Tab stop with its mark after
 * it, and opening one moves only the files under it.
 */
export const AReviewFile: StoryObj<typeof ReviewFiles> = {
  render: () => <ReviewFiles />,
  parameters: { screenshot: "frame" },
  play: async ({ canvas, userEvent }) => {
    const skill = canvas.getByRole("button", { name: "Show SKILL.md" });
    const script = canvas.getByRole("button", { name: "Show scripts/repro.sh" });
    await userEvent.tab();
    await untilFocused(() => skill, "the first file");
    await userEvent.tab();
    await untilFocused(() => script, "the second file");
    const top = script.getBoundingClientRect().top;
    await userEvent.keyboard("{Enter}");
    await waitFor(() => expect(script).toHaveAttribute("aria-expanded", "true"));
    await expect(canvas.getByRole("group", { name: "Show scripts/repro.sh" })).toHaveTextContent(
      "cargo test --quiet",
    );
    await expect(script.getBoundingClientRect().top).toBe(top);
    await userEvent.tab({ shift: true });
    await untilFocused(() => skill, "back on the first file");
    await userEvent.keyboard("{Enter}");
    await waitFor(() => expect(skill).toHaveAttribute("aria-expanded", "true"));
    // The second file moved down by the first's text, and is still in sight on the keyboard.
    await userEvent.tab();
    await untilFocused(() => script, "the second file again");
    expectFocusInSight();
  },
};
