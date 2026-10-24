// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The chosen chat tab's pane: the band, the header, then the terminal** (#3142, the operator's variant A of
 * 2026-10-24): the session tabs docked on the content panel, the chosen tab joined to its pane,
 * and in the pane a sticky band of washed warning cards (words above their ways out), a slim
 * header with the chips at its start and the pane's controls at its end, then the terminal,
 * pinned to its bottom.
 *
 * The warnings are the window's own `Notice`s in its own `NoticePaneRow`, and the header is
 * `PaneHead`; the strip, the panel and the pane's frame wear the classes and parts the window
 * gives them, so what is drawn is what the window draws. The terminal is a stand-in of the
 * harness's last lines in the terminal's own colours: what is held here is everything around it.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { Fragment, useState, type ReactNode } from "react";
import { expect, waitFor, within } from "storybook/test";
import { Sparkles, X } from "lucide-react";
import { Notice, NoticePaneRow } from "../../Notice";
import { NOTICE_MOVED_JUST_NOW } from "../../noticeSettle";
import { PaneActions, PaneHead } from "../../LayoutPanes";
import type { Offer } from "../../actions";
import { ChatMark } from "../../NeedsYou";
import { useTabIds, StripTablist } from "../../StripTablist";
import { Button } from "../button/Button";
import { panelRoot } from "../panel/Panel";
import { TabCell, tabPart, tabsPart } from "./Tabs";
import { expectFocusInSight } from "../../stories.testkit";

/** One warning the band can hold, as `ChatNotices` draws it. */
type Warning =
  | "skills-missing"
  | "mcp-failed"
  | "stale-clone"
  | "vault-refused"
  | "sandbox-long"
  | "sandbox-ask";

/** The pane's own rows of the catalogue, as the window's catalogue offers them for a chat. */
const OFFERS: Record<string, Offer> = {
  "pane.split.right": {
    id: "pane.split.right",
    title: "Split right",
    available: true,
    reason: "",
    does: { verb: "split", direction: "row" },
  },
  "pane.split.down": {
    id: "pane.split.down",
    title: "Split down",
    available: true,
    reason: "",
    does: { verb: "split", direction: "column" },
  },
  "pane.close": {
    id: "pane.close",
    title: "End this pane's chat",
    available: true,
    reason: "",
    does: { verb: "closePane", ends: true },
  },
};

/** The Allows pressed in a story that did something: a press the guard held adds nothing. */
const allowed: string[] = [];

/** The harness's last lines, as a terminal draws them under the header. */
const LAST_LINES = [
  " ▐▛███▜▌   Claude Code v2.1.310",
  "▝▜█████▛▘  Opus 5.5 · Claude Max",
  "  ▘▘ ▝▝    ~/Projects/purlis-plane/workspaces/smart-ide",
  "",
  "> where should a pane's warning live so the tab stays joined?",
  "",
  "● The band sits inside the pane, under 8px of the terminal's fill.",
  "",
  "────────────────────────────────────────────────────────────",
  '❯ Try "how do I log an error?"',
  "────────────────────────────────────────────────────────────",
  "  ⏵⏵ auto mode on (shift+tab to cycle)",
].join("\n");

/** The warnings of one pane, each put away by its Dismiss as the window's are. */
function Warnings({ shown }: { shown: readonly Warning[] }) {
  const [away, setAway] = useState<readonly Warning[]>([]);
  const off = (one: Warning) => () => setAway((was) => [...was, one]);
  // Drawn in the order given: the band's order is the pane's.
  const drawn: Record<Warning, ReactNode> = {
    "vault-refused": (
      <Notice
        cause="vault-refused:2:forge"
        at="pane"
        tone="trouble"
        label="Vault"
        fixes={[
          { label: "Allow steward to use this vault", onPress: () => undefined, allows: true },
          { label: "Keep blocked", onPress: off("vault-refused") },
        ]}
      >
        This chat runs as steward, and vault <code>forge</code> is tagged for devops, so purlis did
        not open it.
      </Notice>
    ),
    "sandbox-ask": (
      <Notice
        cause="sandbox-blocked:2:network:connect:chat"
        at="pane"
        tone="trouble"
        label="Sandbox block"
        fixes={[
          {
            label: "Allow registry.npmjs.org",
            onPress: () => allowed.push("registry.npmjs.org"),
            allows: true,
          },
          { label: "Keep blocked", onPress: off("sandbox-ask") },
        ]}
      >
        The chat&apos;s command is waiting on <code>registry.npmjs.org:443</code>.
      </Notice>
    ),
    "sandbox-long": (
      <Notice
        cause="sandbox-blocked:2:npm"
        at="pane"
        tone="trouble"
        label="Sandbox block"
        fixes={[{ label: "Show what you can do", onPress: () => undefined }]}
        onDismiss={off("sandbox-long")}
      >
        The sandbox refused 4 connections while <code>npm ci --prefer-offline --no-audit</code> ran
        in{" "}
        <code>
          /Users/aharon/Projects/purlis-plane/workspaces/smart-ide/clones/purlis/app/node_modules/.cache/purlis-e2e-fixtures/
        </code>
        : <code>registry.npmjs.org:443</code>, <code>objects.githubusercontent.com:443</code>,{" "}
        <code>codeload.github.com:443</code> and <code>nodejs.org:443</code>.
      </Notice>
    ),
    "skills-missing": (
      <Notice
        cause="skills-missing:2"
        at="pane"
        label="Skills missing"
        fixes={[{ label: "Open in Skill studio", onPress: () => undefined }]}
        onDismiss={off("skills-missing")}
      >
        This chat started without 1 skill steward names, not installed on this machine:{" "}
        <code>mattpocock-skills:grilling</code>.
      </Notice>
    ),
    "mcp-failed": (
      <Notice
        cause="mcp-failed:2:figma"
        at="pane"
        tone="trouble"
        label="MCP server"
        fixes={[{ label: "Retry", onPress: () => undefined }]}
        onDismiss={off("mcp-failed")}
      >
        MCP server <code>figma</code> did not start for steward.
      </Notice>
    ),
    "stale-clone": (
      <Notice
        cause="stale-clone:2"
        at="pane"
        label="Stale clone"
        fixes={[{ label: "Update the clone", onPress: () => undefined }]}
        onDismiss={off("stale-clone")}
      >
        This chat&apos;s clone of <code>purlis/purlis</code> is 14 commits behind <code>main</code>.
      </Notice>
    ),
  };
  return (
    <>
      {shown
        .filter((one) => !away.includes(one))
        .map((one) => (
          <Fragment key={one}>{drawn[one]}</Fragment>
        ))}
    </>
  );
}

/** One pane as `PaneFrame` draws it: the band, the header, then the terminal. */
function Pane({
  name,
  warnings,
  focused,
}: {
  name: string;
  warnings: readonly Warning[];
  focused: boolean;
}) {
  return (
    <div
      className="pane-frame pane-headed"
      data-focused={focused || undefined}
      data-testid={`pane ${name}`}
    >
      <div className="pane-top">
        <NoticePaneRow>
          <Warnings shown={warnings} />
        </NoticePaneRow>
        <PaneHead
          doing={
            <PaneActions
              pane={0}
              offerFor={(id) => OFFERS[id]}
              closeOf={() => OFFERS["pane.close"]}
              onPaneDoes={() => undefined}
            />
          }
        >
          <div className="pane-chips">
            <Button
              size="sm"
              variant="ghost"
              className="pane-skills"
              icon={<Sparkles aria-hidden="true" />}
            >
              11 skills
            </Button>
            <button type="button" className="pane-harness">
              Claude Code
            </button>
            <span className="chat-gauge" data-testid="chat-gauge">
              <span className="gauge-label">ctx</span> 5% <span className="gauge-label">cache</span>{" "}
              100%
            </span>
          </div>
        </PaneHead>
      </div>
      <div className="pane-body">
        <div className={focused ? "pane focused" : "pane"}>
          <div className="xterm">
            <textarea className="xterm-helper-textarea" aria-label={`Terminal of ${name}`} />
            <pre
              style={{
                margin: 0,
                color: "var(--terminal-foreground)",
                fontFamily: "var(--font-mono)",
                fontSize: "var(--font-size-sm)",
                lineHeight: "var(--line-height-body)",
                whiteSpace: "pre-wrap",
                overflowWrap: "anywhere",
              }}
            >
              {LAST_LINES}
            </pre>
          </div>
        </div>
      </div>
    </div>
  );
}

const TABS = ["steward 11", "steward 2"];

/** **The strip, docked on the content panel, and the chosen chat's pane under it.** */
function PaneBand({
  warnings,
  width = 760,
  split = false,
}: {
  warnings: readonly Warning[];
  width?: number;
  /** The chosen tab split in two, the warnings in its first pane, the keyboard in its second. */
  split?: boolean;
}) {
  const idOf = useTabIds();
  const chosen = 1;
  const panes: { name: string; warnings: readonly Warning[]; focused: boolean }[] = split
    ? [
        { name: "steward 2", warnings, focused: false },
        { name: "steward 11", warnings: [], focused: true },
      ]
    : [{ name: "steward 2", warnings, focused: true }];
  return (
    <div className="editor" {...tabsPart("layout")} style={{ inlineSize: width, blockSize: 460 }}>
      <header className="bar" {...tabsPart("strip")}>
        <div className="tabs" data-strip="Chats">
          <StripTablist name="Chats" ids={TABS.map((_, at) => idOf(at))} />
          {TABS.map((name, at) => (
            <TabCell
              key={name}
              className="tab"
              variant="chat"
              selected={at === chosen}
              first={at === 0}
            >
              <button
                type="button"
                role="tab"
                id={idOf(at)}
                aria-selected={at === chosen}
                tabIndex={at === chosen ? 0 : -1}
              >
                <ChatMark state="waiting" />
                <span className="tab-name" {...tabPart("label")}>
                  {name}
                </span>
              </button>
              <button
                type="button"
                className="closer"
                {...tabPart("closer")}
                tabIndex={-1}
                aria-label={`Close ${name}`}
              >
                <X aria-hidden="true" />
              </button>
            </TabCell>
          ))}
        </div>
      </header>
      <div
        className="panes"
        {...panelRoot("content")}
        role="tabpanel"
        aria-labelledby={idOf(chosen)}
      >
        <div data-group="" style={{ display: "flex", blockSize: "100%" }}>
          {panes.map((pane, at) => (
            <Fragment key={pane.name}>
              {at > 0 && <div data-separator="" aria-hidden="true" />}
              <div style={{ flex: "1 1 0", minInlineSize: 0, blockSize: "100%" }}>
                <Pane {...pane} />
              </div>
            </Fragment>
          ))}
        </div>
      </div>
    </div>
  );
}

const meta = {
  component: PaneBand,
} satisfies Meta<typeof PaneBand>;

export default meta;
type Story = StoryObj<typeof meta>;

type Box = DOMRect;
const boxOf = (el: Element | null | undefined): Box => (el as Element).getBoundingClientRect();

/** The band, the header and the terminal of the pane named `name`, top to bottom. */
function rowsOf(canvas: HTMLElement, name = "steward 2") {
  const frame = canvas.querySelector(`[data-testid="pane ${name}"]`) as HTMLElement;
  return {
    frame,
    band: frame.querySelector(".pane-notice-row"),
    head: frame.querySelector(".pane-head"),
    terminal: frame.querySelector(".pane"),
    warnings: [...frame.querySelectorAll(".pane-notices > :not([hidden])")],
  };
}

/** Rules 1 to 3: the band and the header inside the pane, one above the other, the terminal
 *  under both, and the chosen tab joined to the pane's top where the tabs dock. */
async function heldInThePane(canvas: HTMLElement, name = "steward 2") {
  const { frame, band, head, terminal, warnings } = rowsOf(canvas, name);
  const pane = boxOf(frame);
  for (const warning of warnings) {
    const at = boxOf(warning);
    expect(at.top).toBeGreaterThanOrEqual(pane.top + 6);
    expect(at.right).toBeLessThanOrEqual(pane.right + 1);
    // As far as the band shows it: past the band's room the warnings scroll inside it.
    const stack = boxOf(warning.parentElement);
    expect(Math.min(at.bottom, stack.bottom)).toBeLessThanOrEqual(boxOf(head).top + 1);
  }
  if (warnings.length > 0) expect(boxOf(band).bottom).toBeLessThanOrEqual(boxOf(head).top + 1);
  expect(boxOf(head).bottom).toBeLessThanOrEqual(boxOf(terminal).top + 1);
  const layout = canvas.querySelector('[data-scope="tabs"][data-part="layout"]') as Element;
  if (getComputedStyle(layout).getPropertyValue("--tabs-joined").trim() === "1") {
    const tab = canvas.querySelector('[data-part="tab"][data-selected]') as Element;
    expect(boxOf(tab).bottom).toBeCloseTo(pane.top, 0);
    expect(getComputedStyle(tab).backgroundColor).toBe(getComputedStyle(frame).backgroundColor);
  }
}

/** Puts the keyboard down, so the picture shows the pane at rest. */
const atRest = () => (document.activeElement as HTMLElement | null)?.blur();

/**
 * **One warning**: the operator's case, the skills a chat started without. The tab stays joined
 * to the pane; the warning is a wash inside it, its words above its ways out; the header holds
 * the chips and the controls; the terminal is under them. The keyboard reaches the warning's
 * ways out with its ring in sight.
 */
export const AWarning: Story = {
  args: { warnings: ["skills-missing"] },
  play: async ({ canvas, canvasElement }) => {
    const open = await canvas.findByRole("button", { name: "Open in Skill studio" });
    await heldInThePane(canvasElement);
    const says = within(canvas.getByRole("status", { name: "Skills missing" }));
    // The words above their ways out (variant A).
    const words = boxOf(canvas.getByRole("status", { name: "Skills missing" }).firstElementChild);
    expect(boxOf(open).top).toBeGreaterThanOrEqual(words.bottom - 1);
    expect(says.getByRole("button", { name: "Dismiss" })).toBeVisible();
    open.focus();
    expectFocusInSight();
    atRest();
  },
};

/**
 * **A consent, trouble, and one more behind "+1 more"**: the vault refusal has no Dismiss, its
 * Allow stands apart from Keep blocked and neither is the primary look, and Esc on it answers
 * nothing; "+1 more" opens the rest and says Show fewer, in the same width.
 */
export const ConsentAndMore: Story = {
  args: { warnings: ["vault-refused", "mcp-failed", "stale-clone"] },
  play: async ({ canvas, canvasElement, userEvent }) => {
    const vault = await canvas.findByRole("status", { name: "Vault" });
    await heldInThePane(canvasElement);
    expect(within(vault).queryByRole("button", { name: "Dismiss" })).toBeNull();
    const allow = within(vault).getByRole("button", { name: "Allow steward to use this vault" });
    const keep = within(vault).getByRole("button", { name: "Keep blocked" });
    expect(boxOf(keep).left - boxOf(allow).right).toBeGreaterThan(12);

    keep.focus();
    await userEvent.keyboard("{Escape}");
    expect(canvas.getByRole("status", { name: "Vault" })).toBeVisible();
    expect(keep).toHaveFocus();

    const more = canvas.getByRole("button", { name: "+1 more" });
    const width = boxOf(more).width;
    await userEvent.click(more);
    await waitFor(() => expect(more).toHaveAccessibleName("Show fewer"));
    expect(boxOf(more).width).toBeCloseTo(width, 0);
    await userEvent.click(more);
    await waitFor(() => expect(more).toHaveAccessibleName("+1 more"));
    atRest();
  },
};

/**
 * **Esc puts each warning away, and the keyboard goes on**: to the next warning, then to the
 * terminal. The pane is the header and the terminal again, the terminal grown from the top.
 */
export const PutAway: Story = {
  args: { warnings: ["skills-missing", "stale-clone"] },
  play: async ({ canvas, userEvent }) => {
    const first = await canvas.findByRole("button", { name: "Open in Skill studio" });
    first.focus();
    await userEvent.keyboard("{Escape}");
    const next = await canvas.findByRole("button", { name: "Update the clone" });
    await waitFor(() => expect(next).toHaveFocus());
    await userEvent.keyboard("{Escape}");
    await waitFor(() => expect(canvas.getByLabelText("Terminal of steward 2")).toHaveFocus());
    expect(canvas.queryAllByRole("status")).toHaveLength(0);
    atRest();
  },
};

/**
 * **A consent that moved holds its Allow** (#1693, #1695): the warning above it is put away, the
 * consent moves up under the pointer, and a press on its Allow at once does nothing and says
 * why. The keyboard is landed on the consent's line, never on its Allow. Measured in the real
 * engine, where the warning's box is positioned for its mark: the guard reads where the line
 * is drawn against the band, not its `offsetTop`.
 */
export const ConsentMoved: Story = {
  args: { warnings: ["skills-missing", "sandbox-ask"] },
  play: async ({ canvas, userEvent }) => {
    allowed.length = 0;
    const open = await canvas.findByRole("button", { name: "Open in Skill studio" });
    open.focus();
    await userEvent.keyboard("{Escape}");
    const ask = await canvas.findByRole("status", { name: "Sandbox block" });
    await waitFor(() => expect(ask).toHaveFocus());

    await userEvent.click(within(ask).getByRole("button", { name: "Allow registry.npmjs.org" }));
    expect(allowed).toEqual([]);
    expect(await canvas.findByText(NOTICE_MOVED_JUST_NOW)).toBeVisible();
    atRest();
  },
};

/**
 * **The pane's actions** (`PaneActions`, shared with a view's pane, #3147): split side by side,
 * split stacked and close, one cluster at the pane's top right, drawn by the catalogue's own
 * rows. Hidden at rest in the pane with the keyboard, drawn once the keyboard is on them.
 */
export const Actions: Story = {
  args: { warnings: [] },
  play: async ({ canvas, canvasElement, userEvent }) => {
    const split = canvas.getByRole("button", { name: "Split right" });
    split.focus();
    await waitFor(() => expect(split).toBeVisible());
    await userEvent.tab();
    expect(canvas.getByRole("button", { name: "Split down" })).toHaveFocus();
    await userEvent.tab();
    expect(canvas.getByRole("button", { name: "End this pane's chat" })).toHaveFocus();
    expectFocusInSight();
    // At the pane's top right, in the header's row.
    const { frame, head } = rowsOf(canvasElement);
    const actions = boxOf(head?.querySelector(".pane-actions"));
    expect(boxOf(frame).right - actions.right).toBeLessThanOrEqual(16);
    expect(actions.top - boxOf(frame).top).toBeLessThanOrEqual(16);
  },
};

/**
 * **A split pane**: the warning is its own pane's, in that pane's band; the pane beside it has
 * its header at its top, and neither draws over the other.
 */
export const Split: Story = {
  args: { warnings: ["skills-missing"], split: true, width: 960 },
  play: async ({ canvas, canvasElement }) => {
    await canvas.findByRole("button", { name: "Open in Skill studio" });
    await heldInThePane(canvasElement, "steward 2");
    const other = rowsOf(canvasElement, "steward 11");
    expect(other.warnings).toHaveLength(0);
    expect(boxOf(other.head).top - boxOf(other.frame).top).toBeLessThanOrEqual(12);
    const left = boxOf(rowsOf(canvasElement, "steward 2").frame);
    expect(left.right).toBeLessThanOrEqual(boxOf(other.frame).left + 1);
  },
};

/**
 * **A narrow pane and a long warning**: it wraps in place and breaks its path anywhere, its ways
 * out under its words, and nothing runs out of the pane.
 */
export const Narrow: Story = {
  args: { warnings: ["sandbox-long"], width: 400 },
  play: async ({ canvas, canvasElement }) => {
    const block = await canvas.findByRole("status", { name: "Sandbox block" });
    await heldInThePane(canvasElement);
    const pane = boxOf(rowsOf(canvasElement).frame);
    const words = boxOf(block.firstElementChild);
    const stack = block.parentElement?.parentElement as HTMLElement;
    for (const way of within(block).getAllByRole("button")) {
      expect(boxOf(way).top).toBeGreaterThanOrEqual(words.bottom - 1);
      expect(boxOf(way).right).toBeLessThanOrEqual(pane.right + 1);
      // Whole, once the band is scrolled to it: never cut by its own warning (rule 6).
      way.scrollIntoView({ block: "nearest" });
      expect(boxOf(way).bottom).toBeLessThanOrEqual(boxOf(stack).bottom + 1);
      expect(boxOf(way).top).toBeGreaterThanOrEqual(boxOf(stack).top - 1);
    }
    stack.scrollTop = 0;
    expect(boxOf(block.parentElement).right).toBeLessThanOrEqual(pane.right + 1);
  },
};
