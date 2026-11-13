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
import { Button } from "../button/Button";
import { expectFocusInSight, penFor, untilFocused } from "../../stories.testkit";
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

/** The scale spec's 1,000 workspaces (#5501), `ws-0000` to `ws-0999`, every seventh waiting. */
const THOUSAND: [string, number][] = Array.from({ length: 1_000 }, (_, at) => [
  `ws-${String(at).padStart(4, "0")}`,
  at % 7 === 0 ? 1 : 0,
]);

function TheBar({
  railShown = true,
  focusedAt = "smart-ide",
  many = false,
  working: workingAt = [] as string[],
}) {
  /** The workspaces of the project in front a chat is working in (#3127), one chat each. */
  const [working, setWorking] = useState<readonly string[]>(workingAt);
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
      START.map((given) =>
        many && given.plane === START[0].plane
          ? { ...given, ws: [...given.ws, ...THOUSAND] }
          : given,
      ).map((one) => ({
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
          working:
            one.plane === START[0].plane
              ? Object.fromEntries(working.map((name) => [name, 1]))
              : undefined,
          // A chip's drag (#3128): the pins in the order they were dropped in.
          arrange: (order: readonly string[]) => {
            setPins((was) => ({ ...was, [one.plane]: [...order] }));
            setSaid(`Pins: ${order.join(", ")}`);
          },
        },
      })),
    [focused, pins, many, working],
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
      {/* The story's own word, by its test id: a drag's live region is a status too. */}
      <p role="status" data-testid="story-said" style={{ margin: 0 }}>
        {said}
      </p>
      {workingAt.length > 0 && (
        <span>
          <Button
            variant="secondary"
            size="sm"
            onPress={() =>
              setWorking((was) =>
                was.includes("charter")
                  ? was.filter((one) => one !== "charter")
                  : [...was, "charter"],
              )
            }
          >
            A chat in charter
          </Button>
        </span>
      )}
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

/** A computed CSS colour as red, green, blue (0–255) and alpha (0–1). */
function rgba(colour: string): [number, number, number, number] {
  const [r = 0, g = 0, b = 0, a = 1] = (colour.match(/[\d.]+/g) ?? []).map(Number);
  return [r, g, b, colour.startsWith("rgba") || colour.includes("/") ? a : 1];
}

/** The first opaque fill behind `element`: what a translucent fill on it is laid over. */
function groundOf(element: Element): string {
  for (let at = element.parentElement; at !== null; at = at.parentElement) {
    const fill = getComputedStyle(at).backgroundColor;
    if (rgba(fill)[3] === 1) return fill;
  }
  return getComputedStyle(document.body).backgroundColor;
}

/** `fill` laid over the opaque `ground`, as the screen draws it. */
function composited(fill: string, ground: string): [number, number, number] {
  const [r, g, b, a] = rgba(fill);
  const [R, G, B] = rgba(ground);
  return [r * a + R * (1 - a), g * a + G * (1 - a), b * a + B * (1 - a)];
}

/** How far apart two drawn colours are, the largest of their channels' differences. */
/** WCAG's relative luminance of a drawn colour. */
function luminance([r, g, b]: number[]): number {
  const lin = (channel: number) => {
    const c = channel / 255;
    return c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
  };
  return 0.2126 * lin(r) + 0.7152 * lin(g) + 0.0722 * lin(b);
}

/** **How a chip's waiting count reads** (D-3345-3): its amber words over its wash laid on its
 *  own opaque ground, as drawn, and that ground. */
function countOn(count: HTMLElement): { ratio: number; ground: string } {
  const style = getComputedStyle(count);
  const ground = getComputedStyle(count.parentElement ?? count).backgroundColor;
  const drawn = composited(style.backgroundColor, ground);
  const words = composited(style.color, `rgb(${drawn.join(", ")})`);
  const [light, dark] = [luminance(words), luminance(drawn)].sort((a, b) => b - a);
  return { ratio: (light + 0.05) / (dark + 0.05), ground };
}

function apart(one: number[], other: number[]): number {
  return Math.max(...one.map((channel, at) => Math.abs(channel - other[at])));
}

const chipsOf = (bar: HTMLElement) =>
  [...bar.querySelectorAll<HTMLElement>('[data-part="workspace"]')].filter(
    // The plane root's house is not a workspace's chip (#3654).
    (one) => one.dataset.scope === "titlebar" && one.dataset.variant !== "root",
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

/**
 * **The chosen chip, in every state** (R101 as amended by #3345, the sidebar-views prototype's
 * Q4 A): the chosen chip on the accent's wash, any other under the pointer on the card's fill,
 * the chosen one under the pointer on the stronger wash, and the keyboard's ring. Each state
 * draws its own fill in every built-in theme, and none moves the chip's box by a pixel. The
 * picture is the chosen chip under the pointer.
 */
export const ChosenStates: Story = {
  play: async ({ canvas, userEvent }) => {
    const chips = chipsOf(canvas.getByTestId("title-bar"));
    const [chosen, other] = chips;
    await expect(chosen).toHaveAttribute("data-selected");
    const box = (one: HTMLElement) => {
      const { width, height } = one.getBoundingClientRect();
      return `${width}x${height}`;
    };
    const boxes = [box(chosen), box(other)];
    const fill = (one: HTMLElement) => getComputedStyle(one).backgroundColor;
    const fills: Record<string, string> = { rest: fill(other), chosen: fill(chosen) };
    // The pointer may rest where a story before this one left it: it leaves the bar and comes
    // back until the chip hears it arrive, so the chip is surely under it. What the chip draws
    // is read at that moment: a tooltip that opens later may lie under the pointer and take
    // the chip's hover away.
    const point = (one: HTMLElement) =>
      waitFor(async () => {
        await userEvent.hover(canvas.getByText("Nothing chosen"));
        await userEvent.hover(one);
        await expect(one).toHaveAttribute("data-hovered");
        return { fill: fill(one), color: getComputedStyle(one).color, box: box(one) };
      });
    const hovered = await point(other);
    fills.hover = hovered.fill;
    await expect(hovered.box).toBe(boxes[1]);
    // The primary words on the wash, as under the pointer.
    await expect(getComputedStyle(chosen).color).toBe(hovered.color);
    // The pointer leaves, and the tooltip it opened goes, before it reaches the chosen chip.
    await userEvent.hover(canvas.getByText("Nothing chosen"));
    await waitFor(() => expect(other).not.toHaveAttribute("data-hovered"));
    await waitFor(() =>
      expect(document.querySelector('[role="tooltip"][data-scope="tooltip"]')).toBeNull(),
    );
    const both = await point(chosen);
    fills["chosen + hover"] = both.fill;
    await expect(both.box).toBe(boxes[0]);
    // Every state its own fill: the chosen chip never looks hovered, nor at rest.
    // Compared as drawn: each fill laid over the ground the bar stands on, so a translucent wash
    // and an opaque fill are told apart by the colour they make, not by how they are written.
    const drawn = Object.values(fills).map((one) => composited(one, groundOf(chosen)));
    for (const [at, one] of drawn.entries())
      for (const other of drawn.slice(at + 1))
        await expect(apart(one, other), `${one} and ${other}`).toBeGreaterThan(2);
    await userEvent.unhover(chosen);
    // The keyboard's ring on the chosen chip moves nothing either.
    chosen.focus();
    await userEvent.keyboard("{ArrowRight}");
    await userEvent.keyboard("{ArrowLeft}");
    await expect(chosen).toHaveFocus();
    await waitFor(() => expect(chosen).toHaveAttribute("data-focus-visible"));
    await expect(box(chosen)).toBe(boxes[0]);
    // Its waiting count stands on its own ground, the bar's, chosen and under the pointer as at
    // rest, and its amber reads at 4.5:1 there (D-3345-3).
    const count = chosen.querySelector<HTMLElement>('[data-part="count"]');
    if (count === null) throw new Error("the chosen chip draws no count");
    const lit = countOn(count);
    await expect(rgba(lit.ground)[3]).toBe(1);
    await expect(lit.ratio).toBeGreaterThanOrEqual(4.5);
    await expect(fill(chosen)).toBe(fills.chosen);
    // The picture: the chosen chip under the pointer.
    chosen.blur();
    await point(chosen);
  },
};

/**
 * **A chip's working mark** (#3127): while a chat is working in a workspace, its chip's slot holds
 * the chat mark's working ring on the chip's lower trailing corner, ringed in the bar's ground.
 * A shape, said in the chip's description, and still; coming and going it moves no chip by a
 * pixel. The picture is smart-ide (current, waiting) beside charter and skills, each with a
 * chat working.
 */
export const Working: Story = {
  args: { working: ["skills", "charter"] },
  play: async ({ canvas, userEvent }) => {
    const bar = canvas.getByTestId("title-bar");
    const chips = chipsOf(bar);
    const named = (name: string) => chips.find((one) => one.dataset.chipOf === name);
    const markOf = (chip: HTMLElement | undefined) =>
      chip
        ?.closest('[data-part="slot"]')
        ?.querySelector<HTMLElement>('[data-scope="chat-mark"][data-variant="working"]') ?? null;
    const charter = named("charter");
    const smartIde = named("smart-ide");
    await expect(markOf(charter)).not.toBeNull();
    await expect(markOf(named("skills"))).not.toBeNull();
    await expect(markOf(smartIde)).toBeNull();
    await expect(charter).toHaveAccessibleDescription("1 chat working");
    await expect(smartIde).toHaveAccessibleDescription("1 waiting");
    // A ring the eye reads on the bar's ground: 3:1, as a mark (WCAG 1.4.11).
    const badge = markOf(charter)?.querySelector<HTMLElement>('[data-part="badge"]');
    if (badge === null || badge === undefined) throw new Error("the mark draws no badge");
    const ground = composited(getComputedStyle(badge).backgroundColor, groundOf(badge));
    const ring = composited(getComputedStyle(badge).color, `rgb(${ground.join(", ")})`);
    const [light, dark] = [luminance(ring), luminance(ground)].sort((a, b) => b - a);
    await expect((light + 0.05) / (dark + 0.05)).toBeGreaterThanOrEqual(3);
    // Still: it never moves itself.
    await expect(badge.getAnimations()).toEqual([]);
    // Coming and going, it moves no chip by a pixel, and a press on the chip still reaches it.
    const boxes = () =>
      chips.map((one) => {
        const { left, width, top, height } = one.getBoundingClientRect();
        return `${left},${top} ${width}x${height}`;
      });
    const before = boxes();
    const toggle = canvas.getByRole("button", { name: "A chat in charter" });
    await userEvent.click(toggle);
    await waitFor(() => expect(markOf(charter)).toBeNull());
    await expect(charter).not.toHaveAttribute("aria-describedby");
    await expect(boxes()).toEqual(before);
    await userEvent.click(toggle);
    await waitFor(() => expect(markOf(charter)).not.toBeNull());
    await expect(boxes()).toEqual(before);
    // The keyboard walks the chips as ever: Tab past the house to the strip's one stop, the
    // arrows along it, and the mark is no stop of its own.
    await walkTheStrip(userEvent, bar, "Working");
    // Tab leaves the strip for what follows it.
    await userEvent.tab();
    await untilFocused(() => toggle, "Working: Tab leaves the strip");
  },
};

/**
 * **A pin dragged along the strip** (#3128): the pointer carries a chip past its neighbour, by a
 * translation only, so no chip changes size while it moves, and the neighbour moves to make the
 * drop place. Dropped, the order is the new one, and each chip's tooltip names its new ⌘ number.
 * The picture is the strip after the drop.
 */
export const Reorder: Story = {
  play: async ({ canvas, userEvent }) => {
    const bar = canvas.getByTestId("title-bar");
    const names = () => chipsOf(bar).map((one) => one.dataset.chipOf);
    await expect(names()).toEqual(["smart-ide", "charter", "skills"]);
    const widths = () => chipsOf(bar).map((one) => Math.round(one.getBoundingClientRect().width));
    const before = widths();
    const [first, second] = chipsOf(bar);
    const from = first.getBoundingClientRect();
    const past = second.getBoundingClientRect();
    const y = from.top + from.height / 2;
    const at = (x: number) => ({
      clientX: x,
      clientY: y,
      bubbles: true,
      pointerId: 1,
      isPrimary: true,
    });
    // dnd-kit's pointer sensor reads the pointer as a person's arrives: a press, then moves past
    // its few pixels, then a release, each a real PointerEvent.
    first.dispatchEvent(
      new PointerEvent("pointerdown", { ...at(from.left + 8), button: 0, buttons: 1 }),
    );
    // Carried until its middle is over its neighbour's middle: the drop place is the neighbour's.
    const over = from.left + 8 + (past.left + past.width / 2 - (from.left + from.width / 2));
    for (const x of [from.left + 14, from.left + 40, over])
      document.dispatchEvent(new PointerEvent("pointermove", { ...at(x), buttons: 1 }));
    // Mid-drag: no chip has changed its size, and the neighbour stepped aside for the drop place.
    await waitFor(() =>
      expect(first.closest('[data-part="slot"]')).toHaveAttribute("data-dragging"),
    );
    await expect(widths()).toEqual(before);
    await waitFor(() =>
      expect(Math.round(second.getBoundingClientRect().left)).toBeLessThan(Math.round(past.left)),
    );
    document.dispatchEvent(new PointerEvent("pointerup", { ...at(over), buttons: 0 }));
    await waitFor(() => expect(names()).toEqual(["charter", "smart-ide", "skills"]));
    await expect(canvas.getByTestId("story-said")).toHaveTextContent(
      "Pins: charter, smart-ide, skills",
    );
    // The drop chose nothing: smart-ide is still the workspace in front.
    await expect(chipsOf(bar)[1]).toHaveAttribute("aria-current", "true");
    await expect(widths()).toEqual([before[1], before[0], before[2]]);
    // ⌘1–9 follow the order: charter is ⌘1 now.
    await userEvent.hover(chipsOf(bar)[0]);
    await waitFor(() =>
      expect(document.querySelector('[role="tooltip"][data-scope="tooltip"]')).toHaveTextContent(
        /charter · (⌘|Ctrl\+)1/,
      ),
    );
    await userEvent.unhover(chipsOf(bar)[0]);
    await waitFor(() =>
      expect(document.querySelector('[role="tooltip"][data-scope="tooltip"]')).toBeNull(),
    );
    // The keyboard walks the new order: the stop is still the current chip, now second.
    await walkTheStrip(userEvent, bar, "Reorder", -1);
    // The stop went with the arrow (#5912): Tab leaves the strip from the chip the keyboard is on,
    // one stop for all its chips (nothing follows the bar in this story, so the keyboard comes
    // round to the bar's start), and Shift+Tab comes back to that chip.
    const arrowedTo = chipsOf(bar)[0];
    await userEvent.tab();
    const strip = bar.querySelector('[role="toolbar"][aria-label="Pinned workspaces"]');
    await waitFor(() => expect(strip?.contains(document.activeElement)).toBe(false));
    await userEvent.tab({ shift: true });
    await untilFocused(() => arrowedTo, "Reorder: Shift+Tab comes back to the chip arrowed to");
    expectFocusInSight();
    // The picture is of the strip at rest, as before: out of it again, with no tooltip up.
    await userEvent.tab();
    await waitFor(() => expect(strip?.contains(document.activeElement)).toBe(false));
    await waitFor(() =>
      expect(document.querySelector('[role="tooltip"][data-scope="tooltip"]')).toBeNull(),
    );
  },
};

/**
 * **The keyboard into the strip and one step along it**: from nothing focused, Tab to the house
 * and Tab again to the strip's one stop (the current chip), then one arrow `towards` its
 * neighbour, each held in sight.
 */
async function walkTheStrip(
  userEvent: { tab: () => Promise<void>; keyboard: (keys: string) => Promise<void> },
  bar: HTMLElement,
  story: string,
  towards: 1 | -1 = 1,
) {
  (document.activeElement as HTMLElement | null)?.blur();
  const house = () => bar.querySelector('[data-part="workspace"][data-variant="root"]');
  const current = () => chipsOf(bar).find((one) => one.hasAttribute("aria-current")) ?? null;
  await userEvent.tab();
  await untilFocused(house, `${story}: Tab reaches the house`);
  await userEvent.tab();
  await untilFocused(current, `${story}: Tab reaches the strip's stop, the current chip`);
  expectFocusInSight();
  const chips = chipsOf(bar);
  const next = chips[chips.indexOf(current() as HTMLElement) + towards];
  await userEvent.keyboard(towards > 0 ? "{ArrowRight}" : "{ArrowLeft}");
  await untilFocused(() => next, `${story}: the arrow walks to the neighbour`);
  expectFocusInSight();
}

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
 * **The plane root's chip** (#3654, spec #3647, the prototype's Root A): a house right after the
 * project's name and its slash, outside the strip, 28px like a chip. A press brings the plane
 * root in front: it takes the accent's wash and no workspace's chip is current; a pin's press
 * takes it back. It is a Tab stop of its own, and its picture is the root in front.
 */
export const Root: Story = {
  play: async ({ canvas, userEvent }) => {
    const bar = canvas.getByTestId("title-bar");
    const root = canvas.getByRole("button", { name: "Plane root" });
    const strip = canvas.getByRole("toolbar", { name: "Pinned workspaces" });
    await expect(strip.contains(root)).toBe(false);
    const slash = bar.querySelector('[data-part="slash"]');
    if (slash === null) throw new Error("no slash");
    // After the slash, before the strip, in the bar's row.
    await expect(root.getBoundingClientRect().left).toBeGreaterThan(
      slash.getBoundingClientRect().right,
    );
    await expect(strip.getBoundingClientRect().left).toBeGreaterThanOrEqual(
      root.getBoundingClientRect().right,
    );
    await expect(Math.round(root.getBoundingClientRect().height)).toBe(28);
    const rest = getComputedStyle(root).backgroundColor;
    await userEvent.click(root);
    await waitFor(() => expect(root).toHaveAttribute("aria-current", "true"));
    await expect(canvas.getByTestId("story-said")).toHaveTextContent(`Went to ${OUTSIDE}`);
    await expect(chipsOf(bar).some((one) => one.hasAttribute("aria-current"))).toBe(false);
    await userEvent.hover(canvas.getByText(`Went to ${OUTSIDE}`));
    await waitFor(() => expect(root).not.toHaveAttribute("data-hovered"));
    await expect(getComputedStyle(root).backgroundColor).not.toBe(rest);
    // The keyboard reaches it on its own stop, the one before the strip's.
    chipsOf(bar)[0].focus();
    await userEvent.keyboard("{Shift>}{Tab}{/Shift}");
    await expect(root).toHaveFocus();
    await waitFor(() => expect(root).toHaveAttribute("data-focus-visible"));
    expectFocusInSight();
    // A pin's press takes the chosen look back.
    await userEvent.click(chipsOf(bar)[1]);
    await waitFor(() => expect(root).not.toHaveAttribute("aria-current"));
    // The picture: the plane root in front.
    await userEvent.click(root);
    await waitFor(() => expect(root).toHaveAttribute("aria-current", "true"));
    root.blur();
    await userEvent.hover(canvas.getByText(`Went to ${OUTSIDE}`));
  },
};

/**
 * **A name with descenders** (#3326): `git-ci-cd`, dashed while it is not pinned, beside the
 * pinned and the current chips. Every chip's name box holds the font's whole height, ascent to
 * descent, so the clip that draws its ellipsis never cuts a g, j, p, q or y. One rule draws the
 * name in every state, so the states the pointer gives are the screenshots'.
 */
export const Descenders: Story = {
  args: { focusedAt: "git-ci-cd" },
  play: async ({ canvas }) => {
    const chips = chipsOf(canvas.getByTestId("title-bar"));
    await expect(chips[chips.length - 1]).toHaveTextContent("git-ci-cd+");
    const cut = chips.flatMap((chip) => {
      const name = chip.querySelector<HTMLElement>('[data-part="name"]');
      if (name === null) return [`${chip.textContent ?? ""}: no name part`];
      const ink = penFor(name).measureText(name.textContent ?? "");
      const tall = ink.fontBoundingBoxAscent + ink.fontBoundingBoxDescent;
      const box = name.getBoundingClientRect().height;
      return box + 0.5 >= tall ? [] : [`${name.textContent ?? ""}: ${box}px of ${tall}px`];
    });
    await expect(cut).toEqual([]);
    for (const one of chips) await expect(Math.round(one.getBoundingClientRect().height)).toBe(28);
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
    await untilFocused(() => filter, "All workspaces: the keyboard starts in the filter");
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
 * **The workspace popover over 1,000 workspaces** (#5501): only the rows in sight are drawn, each
 * with its pin beside it at the same height, and each says its place in the whole; the arrows
 * walk past the first screenful with the chosen row kept in sight, and typing finds the last.
 */
export const ThousandWorkspaces: Story = {
  args: { many: true },
  play: async ({ canvas, userEvent }) => {
    await userEvent.click(canvas.getByRole("button", { name: "All workspaces" }));
    const filter = await screen.findByRole("combobox", {
      name: "Go to a workspace in charter…",
    });
    await untilFocused(() => filter, "All workspaces: the keyboard starts in the filter");
    const list = screen.getByRole("listbox", { name: "Workspaces in charter" });
    const drawn = within(list).getAllByRole("option");
    await expect(drawn.length).toBeLessThan(60);
    // The root, the three of the prototype, 1,000 more and New workspace…
    await expect(drawn[0]).toHaveAttribute("aria-setsize", "1005");
    for (let step = 0; step < 40; step++) await userEvent.keyboard("{ArrowDown}");
    const chosen = document.getElementById(filter.getAttribute("aria-activedescendant") ?? "");
    await expect(chosen).toHaveAttribute("aria-posinset", "42");
    // In sight inside the popover's scrolling list, and its pin beside it, row for row.
    const scroller = list.closest<HTMLElement>('[data-scope="popover"][data-part="list"]');
    const seen = scroller?.getBoundingClientRect();
    const row = chosen?.getBoundingClientRect();
    // Within a pixel: the virtualizer rounds the scroller's height as it brings a row in.
    await expect(row !== undefined && seen !== undefined).toBe(true);
    await expect(row?.top ?? 0).toBeGreaterThanOrEqual((seen?.top ?? 0) - 1);
    await expect(row?.bottom ?? 0).toBeLessThanOrEqual((seen?.bottom ?? 0) + 1);
    // Rows keep the gap the list draws between them, as a list drawn whole does.
    const next = chosen?.nextElementSibling?.getBoundingClientRect();
    if (next !== undefined && row !== undefined)
      await expect(next.top - row.bottom).toBeGreaterThan(0);
    const pin = screen.getByRole("button", {
      name: `Pin ${chosen?.querySelector("[data-titlebar-name]")?.textContent ?? ""} to the title bar`,
    });
    const centre = (box: DOMRect) => box.top + box.height / 2;
    await expect(
      Math.abs(centre(pin.getBoundingClientRect()) - centre(row as DOMRect)),
    ).toBeLessThan(1);
    expectFocusInSight();
    await userEvent.keyboard("ws-0999");
    await expect(
      document.getElementById(filter.getAttribute("aria-activedescendant") ?? ""),
    ).toHaveAccessibleName("ws-0999");
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
    await untilFocused(() => filter, "The crumb: the keyboard starts in the filter");
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
