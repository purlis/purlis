// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The words the window once faded, drawn for the faded-words gate** (#3790, #3637,
 * `.storybook/faded.setup.ts`). After every story, in Chromium, each word an `opacity` fades on
 * its box or any box above it is measured as drawn on its ground in the cell's theme, and fails
 * under 4.5:1 (3:1 for large text). These stories put in front of it the surfaces whose words
 * were faded: a pane's header line, with the quiet ways of its breadcrumb (the tier #3637 is
 * about), and the project trust ask, whose values were drawn at 0.6. Each is the window's own
 * component, so an opacity brought back in any stylesheet, on the words or on a box above them,
 * reaches the gate.
 *
 * No pictures: what they hold is a measurement, and the gate is the check. `GateSeesAFade` holds
 * the gate itself to seeing an opacity set by a rule on the header's chips and on the breadcrumb.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { expect, within } from "storybook/test";
import { ApprovePlane } from "./ApprovePlane";
import { ChatGauge } from "./ChatGauge";
import type { ListedChat } from "./chatsTree";
import { PaneHead } from "./LayoutPanes";
import { PaneCrumbs } from "./PaneCrumbs";
import { fadedWordsUnderFloor } from "./theme/fadedWords";
import { expectFocusInSight, untilFocused } from "./stories.testkit";

function listed(session: number, name: string, more: Partial<ListedChat> = {}): ListedChat {
  return {
    session,
    name,
    persona: "steward",
    workspace: "alpha",
    shell: false,
    parent: null,
    mode: "task",
    from: null,
    tab: false,
    branch: null,
    report: null,
    outcome: null,
    asking: null,
    harness: "Claude Code",
    ...more,
  };
}

/** A task two chats down, asked for from this tab, working in another workspace. */
const PATH = [
  listed(4, "steward 4", { mode: null, tab: true }),
  listed(9, "talk", { parent: 4 }),
  listed(12, "fix-login", { parent: 9, byYou: true }),
];

/** A pane's header line as a pane showing a task draws it: the breadcrumb, where the chat came
 *  from, its work item and its gauge, on the terminal's fill. */
function HeaderLine() {
  return (
    // A pane's size is its layout's (`container-type: size`), so the story gives it one.
    <div className="pane-frame pane-headed" style={{ inlineSize: 900, blockSize: 120 }}>
      <div className="pane-top">
        <PaneHead doing={null}>
          <div className="pane-chips">
            <PaneCrumbs crumbs={{ path: PATH, elsewhere: "beta" }} onShow={() => undefined} />
            <span className="pane-from">↳ from steward 3 · ops</span>
            <span className="pane-work-item">#3637</span>
            <ChatGauge
              usage={{
                context: { value: 42, tone: "ok" },
                cache: { value: 88, tone: "ok" },
                rebuilds: null,
                turns: [],
                cold: null,
              }}
            />
          </div>
        </PaneHead>
      </div>
    </div>
  );
}

const meta = {
  component: HeaderLine,
  parameters: { screenshot: false },
} satisfies Meta<typeof HeaderLine>;

export default meta;
type Story = StoryObj<typeof meta>;

/** **A pane's header line**: Tab reaches the breadcrumb (its whole path is its tooltip), then
 *  its first way, each with its ring in sight. */
export const APanesHeaderLine: Story = {
  play: async ({ canvas, userEvent }) => {
    const crumbs = canvas.getByRole("navigation", { name: "Chat path" });
    const [first] = within(crumbs).getAllByRole("button");
    await userEvent.tab();
    await untilFocused(() => crumbs, "the breadcrumb");
    expectFocusInSight();
    await userEvent.tab();
    await untilFocused(() => first, "the breadcrumb's first way");
    expectFocusInSight();
    (document.activeElement as HTMLElement | null)?.blur();
  },
};

const NOTHING = { plugins: [], env: [], starts: [], profiles: [], grants: [], fonts: [] };
const DIGEST = "ab12cd34ef56".concat("0".repeat(52));

/** **The project trust ask**: a plugin's how, a persona's grant and a font file's size are its
 *  values; the folder a font comes from is its `where`. Cancel has the keyboard as it opens. */
export const AProjectsTrustAsk: Story = {
  render: () => (
    <ApprovePlane
      ask={{
        path: "/work/alpha",
        contributes: {
          ...NOTHING,
          plugins: [["review-kit", "from the marketplace purlis-tools"]],
          grants: [["steward", "Bash(git status), Read"]],
          fonts: [["themes/plex/fonts/Plex-Regular.woff2", `${DIGEST} 40960`]],
        },
        changes: [],
        first: true,
      }}
      fonts={[{ theme: "plex", workspace: null }]}
      onApprove={() => undefined}
      onCancel={() => undefined}
    />
  ),
  play: async () => {
    const page = within(document.body);
    const dialog = await page.findByRole("dialog", { name: "Open this project?" });
    const cancel = within(dialog).getByRole("button", { name: "Cancel" });
    await untilFocused(() => cancel, "the trust ask's Cancel");
    expectFocusInSight();
  },
};

/**
 * **The gate sees a fade on the header's chips and on the breadcrumb** (#3637): an opacity of
 * 0.5 written into the rule that draws the header's chips, or into the breadcrumb's own, takes
 * the quiet ways under 4.5:1 in every built-in theme, and the gate names them. Put back before
 * the story ends, so the gate after it measures the line as the window draws it.
 */
export const GateSeesAFade: Story = {
  play: async ({ canvasElement }) => {
    const line = canvasElement.querySelector(".pane-head");
    if (line === null) throw new Error("the story drew no pane header");
    await expect(fadedWordsUnderFloor(line)).toEqual([]);
    for (const rule of [
      ".pane-head > .pane-chips > :is(.chat-gauge, .pane-from, .pane-work-item, .pane-crumbs)",
      ".pane-crumbs",
    ]) {
      const fade = document.createElement("style");
      fade.textContent = `${rule} { opacity: 0.5 !important; }`;
      document.head.append(fade);
      try {
        const under = fadedWordsUnderFloor(line).map(({ words }) => words);
        await expect(under, rule).toEqual(expect.arrayContaining(["steward 4", "talk"]));
      } finally {
        fade.remove();
      }
    }
  },
};

/**
 * **What the gate leaves alone, and what it does not** (#3790): words at 0.3 in a disabled
 * button, words hidden from assistive technology, words at opacity 0 and words whose opacity
 * moves for ever are not measured; words fading in to 0.3 are, where they come to rest, and so
 * are words at 0.3 in a box that only turns (an animation of something else) and plain words at
 * 0.3. Drawn in a box of their own and taken away before
 * the story ends, so the gate after it measures nothing of them.
 */
export const GateLeavesAlone: Story = {
  play: async () => {
    const box = document.createElement("div");
    box.style.background = "var(--surface-base)";
    box.style.color = "var(--text-quiet)";
    const faded = (words: string, tag = "p") => {
      const one = document.createElement(tag);
      one.textContent = words;
      one.style.opacity = "0.3";
      box.append(one);
      return one;
    };
    (faded("a disabled control", "button") as HTMLButtonElement).disabled = true;
    faded("hidden from assistive technology").setAttribute("aria-hidden", "true");
    faded("not drawn").style.opacity = "0";
    /** The window's own keyframes, timed by its motion tokens: no motion is written here. */
    const moved = (one: HTMLElement, name: string, iterations = "1") => {
      one.style.animationName = name;
      one.style.animationDuration = "var(--motion-duration-enter)";
      one.style.animationIterationCount = iterations;
    };
    moved(faded("fading for ever"), "charter-breathe", "infinite");
    moved(faded("arriving"), "charter-appear");
    moved(faded("turning"), "charter-spin", "infinite");
    faded("plain");
    document.body.append(box);
    try {
      const under = fadedWordsUnderFloor(box).map(({ words }) => words);
      await expect(under).toEqual(["arriving", "turning", "plain"]);
    } finally {
      box.remove();
    }
  },
};
