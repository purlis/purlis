// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The dialog family** (#2910): a form, a question, a dialog whose body is longer than the
 * window, a list beside its details, and text that changes with a choice. Each is drawn, held to
 * axe, walked by the keyboard, and compared with its baseline in Chromium and WebKit
 * (`docs/component-stories.md`). Every picture is of the page, a dialog being drawn over it, and of
 * the dialog open: a story that answers it takes its picture first (`pictureNow`).
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState, type ReactNode } from "react";
import { expect, waitFor, within } from "storybook/test";
import { expectFocusInSight, pictureNow } from "../../stories.testkit";
import { Menu, MenuItem, MenuTrigger, MenuWords } from "../menu/Menu";
import { Button } from "../button/Button";
import { TextField } from "../field/Field";
import { AlertDialog, Dialog, type DialogSize } from "./Dialog";
import { ListFilter, noMatchSaid, useListFilter } from "../list/ListFilter";
import { Reserved } from "./Reserved";

/** What a person reads after answering: which answer was taken, in words. */
function Said({ said }: { said: string }) {
  return (
    <p role="status" style={{ margin: 0 }}>
      {said}
    </p>
  );
}

/** A dialog that opens from a button and says, once it is gone, how it was answered. */
function Opened({
  label,
  children,
}: {
  label: string;
  children: (close: (said: string) => void) => ReactNode;
}) {
  const [open, setOpen] = useState(true);
  const [said, setSaid] = useState("Not answered yet");
  const close = (how: string) => {
    setSaid(how);
    setOpen(false);
  };
  return (
    <div style={{ display: "grid", gap: "var(--space-3)", justifyItems: "start" }}>
      <Button onPress={() => setOpen(true)}>{label}</Button>
      <Said said={said} />
      {open && children(close)}
    </div>
  );
}

/** A box labelled for what it holds. */
function Box({ label }: { label: string }) {
  const [value, setValue] = useState("");
  return (
    <div style={{ marginBlockEnd: "var(--space-5)" }}>
      <TextField label={label} value={value} onChange={setValue} />
    </div>
  );
}

function Form({ size = "standard" }: { size?: DialogSize }) {
  return (
    <Opened label="New workspace">
      {(close) => (
        <Dialog
          size={size}
          onClose={() => close("Cancelled")}
          title="New workspace"
          subtitle="in smart-ide"
          main={{ children: "Create workspace", onPress: () => close("Created") }}
        >
          <Box label="Name" />
        </Dialog>
      )}
    </Opened>
  );
}

const meta = {
  component: Form,
  parameters: { screenshot: "page" },
} satisfies Meta<typeof Form>;

export default meta;
type Story = StoryObj<typeof meta>;

/** The answers, in their places: Cancel first, the main action last. */
async function answersInPlace(dialog: HTMLElement, names: string[]) {
  const row = dialog.querySelector('[data-scope="dialog"][data-part="actions"]');
  const buttons = [...(row?.querySelectorAll("button") ?? [])];
  for (const [at, name] of names.entries()) await expect(buttons[at]).toHaveAccessibleName(name);
  await expect(buttons).toHaveLength(names.length);
}

/**
 * **A form**: it starts on its field, ⏎ there is its main action, Tab goes round its answers and
 * back, and the focus is always in sight.
 */
export const AForm: Story = {
  play: async ({ userEvent }) => {
    // A dialog is drawn over the page, outside the story's page.
    const page = within(document.body);
    const dialog = await page.findByRole("dialog", { name: "New workspace" });
    await answersInPlace(dialog, ["Cancel", "Create workspace"]);
    const name = page.getByRole("textbox", { name: "Name" });
    await waitFor(() => expect(name).toHaveFocus());
    expectFocusInSight();
    await pictureNow();
    await userEvent.tab();
    await expect(page.getByRole("button", { name: "Cancel" })).toHaveFocus();
    expectFocusInSight();
    await userEvent.tab();
    await expect(page.getByRole("button", { name: "Create workspace" })).toHaveFocus();
    expectFocusInSight();
    // The keyboard stays in the dialog: past its last answer is its first field.
    await userEvent.tab();
    await expect(name).toHaveFocus();
    await userEvent.type(name, "docs{Enter}");
    await waitFor(() =>
      expect(page.getByRole("status", { hidden: true })).toHaveTextContent("Created"),
    );
    await expect(page.queryByRole("dialog")).toBeNull();
  },
};

/** **Esc closes it**, and the keyboard goes back to what had it. */
export const EscapeCloses: Story = {
  play: async ({ userEvent }) => {
    // A dialog is drawn over the page, outside the story's page.
    const page = within(document.body);
    await page.findByRole("dialog", { name: "New workspace" });
    await waitFor(() => expect(page.getByRole("textbox", { name: "Name" })).toHaveFocus());
    await pictureNow();
    await userEvent.keyboard("{Escape}");
    await waitFor(() =>
      expect(page.getByRole("status", { hidden: true })).toHaveTextContent("Cancelled"),
    );
    await expect(page.queryByRole("dialog")).toBeNull();
  },
};

function WithAMenu() {
  return (
    <Opened label="Workspace">
      {(close) => (
        <Dialog
          onClose={() => close("Cancelled")}
          title="Workspace"
          main={{ children: "Save", onPress: () => close("Saved") }}
        >
          <MenuTrigger>
            <Button>More</Button>
            <Menu aria-label="More">
              <MenuItem id="rename" textValue="Rename">
                <MenuWords said="Rename" />
              </MenuItem>
            </Menu>
          </MenuTrigger>
        </Dialog>
      )}
    </Opened>
  );
}

/** **Esc closes one layer**: a menu open in the dialog first, then the dialog. */
export const EscapeClosesOneLayer: Story = {
  render: () => <WithAMenu />,
  play: async ({ userEvent }) => {
    const page = within(document.body);
    await page.findByRole("dialog", { name: "Workspace" });
    const more = page.getByRole("button", { name: "More" });
    await waitFor(() => expect(more).toHaveFocus());
    await pictureNow();
    await userEvent.keyboard("{Enter}");
    const menu = await page.findByRole("menu");
    // React Aria puts the keyboard on the menu's first row on the next frame, which WebKit may
    // draw late: the Esc is the menu's once the keyboard is in it.
    await waitFor(() =>
      expect(within(menu).getByRole("menuitem", { name: "Rename" })).toHaveFocus(),
    );
    await userEvent.keyboard("{Escape}");
    // A menu may take a moment to go: WebKit draws the next frame late in a busy run (as the menu
    // family's own stories allow).
    await waitFor(() => expect(page.queryByRole("menu")).toBeNull(), { timeout: 3_000 });
    await waitFor(() => expect(more).toHaveFocus());
    await expect(page.getByRole("dialog", { name: "Workspace" })).toBeVisible();
    await expect(page.getByRole("status", { hidden: true })).toHaveTextContent("Not answered yet");
    await userEvent.keyboard("{Escape}");
    await waitFor(() =>
      expect(page.getByRole("status", { hidden: true })).toHaveTextContent("Cancelled"),
    );
  },
};

function Question() {
  return (
    <Opened label="Delete persona">
      {(close) => (
        <AlertDialog
          onClose={() => close("Kept")}
          title="Delete persona steward?"
          description={<p>Its definition, its memory and its reference files go with it.</p>}
          main={{ children: "Delete persona", variant: "danger", onPress: () => close("Deleted") }}
        />
      )}
    </Opened>
  );
}

/**
 * **A question that cannot be taken back**: it starts on its safe answer, so ⏎ keeps everything,
 * and its act is pressed, never found.
 */
export const AQuestion: Story = {
  render: () => <Question />,
  play: async ({ userEvent }) => {
    // A dialog is drawn over the page, outside the story's page.
    const page = within(document.body);
    const alert = await page.findByRole("alertdialog", { name: "Delete persona steward?" });
    await answersInPlace(alert, ["Cancel", "Delete persona"]);
    await waitFor(() => expect(page.getByRole("button", { name: "Cancel" })).toHaveFocus());
    expectFocusInSight();
    await pictureNow();
    await userEvent.keyboard("{Enter}");
    await waitFor(() =>
      expect(page.getByRole("status", { hidden: true })).toHaveTextContent("Kept"),
    );
  },
};

function Long() {
  const fields = Array.from({ length: 14 }, (_, at) => `Field ${at + 1}`);
  return (
    <Opened label="Ask a persona">
      {(close) => (
        <Dialog
          onClose={() => close("Cancelled")}
          title="Ask steward"
          subtitle="From docs 1"
          main={{ children: "Ask steward", onPress: () => close("Asked") }}
        >
          {fields.map((label) => (
            <Box key={label} label={label} />
          ))}
        </Dialog>
      )}
    </Opened>
  );
}

/** Whether `inner` is drawn wholly inside `outer`'s box. */
const inside = (inner: DOMRect, outer: DOMRect) =>
  inner.top >= outer.top - 0.5 && inner.bottom <= outer.bottom + 0.5;

/**
 * **A body longer than the window**: the dialog is the window less 48 px at most, the body
 * scrolls, the header and the footer stay in view, and every field the keyboard reaches is in
 * sight, never under the footer (WCAG 2.4.11).
 */
export const LongerThanTheWindow: Story = {
  render: () => <Long />,
  play: async ({ userEvent }) => {
    // A dialog is drawn over the page, outside the story's page.
    const page = within(document.body);
    const dialog = await page.findByRole("dialog", { name: "Ask steward" });
    const body = dialog.querySelector('[data-part="body"]') as HTMLElement;
    const footer = dialog.querySelector('[data-part="footer"]') as HTMLElement;
    await waitFor(() => expect(page.getByRole("textbox", { name: "Field 1" })).toHaveFocus());
    await expect(dialog.getBoundingClientRect().height).toBeLessThanOrEqual(
      window.innerHeight - 48 + 0.5,
    );
    for (let at = 1; at <= 14; at += 1) {
      const field = page.getByRole("textbox", { name: `Field ${at}` });
      await expect(field).toHaveFocus();
      expectFocusInSight();
      await expect(inside(field.getBoundingClientRect(), body.getBoundingClientRect())).toBe(true);
      const box = field.getBoundingClientRect();
      const under = footer.getBoundingClientRect();
      await expect(box.bottom <= under.top + 0.5 || box.top >= under.bottom - 0.5).toBe(true);
      await userEvent.tab();
    }
    // The answers, in view all along.
    await expect(page.getByRole("button", { name: "Cancel" })).toHaveFocus();
    expectFocusInSight();
    await expect(inside(footer.getBoundingClientRect(), dialog.getBoundingClientRect())).toBe(true);
  },
};

const PERSONAS = [
  "steward",
  "designer",
  "reviewer",
  "release-captain",
  "security",
  "docs-writer",
  "triager",
  "tester",
  "researcher",
  "ops",
];

const NOUN = { one: "persona", many: "personas" };

function Picker() {
  const { query, setQuery, shown } = useListFilter(PERSONAS, (one) => one);
  const [picked, setPicked] = useState("steward");
  const harness = picked === "steward" ? "Claude Code" : "opencode";
  return (
    <Opened label="Start a chat">
      {(close) => (
        <Dialog
          size="wide"
          height={620}
          onClose={() => close("Cancelled")}
          title="Start a chat"
          subtitle="In smart-ide · purlis's built-in profiles"
          summary={`${picked} on ${harness}`}
          main={{ children: "Start chat", onPress: () => close(`Started ${picked}`) }}
        >
          <div style={{ display: "grid", gridTemplateColumns: "15rem 1fr", gap: "var(--space-6)" }}>
            <div>
              <ListFilter
                noun={NOUN}
                total={PERSONAS.length}
                query={query}
                onQuery={setQuery}
                controls="story-personas"
              />
              <div id="story-personas">
                {shown.length === 0 ? (
                  <p role="status">{noMatchSaid(NOUN, query)}</p>
                ) : (
                  <div role="radiogroup" aria-label="Persona">
                    {shown.map((one) => (
                      <label key={one} style={{ display: "flex", gap: "var(--space-3)" }}>
                        <input
                          type="radio"
                          name="persona"
                          checked={picked === one}
                          onChange={() => setPicked(one)}
                        />
                        {one}
                      </label>
                    ))}
                  </div>
                )}
              </div>
            </div>
            <div>
              <h3 style={{ marginTop: 0 }}>{picked}</h3>
              <Reserved as="div" texts={["Billed to Claude Code.", "Billed to opencode."]}>
                {`Billed to ${harness}.`}
              </Reserved>
            </div>
          </div>
        </Dialog>
      )}
    </Opened>
  );
}

/**
 * **A list beside its details**: the wide size, a fixed height, a list over seven items under
 * its filter (which keeps its focus and caret as the list narrows), and text that changes with
 * the choice keeping its room, so choosing moves and resizes nothing.
 */
export const AListBesideItsDetails: Story = {
  render: () => <Picker />,
  play: async ({ userEvent }) => {
    // A dialog is drawn over the page, outside the story's page.
    const page = within(document.body);
    const dialog = await page.findByRole("dialog", { name: "Start a chat" });
    await expect(dialog).toHaveAttribute("data-size", "wide");
    const filter = page.getByRole("searchbox", { name: "Filter personas" });
    await expect(filter).toHaveAttribute("placeholder", "Filter 10 personas");
    await waitFor(() => expect(filter).toHaveFocus());
    const before = dialog.getBoundingClientRect();
    await userEvent.type(filter, "re");
    await expect(filter).toHaveFocus();
    await expect(filter).toHaveValue("re");
    await expect(page.getAllByRole("radio")).toHaveLength(3);
    await userEvent.click(page.getByRole("radio", { name: "reviewer" }));
    const after = dialog.getBoundingClientRect();
    await expect([after.width, after.height, after.top]).toEqual([
      before.width,
      before.height,
      before.top,
    ]);
    await userEvent.clear(filter);
    await userEvent.type(filter, "zz");
    // The line is said once the list has narrowed to nothing, in the list's place.
    await waitFor(() => expect(page.getByText("No persona is called “zz”.")).toBeVisible());
    expectFocusInSight();
  },
};
