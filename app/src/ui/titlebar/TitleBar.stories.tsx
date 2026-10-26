// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The title bar's workspaces** (#2916, R90–R115): the project and its pinned workspaces as
 * `prototype/switcher` draws them, the ⌄'s workspace popover, and the two-column switcher the
 * project's crumb opens while the rail is hidden. Each held to axe, walked by the keyboard and
 * compared with its baseline (`docs/component-stories.md`). A popover draws outside its frame,
 * so those pictures are the page's.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { useMemo, useState } from "react";
import { expect, screen, waitFor, within } from "storybook/test";
import { OUTSIDE } from "../../actions";
import { expectFocusInSight } from "../../stories.testkit";
import { TitleBarSwitch, type SwitchProject } from "../../TitleBarSwitch";

/** The prototype's projects and workspaces, with what waits in each. */
const START: { plane: string; name: string; ws: [string, number][]; pins: string[] }[] = [
  {
    plane: "/p/charter",
    name: "charter",
    ws: [
      ["smart-ide", 1],
      ["charter", 0],
      ["skills", 0],
    ],
    pins: ["smart-ide", "charter", "skills"],
  },
  {
    plane: "/p/volaticloud",
    name: "volaticloud",
    ws: [
      ["console-ux", 2],
      ["platform", 0],
    ],
    pins: ["console-ux"],
  },
  {
    plane: "/p/easydmarc-umbrella",
    name: "easydmarc-umbrella",
    ws: [
      ["keycloak", 2],
      ["invites", 1],
      ["reporting", 0],
      ["sending-domains", 0],
    ],
    pins: ["keycloak", "invites"],
  },
];

function TheBar({ railShown = true, focusedAt = "smart-ide" }) {
  const [front, setFront] = useState(START[0].plane);
  const [focused, setFocused] = useState<Record<string, string>>({
    [START[0].plane]: focusedAt,
    [START[1].plane]: "console-ux",
    [START[2].plane]: "keycloak",
  });
  const [pins, setPins] = useState<Record<string, string[]>>(
    Object.fromEntries(START.map((one) => [one.plane, one.pins])),
  );
  const [said, setSaid] = useState("Nothing chosen");
  const projects = useMemo<SwitchProject[]>(
    () =>
      START.map((one) => ({
        plane: one.plane,
        name: one.name,
        waiting: one.ws.reduce((n, [, w]) => n + w, 0),
        workspaces: {
          names: [
            ...pins[one.plane],
            ...one.ws.map(([name]) => name).filter((name) => !pins[one.plane].includes(name)),
          ],
          pins: pins[one.plane],
          focused: focused[one.plane],
          waiting: Object.fromEntries([[OUTSIDE, 0], ...one.ws]),
        },
      })),
    [focused, pins],
  );
  return (
    <div style={{ display: "grid", gap: "var(--space-6)" }}>
      <header
        className="title-bar"
        data-testid="title-bar"
        style={{ inlineSize: "56rem", ["--window-controls" as string]: "78px" }}
      >
        <span className="title-bar-leading">
          <TitleBarSwitch
            projects={projects}
            inFront={front}
            railShown={railShown}
            does={{
              go: (plane, workspace) => {
                setFront(plane);
                setFocused((was) => ({ ...was, [plane]: workspace }));
                setSaid(`Went to ${workspace}`);
              },
              choose: (plane) => setFront(plane),
              pin: (plane, workspace, pinned) =>
                setPins((was) => ({
                  ...was,
                  [plane]: pinned
                    ? [...was[plane], workspace]
                    : was[plane].filter((one) => one !== workspace),
                })),
              create: () => setSaid("New workspace"),
            }}
          />
        </span>
      </header>
      <p role="status" style={{ margin: 0 }}>
        {said}
      </p>
    </div>
  );
}

const meta = {
  component: TheBar,
  parameters: { screenshot: "page" },
} satisfies Meta<typeof TheBar>;

export default meta;
type Story = StoryObj<typeof meta>;

/**
 * **What keeps `element` from being seen**, or nothing: its box, then each element up to the
 * document, by the properties a visibility check reads.
 */
function whatHides(element: Element): string {
  const box = element.getBoundingClientRect();
  if (box.width === 0 || box.height === 0) return `no size: ${box.width}x${box.height}`;
  if (box.bottom <= 0 || box.top >= window.innerHeight)
    return `out of the viewport: ${box.top}..${box.bottom} of ${window.innerHeight}`;
  for (let at: Element | null = element; at !== null; at = at.parentElement) {
    const style = getComputedStyle(at);
    const named = `${at.tagName.toLowerCase()}[${at.getAttribute("data-part") ?? at.className}]`;
    if (style.display === "none") return `display none on ${named}`;
    if (style.visibility !== "visible") return `visibility ${style.visibility} on ${named}`;
    if (Number(style.opacity) === 0) return `opacity 0 on ${named}`;
    if (at.hasAttribute("hidden")) return `hidden on ${named}`;
  }
  return "";
}

const chipsOf = (bar: HTMLElement) =>
  [...bar.querySelectorAll<HTMLElement>('[data-part="workspace"]')].filter(
    (one) => one.dataset.scope === "titlebar",
  );

/**
 * **The pins** (R92, R101, R102): the project in quiet words, a faint slash, a chip per pin 28px
 * tall with its amber count, the current one lit. Choosing another moves nothing beside it.
 */
export const Pins: Story = {
  play: async ({ canvas, userEvent }) => {
    const bar = canvas.getByTestId("title-bar");
    const chips = chipsOf(bar);
    await expect(chips.map((one) => one.textContent)).toEqual(["smart-ide1", "charter", "skills"]);
    for (const one of chips) await expect(Math.round(one.getBoundingClientRect().height)).toBe(28);
    await expect(chips[0]).toHaveAttribute("aria-current", "true");
    const before = chips.map((one) => Math.round(one.getBoundingClientRect().left));
    await userEvent.click(chips[2]);
    await waitFor(() => expect(chips[2]).toHaveAttribute("aria-current", "true"));
    await expect(chips.map((one) => Math.round(one.getBoundingClientRect().left))).toEqual(before);
    // Its tooltip says its key (R102).
    await userEvent.hover(chips[1]);
    await waitFor(() =>
      expect(document.querySelector('[role="tooltip"][data-scope="tooltip"]')).toHaveTextContent(
        /charter · (⌘|Ctrl\+)2/,
      ),
    );
    await userEvent.unhover(chips[1]);
    await waitFor(() =>
      expect(document.querySelector('[role="tooltip"][data-scope="tooltip"]')).toBeNull(),
    );
    // The keyboard: one stop, whose arrows walk the chips.
    chips[0].focus();
    expectFocusInSight();
    await userEvent.keyboard("{ArrowRight}");
    await expect(chips[1]).toHaveFocus();
    await userEvent.keyboard("{Escape}");
  },
};

/** **The dashed chip** (R105): the current workspace while it is not pinned, after the pins. */
export const Unpinned: Story = {
  args: { focusedAt: "docs" },
  play: async ({ canvas }) => {
    const chips = chipsOf(canvas.getByTestId("title-bar"));
    const last = chips[chips.length - 1];
    await expect(last).toHaveAttribute("data-variant", "unpinned");
    await expect(last).toHaveTextContent("docs+");
  },
};

/**
 * **A name with descenders** (#3326): `git-ci-cd`, dashed while it is not pinned, then lit by
 * the pointer. The name's box holds the font's whole height, ascent to descent, so the clip that
 * draws its ellipsis never cuts a g, j, p, q or y.
 */
export const Descenders: Story = {
  args: { focusedAt: "git-ci-cd" },
  play: async ({ canvas, userEvent }) => {
    const chips = chipsOf(canvas.getByTestId("title-bar"));
    const chip = chips[chips.length - 1];
    await expect(chip).toHaveTextContent("git-ci-cd+");
    const fits = () => {
      const name = chip.querySelector<HTMLElement>('[data-part="name"]');
      if (name === null) return "no name part";
      const style = getComputedStyle(name);
      const pen = document.createElement("canvas").getContext("2d");
      if (pen === null) return "no canvas";
      pen.font = `${style.fontWeight} ${style.fontSize} ${style.fontFamily}`;
      const ink = pen.measureText(name.textContent ?? "");
      const tall = ink.fontBoundingBoxAscent + ink.fontBoundingBoxDescent;
      const box = name.getBoundingClientRect().height;
      return box + 0.5 >= tall ? "" : `the name is ${box}px tall, the font ${tall}px`;
    };
    await expect(fits()).toBe("");
    await userEvent.hover(chip);
    await waitFor(() => expect(chip).toHaveAttribute("data-hovered"));
    await expect(fits()).toBe("");
    await expect(Math.round(chip.getBoundingClientRect().height)).toBe(28);
    await userEvent.unhover(chip);
  },
};

/**
 * **The workspace popover** (R106–R109): 320px under the ⌄, its filter with the key, the root
 * first, the pins, the rest, a pin toggle on each and New workspace… last; the keys in its
 * footer. Typing chooses the first match; ↑ and ↓ stop at the ends; Escape closes it.
 */
export const WorkspacePopover: Story = {
  play: async ({ canvas, userEvent }) => {
    const all = canvas.getByRole("button", { name: "All workspaces" });
    await userEvent.click(all);
    const filter = await screen.findByRole("combobox", {
      name: "Go to a workspace in charter…",
    });
    await waitFor(() => expect(filter).toHaveFocus());
    const popover = filter.closest<HTMLElement>('[data-scope="popover"][data-part="root"]');
    await expect(Math.round(popover?.getBoundingClientRect().width ?? 0)).toBe(320);
    const list = screen.getByRole("listbox", { name: "Workspaces in charter" });
    await expect(within(list).getAllByRole("option")).toHaveLength(5);
    await userEvent.keyboard("{ArrowDown}{ArrowDown}{ArrowDown}{ArrowDown}{ArrowDown}");
    await expect(within(list).getAllByRole("option")[4]).toHaveAttribute("aria-selected", "true");
    await userEvent.keyboard("{ArrowUp}{ArrowUp}{ArrowUp}");
    expectFocusInSight();
  },
};

/**
 * **The two-column switcher** (R93–R100), while the rail is hidden: the crumb opens it, 560px,
 * the projects with their marks and counts, and the workspaces of the one chosen with their pins.
 * → steps across to the workspaces.
 */
export const ProjectSwitcher: Story = {
  args: { railShown: false },
  play: async ({ canvas, userEvent }) => {
    const crumb = canvas.getByTestId("title-project");
    await expect(Math.round(crumb.getBoundingClientRect().height)).toBe(28);
    await userEvent.click(crumb);
    const filter = await screen.findByRole("combobox", { name: "Go to a project or workspace…" });
    await waitFor(() => expect(filter).toHaveFocus());
    const popover = filter.closest<HTMLElement>('[data-scope="popover"][data-part="root"]');
    await expect(Math.round(popover?.getBoundingClientRect().width ?? 0)).toBe(560);
    await userEvent.keyboard("{ArrowDown}");
    await expect(
      within(screen.getByRole("listbox", { name: "Projects" })).getAllByRole("option")[1],
    ).toHaveAttribute("aria-selected", "true");
    // The workspaces of the project chosen, beside it: shown, not switched to (R97). Where it is
    // not seen, what hides it is said first, so a red run names the cause.
    const shown = screen.getByRole("listbox", { name: "Workspaces in volaticloud" });
    await waitFor(() => expect(whatHides(shown)).toBe(""));
    await expect(shown).toBeVisible();
    await userEvent.keyboard("{ArrowRight}");
    expectFocusInSight();
  },
};
