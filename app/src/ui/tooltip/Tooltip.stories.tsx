// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The tooltip family** (#2908): a hint on words, an icon button's name, an icon button's
 * second line and a disabled button's reason, each drawn open, held to axe, walked by the
 * keyboard (Tab opens, Esc dismisses with the focus kept, Tab leaves) and compared with its
 * baseline in Chromium and WebKit (`docs/component-stories.md`). A tooltip is drawn outside the
 * story's frame, so its picture is the page's.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, waitFor } from "storybook/test";
import { RefreshCw, X } from "lucide-react";
import { expectFocusInSight } from "../../stories.testkit";
import { Button, IconButton } from "../button/Button";
import { ReadOnlyFieldset } from "../../ReadOnlyLine";
import { TextField } from "../field/Field";
import { Tooltip, TOOLTIP_DELAY } from "./Tooltip";

/** What a story draws: one target with its hint, and a button after it to Tab on to. */
function Hinted({ target }: { target: "words" | "icon" | "detail" | "unavailable" }) {
  return (
    <div style={{ display: "flex", gap: "var(--space-4)", alignItems: "center" }}>
      {target === "words" ? (
        <Tooltip tip="/Users/me/work/purlis">
          <span>purlis</span>
        </Tooltip>
      ) : target === "icon" ? (
        <IconButton aria-label="Close the tab" icon={<X />} />
      ) : target === "detail" ? (
        <IconButton
          aria-label="Reset"
          tooltip="Takes the value this file sets out"
          icon={<RefreshCw />}
        />
      ) : (
        <Button isDisabled tooltip="This project is read-only here.">
          Publish
        </Button>
      )}
      <Button>Next</Button>
    </div>
  );
}

const meta = {
  component: Hinted,
  parameters: { screenshot: "page" },
} satisfies Meta<typeof Hinted>;

export default meta;
type Story = StoryObj<typeof meta>;

/** The tooltip shown now, which is drawn in the page and not in the story's frame. */
const shown = () => document.querySelector<HTMLElement>('[role="tooltip"]');

/**
 * Tab reaches the target and opens its tooltip at once; Esc takes the tooltip down and leaves the
 * focus where it was (WCAG 1.4.13); Tab leaves; Shift+Tab comes back, which is what the baseline
 * shows: the target in focus with its tooltip open.
 */
type Canvas = Parameters<NonNullable<Story["play"]>>[0]["canvas"];

const walked =
  (find: (canvas: Canvas) => HTMLElement, says: string | RegExp): Story["play"] =>
  async ({ canvas, userEvent }) => {
    const target = find(canvas);
    await userEvent.tab();
    await expect(target).toHaveFocus();
    expectFocusInSight();
    await waitFor(() => expect(shown()).toHaveTextContent(says));
    await userEvent.keyboard("{Escape}");
    await waitFor(() => expect(shown()).toBeNull());
    await expect(target).toHaveFocus();
    await userEvent.tab();
    await expect(target).not.toHaveFocus();
    await waitFor(() => expect(shown()).toBeNull());
    await userEvent.tab({ shift: true });
    await expect(target).toHaveFocus();
    await waitFor(() => expect(shown()).toHaveTextContent(says));
    expectFocusInSight();
  };

const button = (name: string) => (canvas: Canvas) => canvas.getByRole("button", { name });

/** Words with a hint take the keyboard, so the hint is never only a hover's. */
export const OnWords: Story = {
  args: { target: "words" },
  play: walked((canvas) => canvas.getByText("purlis"), "/Users/me/work/purlis"),
};

/** An icon button's tooltip is its name: what a person reads is what a screen reader says. */
export const IconButtonName: Story = {
  args: { target: "icon" },
  play: walked(button("Close the tab"), "Close the tab"),
};

/** An icon button's hint is a second line under its name, and its description. */
export const IconButtonDetail: Story = {
  args: { target: "detail" },
  play: walked(button("Reset"), /^ResetTakes the value this file sets out$/),
};

/** A disabled button with a hint keeps the focus, so the reason it is disabled can be read. */
export const DisabledWithReason: Story = {
  args: { target: "unavailable" },
  play: walked(button("Publish"), "This project is read-only here."),
};

/** The sentence a read-only project's controls are off with, as the core says it. */
const READ_ONLY = "This project requires the feature memory-pro, which this purlis does not have.";

/** A read-only project's group: a disabled field, and the group's word that says why. */
function ReadOnlyGroup() {
  return (
    <ReadOnlyFieldset said={READ_ONLY}>
      <TextField label="Depth" value="3" onChange={() => {}} isDisabled />
    </ReadOnlyFieldset>
  );
}

/**
 * **A read-only group's reason, by the pointer and by the keyboard** (#2908), in WebKit as in
 * Chromium: the field keeps its native `disabled`; the group is described by the sentence, and
 * its "Read-only" shows it under the pointer and is the Tab stop the disabled field is not.
 */
export const ReadOnlyReason: StoryObj = {
  render: () => <ReadOnlyGroup />,
  play: async ({ canvas, userEvent }) => {
    const field = canvas.getByRole("textbox", { name: "Depth" });
    await expect(field).toBeDisabled();
    // The group is named "Read-only" by its legend and described by the sentence; a settings
    // row in it adds the sentence to its control's description too (`SettingRow`).
    await expect(canvas.getByRole("group", { name: "Read-only" })).toHaveAccessibleDescription(
      READ_ONLY,
    );
    const why = canvas.getByText("Read-only");
    // The pointer comes onto the group's word from the page, and rests.
    await userEvent.hover(document.body);
    await userEvent.hover(why);
    await waitFor(() => expect(shown()).toHaveTextContent(READ_ONLY), {
      timeout: TOOLTIP_DELAY + 2000,
    });
    await userEvent.unhover(why);
    await waitFor(() => expect(shown()).toBeNull(), { timeout: 2000 });
    // The keyboard: the field takes none, the word takes it and opens its tooltip at once.
    await userEvent.tab();
    await expect(why).toHaveFocus();
    expectFocusInSight();
    await waitFor(() => expect(shown()).toHaveTextContent(READ_ONLY));
    await userEvent.keyboard("{Escape}");
    await waitFor(() => expect(shown()).toBeNull());
    await expect(why).toHaveFocus();
    // Back, for the baseline: the word in focus with its tooltip.
    await userEvent.tab();
    await userEvent.tab({ shift: true });
    await waitFor(() => expect(shown()).toHaveTextContent(READ_ONLY));
  },
};
