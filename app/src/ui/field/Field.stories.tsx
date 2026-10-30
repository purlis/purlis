// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The field family** (#2905): a field with its label, help and error, a refused value, each
 * size of the text input with its glass, an unavailable box, a textarea, inline validation on
 * blur, ⌘⏎ sending a form, and a filter that keeps the keyboard and its caret when its list is
 * drawn again. Each is drawn, held to axe, walked by the keyboard and compared with its baseline
 * in Chromium and WebKit (`docs/component-stories.md`).
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { expect, waitFor, within } from "storybook/test";
import { ListFilter, Search } from "lucide-react";
import { expectFocusInSight, untilFocused } from "../../stories.testkit";
import { Button } from "../button/Button";
import { Dialog } from "../dialog/Dialog";
import { FIELD_SIZES, Form, TextArea, TextField, type PlainTextFieldProps } from "./Field";

/** A field that holds its own value, so the keyboard can change it. */
function Held(props: Omit<PlainTextFieldProps, "value" | "onChange"> & { start?: string }) {
  const [value, setValue] = useState(props.start ?? "");
  return (
    <div style={{ inlineSize: "20rem" }}>
      <TextField {...props} value={value} onChange={setValue} />
    </div>
  );
}

const meta = {
  component: Held,
} satisfies Meta<typeof Held>;

export default meta;
type Story = StoryObj<typeof meta>;

/** ⌘⏎ on a Mac, Ctrl+Enter elsewhere: the platform the story runs on. */
const SEND = navigator.platform.startsWith("Mac")
  ? "{Meta>}{Enter}{/Meta}"
  : "{Control>}{Enter}{/Control}";

const NAME_RULE = (value: string) =>
  /^[\w.-]*$/.test(value) ? undefined : "A name is letters, digits, ., _ and -.";

/** Tab lands in the box with its ring in sight; what is typed is in it; Tab leaves it. */
export const Labelled: Story = {
  args: {
    label: "Name",
    help: "It becomes a directory under workspaces/.",
    placeholder: "docs",
  },
  play: async ({ canvas, userEvent }) => {
    const box = canvas.getByRole("textbox", { name: "Name" });
    await expect(box).toHaveAccessibleDescription("It becomes a directory under workspaces/.");
    await userEvent.tab();
    await expect(box).toHaveFocus();
    expectFocusInSight();
    await userEvent.keyboard("docs");
    await expect(box).toHaveValue("docs");
    await userEvent.tab();
    await expect(box).not.toHaveFocus();
    // Back in it, which is what the baseline shows: the ring.
    await userEvent.tab({ shift: true });
    await expect(box).toHaveFocus();
  },
};

/** Nothing is said while the person types; leaving the box says what is wrong, and typing it
 *  right takes it away. */
export const ValidatedOnBlur: Story = {
  args: { label: "Name", help: "Letters, digits, ., _ and -.", validate: NAME_RULE },
  play: async ({ canvas, userEvent }) => {
    const box = canvas.getByRole("textbox", { name: "Name" });
    await userEvent.tab();
    await userEvent.keyboard("my name");
    // Nothing said yet, though the line is there, a live region, so VoiceOver hears words land.
    await expect(canvas.queryByRole("alert")).toBeNull();
    await userEvent.tab();
    await waitFor(() =>
      expect(canvas.getByRole("alert")).toHaveTextContent("A name is letters, digits, ., _ and -."),
    );
    await expect(box).toHaveAttribute("aria-invalid", "true");
    await userEvent.tab({ shift: true });
    await expect(box).toHaveFocus();
    expectFocusInSight();
    await userEvent.keyboard("{Backspace>5/}-name");
    await waitFor(() => expect(canvas.queryByRole("alert")).toBeNull());
    await userEvent.keyboard("{Backspace>5/} x");
    await userEvent.tab();
    // The baseline: a refused value, its edge and its words.
    await waitFor(() => expect(canvas.getByRole("alert")).toBeVisible());
  },
};

/** A refusal from elsewhere, the core's words, is said at once under the box. */
export const Refused: Story = {
  args: { label: "Name", start: "docs", error: "A workspace called docs is already here." },
  play: async ({ canvas, userEvent }) => {
    const box = canvas.getByRole("textbox", { name: "Name" });
    await expect(box).toHaveAccessibleDescription("A workspace called docs is already here.");
    await userEvent.tab();
    await expect(box).toHaveFocus();
    expectFocusInSight();
  },
};

const LONG_REFUSAL =
  "The core would not take this profile: it mounts /Volumes/shared/builds, which is outside the " +
  "workspace, and a workspace's profile may only mount what the workspace holds.";

/**
 * **A refusal too long for its line** (#3033): cut short at rest, and drawn whole over what
 * follows while the field holds the keyboard or the pointer is on the line, so nothing under it
 * moves. The box is described by the whole words, once.
 */
export const LongRefusal: Story = {
  args: { label: "Profile", start: "build", error: LONG_REFUSAL },
  render: (args) => (
    <div style={{ display: "grid", gap: "var(--space-4)", inlineSize: "20rem" }}>
      <Held {...args} />
      <Held label="Next" />
    </div>
  ),
  play: async ({ canvas, canvasElement, userEvent }) => {
    const box = canvas.getByRole("textbox", { name: "Profile" });
    const next = canvas.getByRole("textbox", { name: "Next" });
    await expect(box).toHaveAccessibleDescription(LONG_REFUSAL);
    const line = canvasElement.querySelector('[data-part="error"]') as HTMLElement;
    // The whole copy stands beside the live line, in its room, never in it.
    const room = line.parentElement as HTMLElement;
    await waitFor(() => expect(room).toHaveAttribute("data-clipped"));
    const whole = room.querySelector("[data-whole]") as HTMLElement;
    await expect(line.contains(whole)).toBe(false);
    await expect(whole).not.toBeVisible();
    const before = next.getBoundingClientRect().top;
    const height = line.getBoundingClientRect().height;
    await userEvent.tab();
    await expect(box).toHaveFocus();
    expectFocusInSight();
    // Whole, over what follows: the line keeps its height and the next field stays put.
    await expect(whole).toBeVisible();
    await expect(whole).toHaveTextContent(LONG_REFUSAL);
    await expect(whole.scrollWidth).toBeLessThanOrEqual(whole.clientWidth);
    await expect(line.getBoundingClientRect().height).toBe(height);
    await expect(next.getBoundingClientRect().top).toBe(before);
    // Leaving the field puts it back on its line.
    await userEvent.tab();
    await expect(next).toHaveFocus();
    await expect(whole).not.toBeVisible();
    // Under the pointer it is whole too: a synthetic hover never matches CSS `:hover`, so that
    // is held by the unit test through the cascade (`Field.test.tsx`).
    // The baseline: the box with the keyboard, its refusal drawn whole.
    await userEvent.tab({ shift: true });
    await expect(box).toHaveFocus();
    await expect(whole).toBeVisible();
    await expect(next.getBoundingClientRect().top).toBe(before);
  },
};

/** A dialog whose last field is refused at length: the create dialogs, whose body scrolls. */
function RefusedLast() {
  return (
    <Dialog
      onClose={() => {}}
      title="New workspace"
      subtitle="in smart-ide"
      main={{ children: "Create workspace", onPress: () => {} }}
    >
      <Held label="Name" start="docs" />
      <Held label="Profile" start="build" error={LONG_REFUSAL} />
    </Dialog>
  );
}

/**
 * **A long refusal on a dialog's last field** (#3033): the dialog's body scrolls, and would cut
 * the whole words off at its end. With the keyboard in the field they are brought into sight, and
 * the pointer goes through them to the answers they cover.
 */
export const LongRefusalLastInADialog: Story = {
  args: { label: "Profile" },
  parameters: { screenshot: "page" },
  render: () => <RefusedLast />,
  play: async ({ userEvent }) => {
    // A dialog is drawn over the page, outside the story's page; each part is found again where
    // it is asked about, as the dialog may draw its body anew as it opens.
    const page = within(document.body);
    const dialog = async () => page.findByRole("dialog", { name: "New workspace" });
    const whole = async () =>
      (await dialog()).querySelector(
        "[data-error-room][data-clipped] > [data-whole]",
      ) as HTMLElement;
    const body = async () =>
      (await dialog()).querySelector('[data-scope="dialog"][data-part="body"]') as HTMLElement;
    await untilFocused(
      () => page.getByRole("textbox", { name: "Name" }),
      "The dialog: the keyboard starts in Name",
    );
    await expect(page.getByRole("textbox", { name: "Profile" })).toHaveAccessibleDescription(
      LONG_REFUSAL,
    );
    await userEvent.tab();
    await expect(page.getByRole("textbox", { name: "Profile" })).toHaveFocus();
    expectFocusInSight();
    await waitFor(async () => expect(await whole()).toBeVisible());
    // In sight: its last line is inside what the body shows.
    await waitFor(async () =>
      expect(Math.round((await whole()).getBoundingClientRect().bottom)).toBeLessThanOrEqual(
        Math.round((await body()).getBoundingClientRect().bottom),
      ),
    );
    await expect(getComputedStyle(await whole()).pointerEvents).toBe("none");
  },
};

/** Each size, one box to a row: sm 24, md 28, lg 32, the last only for a list's main filter. */
export const Sizes: Story = {
  args: { "aria-label": "Filter" },
  render: () => (
    <div style={{ display: "grid", gap: "var(--space-4)", inlineSize: "20rem" }}>
      {FIELD_SIZES.map((size) => (
        <Held
          key={size}
          type="search"
          size={size}
          icon={<Search aria-hidden="true" />}
          aria-label={`Filter, ${size}`}
          placeholder={`Filter 28 personas (${size})`}
        />
      ))}
    </div>
  ),
  play: async ({ canvas, userEvent }) => {
    for (const size of FIELD_SIZES) {
      await userEvent.tab();
      await expect(canvas.getByRole("searchbox", { name: `Filter, ${size}` })).toHaveFocus();
      expectFocusInSight();
    }
  },
};

/**
 * **An sm field beside an sm button at a row's tightest gap** (#3250, WCAG 2.5.8): the compact
 * density draws the chrome at 18px, so sm's floor is 24 CSS px, as the button's is (#3210). Each
 * is a 24px target by itself in every cell, so axe's `target-size` passes with no spacing to
 * lean on.
 */
export const SmallBesideAButton: Story = {
  args: { "aria-label": "Filter the memories" },
  render: () => (
    <div
      style={{ display: "flex", alignItems: "center", gap: "var(--space-1)", inlineSize: "20rem" }}
    >
      <Held
        type="search"
        size="sm"
        icon={<Search aria-hidden="true" />}
        aria-label="Filter the memories"
        placeholder="Filter 28 memories"
      />
      <Button size="sm">Clear</Button>
    </div>
  ),
  play: async ({ canvas, userEvent }) => {
    const field = canvas.getByRole("searchbox", { name: "Filter the memories" });
    const box = field.parentElement as HTMLElement;
    const clear = canvas.getByRole("button", { name: "Clear" });
    for (const one of [box, clear]) {
      await expect(one.getBoundingClientRect().height).toBeGreaterThanOrEqual(24);
    }
    await userEvent.tab();
    await expect(field).toHaveFocus();
    expectFocusInSight();
  },
};

/** Unavailable: out of the tab sequence, drawn in the faint tier, nothing moves. */
export const Unavailable: Story = {
  args: { label: "Host", start: "git.example.com", isDisabled: true },
  render: (args) => (
    <div style={{ display: "grid", gap: "var(--space-4)" }}>
      <Held {...args} />
      <Button>After</Button>
    </div>
  ),
  play: async ({ canvas, userEvent }) => {
    await expect(canvas.getByRole("textbox", { name: "Host" })).toBeDisabled();
    await userEvent.tab();
    await expect(canvas.getByRole("button", { name: "After" })).toHaveFocus();
  },
};

/** Lines: Enter starts a new one, and the box is three controls tall at least. */
function Lines() {
  const [value, setValue] = useState("");
  return (
    <div style={{ inlineSize: "24rem" }}>
      <TextArea
        label="Hosts"
        help="One host per line."
        value={value}
        onChange={setValue}
        validate={(v) => (/ /.test(v) ? "One host per line, with no spaces." : undefined)}
      />
    </div>
  );
}

export const Textarea: Story = {
  args: { "aria-label": "Hosts" },
  render: () => <Lines />,
  play: async ({ canvas, userEvent }) => {
    const box = canvas.getByRole("textbox", { name: "Hosts" });
    await userEvent.tab();
    await expect(box).toHaveFocus();
    await userEvent.keyboard("github.com{Enter}gitlab.com");
    await expect(box).toHaveValue("github.com\ngitlab.com");
    expectFocusInSight();
    await userEvent.tab();
    await expect(canvas.queryByRole("alert")).toBeNull();
  },
};

/** A form: ⌘⏎ (Ctrl+Enter off macOS) sends its primary action from a box and from a textarea;
 *  a field that is not right stops it and takes the focus. */
function Sending() {
  const [name, setName] = useState("");
  const [note, setNote] = useState("");
  const [sent, setSent] = useState<string>();
  return (
    <div style={{ display: "grid", gap: "var(--space-4)", inlineSize: "24rem" }}>
      <Form
        aria-label="New workspace"
        style={{ display: "grid", gap: "var(--space-4)" }}
        onSubmit={(event) => {
          event.preventDefault();
          setSent(name);
        }}
      >
        <TextField label="Name" value={name} onChange={setName} validate={NAME_RULE} />
        <TextArea label="What it is for" value={note} onChange={setNote} />
        <Button type="submit" variant="primary">
          Create
        </Button>
      </Form>
      <p role="status" style={{ margin: 0 }}>
        {sent === undefined ? "Nothing sent" : `Sent ${sent}`}
      </p>
    </div>
  );
}

export const SendsOnModEnter: Story = {
  args: { "aria-label": "Name" },
  render: () => <Sending />,
  play: async ({ canvas, userEvent }) => {
    const name = canvas.getByRole("textbox", { name: "Name" });
    await userEvent.tab();
    await userEvent.keyboard("my name");
    await userEvent.tab();
    await userEvent.keyboard(SEND);
    // Not sent: the name is wrong, said under it, and the keyboard is back in it.
    await expect(canvas.getByRole("status")).toHaveTextContent("Nothing sent");
    await untilFocused(() => name, "Not sent: the keyboard is back in Name");
    expectFocusInSight();
    await userEvent.keyboard("{Backspace>5/}-name");
    await userEvent.tab();
    await userEvent.keyboard("A place for the docs.");
    await userEvent.keyboard(SEND);
    await waitFor(() => expect(canvas.getByRole("status")).toHaveTextContent("Sent my-name"));
  },
};

/** A list's main filter: its list is drawn again with each letter, and its box with it, and the
 *  keyboard and the caret stay where they were (R168). */
const PERSONAS = ["steward", "reviewer", "researcher", "release-manager", "designer"];

function Filtered() {
  const [words, setWords] = useState("");
  const shown = PERSONAS.filter((one) => one.includes(words.toLowerCase()));
  return (
    // The whole pane is keyed on what is typed, so React draws a new box for every letter.
    <div key={words} style={{ display: "grid", gap: "var(--space-3)", inlineSize: "18rem" }}>
      <TextField
        type="search"
        size="lg"
        icon={<ListFilter aria-hidden="true" />}
        keepsFocus="story-personas-filter"
        aria-label="Filter personas"
        placeholder={`Filter ${PERSONAS.length} personas`}
        value={words}
        onChange={setWords}
      />
      <ul aria-label="Personas" style={{ margin: 0 }}>
        {shown.map((one) => (
          <li key={one}>{one}</li>
        ))}
      </ul>
    </div>
  );
}

export const FilterKeepsTheCaret: Story = {
  args: { "aria-label": "Filter personas" },
  render: () => <Filtered />,
  play: async ({ canvas, userEvent }) => {
    await userEvent.tab();
    const box = () => canvas.getByRole("searchbox", { name: "Filter personas" });
    await userEvent.keyboard("re");
    await waitFor(() => expect(canvas.getAllByRole("listitem")).toHaveLength(3));
    await userEvent.keyboard("s");
    await waitFor(() => expect(canvas.getAllByRole("listitem")).toHaveLength(1));
    await expect(box()).toHaveValue("res");
    await expect(box()).toHaveFocus();
    // The caret moved into the words: the next redraw keeps it there.
    await userEvent.keyboard("{ArrowLeft}{Backspace}");
    await waitFor(() => expect(box()).toHaveValue("rs"));
    await expect(box()).toHaveFocus();
    await expect((box() as HTMLInputElement).selectionStart).toBe(1);
    expectFocusInSight();
  },
};
