// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The chosen chat tab's pane: the header, the band, then the terminal** (#3142, the operator's
 * variant A of 2026-10-24; #4754): the session tabs docked on the content panel, the chosen tab
 * joined to its pane, and in the pane a slim header with the chips at its start and the pane's
 * controls at its end, the bell among them, then the band of compact rows (#5215), folded to one
 * line or open under its head, then the terminal, pinned to its bottom.
 *
 * The warnings are the window's own `Notice`s in its own `NoticePaneRow`, and the header is
 * `PaneHead`; the strip, the panel and the pane's frame wear the classes and parts the window
 * gives them, so what is drawn is what the window draws. The terminal is a stand-in of the
 * harness's last lines in the terminal's own colours: what is held here is everything around it.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { Fragment, useRef, useState, type ReactNode } from "react";
import { expect, waitFor, within } from "storybook/test";
import type { Shown } from "../../bindings";
import { Notice, NoticePaneRow } from "../../Notice";
import { TerminalPromptNotice } from "../../TerminalPromptNotice";
import { NOTICE_MOVED_JUST_NOW, SETTLE_MS, paneBand } from "../../noticeSettle";
import { PaneActions, PaneBellAt, PaneBellOf, PaneHead } from "../../LayoutPanes";
import { HOLDS_NOTHING, type BandHolds, type PaneNoticesShown } from "../../paneNotices";
import { PANE_NOTICES } from "../../paneNoticesPref";
import type { Offer } from "../../actions";
import { ChatMark } from "../chat-mark/ChatMark";
import { useTabIds } from "./TabStrip";
import { panelRoot } from "../panel/Panel";
import { TabCell, TabCloser, tabPart, tabsPart } from "./Tabs";
import { expectFocusInSight, untilFocused } from "../../stories.testkit";
import { Button } from "../button/Button";

/** One warning the band can hold, as `ChatNotices` draws it. */
type Warning =
  | "skills-missing"
  | "mcp-failed"
  | "stale-clone"
  | "vault-refused"
  | "sandbox-long"
  | "sandbox-ask"
  | "terminal-prompt";

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
    title: "Close this pane",
    available: true,
    reason: "",
    does: { verb: "closePane" },
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

/** The chat on screen stopped on a prompt in its own terminal. */
const PROMPT: Shown = {
  session: 2,
  ask: "terminal:2:1",
  says: "Waiting in its terminal: a permission prompt",
  options: [],
  source: "terminal",
  chain: ["steward 2"],
  answer: { via: "in-its-pane" },
  since: null,
};

/** The warnings of one pane, each put away by its Dismiss as the window's are. */
function Warnings({ shown }: { shown: readonly Warning[] }) {
  const [away, setAway] = useState<readonly Warning[]>([]);
  const off = (one: Warning) => () => setAway((was) => [...was, one]);
  // Drawn in the order given: the band's order is the pane's.
  const drawn: Record<Warning, ReactNode> = {
    "terminal-prompt": <TerminalPromptNotice session={2} waiting={[PROMPT]} />,
    "vault-refused": (
      <Notice
        cause="vault-refused:2:forge"
        at="pane"
        tone="trouble"
        label="Vault"
        fixes={[
          {
            label: "Allow steward to use this vault",
            onPress: () => allowed.push("forge"),
            allows: true,
          },
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

/** One pane as `PaneFrame` draws it: the header with its bell, the band, then the terminal. */
function Pane({
  name,
  warnings,
  focused,
  band,
  round = 0,
}: {
  name: string;
  warnings: readonly Warning[];
  focused: boolean;
  /** Each new round raises the pane's warnings again, every one put away before standing anew. */
  round?: number;
  /** The pane the band is (`paneBand`), so it reads on from where it was when drawn anew. */
  band?: string;
}) {
  const [holds, setHolds] = useState<BandHolds>(HOLDS_NOTHING);
  const bellAt = useRef<HTMLButtonElement>(null);
  const minimised = PANE_NOTICES.use() === "minimised";
  return (
    <div
      className="pane-frame pane-headed"
      data-focused={focused || undefined}
      data-testid={`pane ${name}`}
    >
      <div className="pane-top">
        <PaneHead
          doing={
            <PaneBellAt.Provider value={bellAt}>
              <PaneBellOf.Provider value={holds}>
                <PaneActions
                  pane={0}
                  offerFor={(id) => OFFERS[id]}
                  closeOf={() => OFFERS["pane.close"]}
                  onPaneDoes={() => undefined}
                />
              </PaneBellOf.Provider>
            </PaneBellAt.Provider>
          }
        >
          <div className="pane-chips">
            <span className="pane-name ui-text-heading">work</span>
          </div>
        </PaneHead>
        <NoticePaneRow
          band={band}
          bell={{
            minimised,
            minimise: () => PANE_NOTICES.set("minimised"),
            expand: () => PANE_NOTICES.set("expanded"),
            onHolds: setHolds,
            focus: () => bellAt.current?.focus(),
          }}
        >
          <Warnings key={round} shown={warnings} />
        </NoticePaneRow>
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
  height = 460,
  split = false,
  round = 0,
}: {
  warnings: readonly Warning[];
  /** The warnings raised again this many times (`Pane`'s `round`). */
  round?: number;
  /** Minimised or expanded, for every pane (#4754): set before the story is drawn. */
  shown?: PaneNoticesShown;
  width?: number;
  height?: number;
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
    <div
      className="editor"
      {...tabsPart("layout")}
      style={{ inlineSize: width, blockSize: height }}
    >
      <header className="bar" {...tabsPart("strip")}>
        <div className="tabs" data-strip="Chats">
          {/* The strip's tablist as `TabStripRow` draws it: empty, owning the tabs by id. */}
          <span
            className="strip-tablist"
            role="tablist"
            aria-label="Chats"
            aria-orientation="horizontal"
            aria-owns={TABS.map((_, at) => idOf(at)).join(" ")}
          />
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
              <TabCloser label={`Close ${name}`} onPress={() => {}} />
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
                <Pane {...pane} round={round} />
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
  // Expanded unless a story says otherwise, so each warning's own story reads it; the window's
  // default, minimised, has its own stories below. Set without telling the layout file.
  args: { shown: "expanded" },
  beforeEach: ({ args }) => {
    PANE_NOTICES.keep(args.shown ?? "expanded");
    return () => PANE_NOTICES.forget();
  },
} satisfies Meta<typeof PaneBand>;

export default meta;
type Story = StoryObj<typeof meta>;

type Box = DOMRect;
const boxOf = (el: Element | null | undefined): Box => (el as Element).getBoundingClientRect();

/** The header, the band and the terminal of the pane named `name`, top to bottom. */
function rowsOf(canvas: HTMLElement, name = "steward 2") {
  const frame = canvas.querySelector(`[data-testid="pane ${name}"]`) as HTMLElement;
  return {
    frame,
    band: frame.querySelector('[data-scope="notice-row"][data-part="root"]'),
    head: frame.querySelector(".pane-head"),
    terminal: frame.querySelector(".pane"),
    warnings: [
      ...frame.querySelectorAll('[data-scope="notice-row"][data-part="stack"] > :not([hidden])'),
    ],
  };
}

/** Rules 1 to 3, and #4754: the header and the band inside the pane, the band under the header,
 *  the terminal under both, and the chosen tab joined to the pane's top where the tabs dock. */
async function heldInThePane(canvas: HTMLElement, name = "steward 2") {
  const { frame, band, head, terminal, warnings } = rowsOf(canvas, name);
  const pane = boxOf(frame);
  expect(boxOf(head).top).toBeGreaterThanOrEqual(pane.top + 2);
  for (const warning of warnings) {
    const at = boxOf(warning);
    expect(at.top).toBeGreaterThanOrEqual(boxOf(head).bottom - 1);
    expect(at.right).toBeLessThanOrEqual(pane.right + 1);
    // As far as the band shows it: past the band's room the warnings scroll inside it.
    const stack = boxOf(warning.parentElement);
    expect(Math.min(at.bottom, stack.bottom)).toBeLessThanOrEqual(boxOf(terminal).top + 1);
  }
  if (warnings.length > 0) {
    expect(boxOf(band).top).toBeGreaterThanOrEqual(boxOf(head).bottom - 1);
    expect(boxOf(band).bottom).toBeLessThanOrEqual(boxOf(terminal).top + 1);
  }
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
 * to the pane; the warning is one row inside it, its ways out beside its words; the header holds
 * the chips and the controls; the terminal is under them. The keyboard reaches the warning's
 * ways out with its ring in sight.
 */
export const AWarning: Story = {
  args: { warnings: ["skills-missing"] },
  play: async ({ canvas, canvasElement }) => {
    const open = await canvas.findByRole("button", { name: "Open in Skill studio" });
    await heldInThePane(canvasElement);
    const says = within(canvas.getByRole("status", { name: "Skills missing" }));
    // One row: its ways out beside its words (#5215).
    const words = boxOf(
      canvas.getByRole("status", { name: "Skills missing" }).querySelector('[data-part="words"]'),
    );
    expect(boxOf(open).left).toBeGreaterThanOrEqual(words.right - 1);
    expect(says.getByRole("button", { name: "Dismiss" })).toBeVisible();
    open.focus();
    expectFocusInSight();
    atRest();
  },
};

/**
 * **A consent among notes**: the vault refusal has no Dismiss, its Allow stands apart from Keep
 * blocked and neither is the primary look, and Esc on it answers nothing. Every notice stands
 * expanded, with no cap and no "+N more".
 */
export const ConsentAndNotes: Story = {
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

    expect(rowsOf(canvasElement).warnings).toHaveLength(3);
    expect(canvas.queryByRole("button", { name: /more/ })).toBeNull();
    atRest();
  },
};

/** The pane's bell, by its name. */
const bellOf = (canvas: ReturnType<typeof within>) =>
  canvas.getByRole("button", { name: /^Pane notices:/ });

/** The bell's count: a pill on its corner, beside the button, said by the bell's own name. */
const countOf = (bell: HTMLElement) =>
  bell.parentElement?.querySelector('[data-scope="count"]') ?? null;

/**
 * **Folded, the window's default** (#4754, #5215): the notes fold into the bell's count, in the
 * trouble colour while one is trouble, and the band is one line naming how many wait and on what.
 * The bell holds its slot before split and close. Show opens it, and the keyboard goes to the
 * fold; the fold closes it, and the keyboard goes back to the line.
 */
export const Minimised: Story = {
  args: {
    shown: "minimised",
    warnings: ["vault-refused", "mcp-failed", "stale-clone", "skills-missing"],
  },
  play: async ({ canvas, canvasElement, userEvent }) => {
    const line = await canvas.findByRole("button", { name: /^1 waiting · Vault\s*·\s*Show$/ });
    await waitFor(() => expect(rowsOf(canvasElement).warnings).toHaveLength(0));
    await heldInThePane(canvasElement);
    expect(canvas.getByRole("region", { name: /^Pane notices/ })).toHaveAccessibleName(
      "Pane notices: 1 waiting on you · 3 notices",
    );
    const bell = bellOf(canvas);
    expect(bell).toHaveAccessibleName("Pane notices: 3 notices, 1 in trouble, 1 asking");
    expect(countOf(bell)).toHaveTextContent(/^3$/);
    expect(bell).toHaveAttribute("aria-expanded", "false");
    // Before split and close, in the header's row.
    const split = canvas.getByRole("button", { name: "Split right" });
    expect(boxOf(bell).right).toBeLessThanOrEqual(boxOf(split).left + 1);
    expect(boxOf(bell).top).toBeCloseTo(boxOf(split).top, 0);

    line.focus();
    await userEvent.keyboard("{Enter}");
    const fold = await canvas.findByRole("button", { name: "Fold the band" });
    await untilFocused(() => fold, "Show gives the keyboard to the fold");
    expectFocusInSight();
    await userEvent.keyboard("{Enter}");
    await untilFocused(
      () => canvas.getByRole("button", { name: /^1 waiting · Vault\s*·\s*Show$/ }),
      "the fold gives the keyboard back to the line",
    );
    expectFocusInSight();
    atRest();
  },
};

/**
 * **Each kind keeps its own actions on its row** (#5215): a prompt in the terminal with Go to prompt,
 * a consent with its full waiting edge and its Allow apart from Keep blocked, and a note with its
 * ×. The keyboard takes each row's buttons in order, and the × last.
 */
export const EachKindOfRow: Story = {
  args: { warnings: ["terminal-prompt", "sandbox-ask", "mcp-failed"] },
  play: async ({ canvas, userEvent }) => {
    const prompt = await canvas.findByRole("status", { name: "Terminal prompt" });
    const go = within(prompt).getByRole("button", { name: "Go to prompt" });
    go.focus();
    await untilFocused(() => go, "on Go to prompt");
    await userEvent.tab();
    await untilFocused(
      () => within(prompt).getByRole("button", { name: "Close" }),
      "on the prompt's ×",
    );
    expectFocusInSight();
    const block = canvas.getByRole("status", { name: "Sandbox block" })
      .parentElement as HTMLElement;
    expect(getComputedStyle(block).borderTopWidth).toBe("1px");
    expect(getComputedStyle(block).borderLeftWidth).toBe("1px");
    atRest();
  },
};

/**
 * **A long message opened** (#5215): one row until its toggle is pressed; opened, it shows whole, the
 * band grows, the terminal keeps its three lines, and past that the band scrolls as one, with the
 * ways out pinned at the message's foot, above the band's fade. Esc folds it, and the keyboard lands
 * on the toggle.
 */
export const ALongMessageOpened: Story = {
  args: { warnings: ["sandbox-long", "vault-refused"], width: 420 },
  play: async ({ canvas, canvasElement, userEvent }) => {
    const row = await canvas.findByRole("status", { name: "Sandbox block" });
    const toggle = await within(row).findByRole("button", { name: "Show the whole message" });
    toggle.focus();
    await userEvent.keyboard("{Enter}");
    const box = row.parentElement as HTMLElement;
    await waitFor(() => expect(box).toHaveAttribute("data-whole"));
    // Whole, in no box of its own, and its toggle in sight at its foot.
    const message = row.querySelector('[data-part="message"]') as HTMLElement;
    expect(message.scrollHeight).toBeLessThanOrEqual(message.clientHeight + 1);
    expectFocusInSight();
    // The terminal keeps at least its three lines.
    const { terminal } = rowsOf(canvasElement);
    const size = parseFloat(getComputedStyle(terminal as Element).fontSize);
    expect(boxOf(terminal).height).toBeGreaterThanOrEqual(3 * size);
    // A tooltip that is up takes the first Esc (`ui/tooltip`), and the row the next.
    await userEvent.keyboard("{Escape}");
    if (box.hasAttribute("data-whole")) await userEvent.keyboard("{Escape}");
    await waitFor(() => expect(box).not.toHaveAttribute("data-whole"));
    await untilFocused(
      () => within(row).getByRole("button", { name: "Show the whole message" }),
      "Esc lands on the toggle",
    );
    // Opened again for the picture, the keyboard put down, so no ring or tip is in it.
    await userEvent.keyboard("{Enter}");
    await waitFor(() => expect(box).toHaveAttribute("data-whole"));
    atRest();
  },
};

/**
 * **Opened in a short pane, the row is as tall as what it says** (#5215): the band scrolls as one,
 * so nothing caps a row, and what it says never runs over the row after it.
 */
export const ALongMessageOpenedInAShortPane: Story = {
  args: { warnings: ["sandbox-long", "vault-refused"], width: 420, height: 260 },
  play: async ({ canvas, canvasElement, userEvent }) => {
    const row = await canvas.findByRole("status", { name: "Sandbox block" });
    const toggle = await within(row).findByRole("button", { name: "Show the whole message" });
    toggle.focus();
    await userEvent.keyboard("{Enter}");
    const box = row.parentElement as HTMLElement;
    await waitFor(() => expect(box).toHaveAttribute("data-whole"));
    expect(boxOf(row).bottom).toBeLessThanOrEqual(boxOf(box).bottom + 1);
    const { terminal, warnings } = rowsOf(canvasElement);
    expect(boxOf(warnings[1]).top).toBeGreaterThanOrEqual(boxOf(row).bottom - 1);
    // Scrolling as one: the stack scrolls, and the message has no box of its own.
    const stack = box.parentElement as HTMLElement;
    expect(stack.scrollHeight).toBeGreaterThan(stack.clientHeight);
    const message = row.querySelector('[data-part="message"]') as HTMLElement;
    expect(message.scrollHeight).toBeLessThanOrEqual(message.clientHeight + 1);
    const size = parseFloat(getComputedStyle(terminal as Element).fontSize);
    expect(boxOf(terminal).height).toBeGreaterThanOrEqual(3 * size);
    atRest();
  },
};

/** How far the bell's pill may rise above the bell, in px: its one step (`--space-1`, 2px at
 *  the default text size), and a pixel for rounding. */
const RISES_AT_MOST = 3;

/** Whether two boxes share any of their area. */
const overlaps = (a: Box, b: Box) =>
  a.left < b.right && a.right > b.left && a.top < b.bottom && a.bottom > b.top;

/**
 * **The bell beside its neighbours** (#5189): the size of Split right, its count the family's
 * small pill on its top corner, which leaves the bell to be seen; its hint opens below the
 * head, as the head's other hints do, over no head button; and the head's buttons stand a small
 * gap apart, so a hovered or focused one's ground never touches the next (the person's ruling
 * on the M70 prototype, spec #5191).
 */
export const BellBesideItsNeighbours: Story = {
  args: {
    shown: "minimised",
    warnings: ["vault-refused", "mcp-failed", "stale-clone", "skills-missing"],
  },
  // No picture: it ends with the bell's hint open, which comes after a delay and a fade; the
  // head as it rests is pictured in Minimised.
  parameters: { screenshot: false },
  play: async ({ canvas, canvasElement, userEvent }) => {
    const bell = bellOf(canvas);
    await waitFor(() => expect(countOf(bell)).toHaveTextContent(/^3$/));
    const split = canvas.getByRole("button", { name: "Split right" });
    expect(boxOf(bell).width).toBeCloseTo(boxOf(split).width, 0);
    expect(boxOf(bell).height).toBeCloseTo(boxOf(split).height, 0);
    // The pill is the family's small count, on the bell's top corner: it rises a little above
    // the bell, never past its end towards Split right, and leaves the bell's mark to be seen,
    // covering no more than a quarter of it.
    const count = boxOf(countOf(bell));
    expect(countOf(bell)).toHaveAttribute("data-size", "sm");
    expect(count.right).toBeLessThanOrEqual(boxOf(bell).right + 0.5);
    expect(count.top).toBeGreaterThanOrEqual(boxOf(bell).top - RISES_AT_MOST);
    expect(count.bottom).toBeLessThanOrEqual(boxOf(bell).top + boxOf(bell).height / 2);
    expect(overlaps(count, boxOf(split))).toBe(false);
    const mark = boxOf(bell.querySelector("svg"));
    const covered =
      Math.max(0, Math.min(count.right, mark.right) - Math.max(count.left, mark.left)) *
      Math.max(0, Math.min(count.bottom, mark.bottom) - Math.max(count.top, mark.top));
    expect(covered, "the bell's mark the pill covers").toBeLessThanOrEqual(
      (mark.width * mark.height) / 4,
    );

    const { head } = rowsOf(canvasElement);
    const actions = head?.querySelector(".pane-actions") as HTMLElement;
    const buttons = within(actions).getAllByRole("button");
    const boxes = buttons.map(boxOf).sort((a, b) => a.left - b.left);
    expect(boxes.length).toBeGreaterThanOrEqual(4);
    for (let i = 1; i < boxes.length; i++) {
      expect(boxes[i].left - boxes[i - 1].right, `gap before head button ${i}`).toBeGreaterThan(1);
    }

    // From the control after it, back: nothing on the head's line is pressed before the bell now
    // that the chat's facts are the foot's (#5202, #5204).
    const all = canvas.getAllByRole("button");
    all[all.indexOf(bell) + 1].focus();
    await userEvent.tab({ shift: true });
    await untilFocused(() => bell, "Tab onto the bell");
    const tip = await within(document.body).findByRole("tooltip");
    await waitFor(() => {
      const at = boxOf(tip);
      expect(at.top).toBeGreaterThanOrEqual(boxOf(bell).bottom);
      for (const one of buttons) {
        expect(overlaps(at, boxOf(one)), one.getAttribute("aria-label") ?? "").toBe(false);
      }
    });
    // The picture is of the head at rest: neither a ring nor a tooltip.
    bell.blur();
    await waitFor(() => expect(document.querySelector('[role="tooltip"]')).toBeNull());
  },
};

/**
 * **Expanded** (#4754): every notice stands, the consent first as the pane wrote it, under a head
 * row that says how many and how many wait on you, with Dismiss all, which counts the notes it
 * clears and leaves the consent, and its fold. The bell's count is the same as minimised.
 */
export const Expanded: Story = {
  args: { warnings: ["vault-refused", "mcp-failed", "stale-clone", "skills-missing"] },
  play: async ({ canvas, canvasElement }) => {
    await canvas.findByRole("status", { name: "Vault" });
    await waitFor(() => expect(rowsOf(canvasElement).warnings).toHaveLength(4));
    await heldInThePane(canvasElement);
    const band = within(canvas.getByRole("region", { name: /^Pane notices/ }));
    expect(band.getByText("1 waiting on you · 3 notices")).toBeVisible();
    expect(countOf(bellOf(canvas))).toHaveTextContent(/^3$/);
    expect(bellOf(canvas)).toHaveAttribute("aria-expanded", "true");
    const all = band.getByRole("button", { name: "Dismiss all 3" });
    all.focus();
    expectFocusInSight();
    atRest();
  },
};

/**
 * **Dismiss all** (#4754): the notes go, each by its own Dismiss, and the consent stands; the
 * keyboard stays on the band's head while it is drawn.
 */
export const DismissAll: Story = {
  args: { warnings: ["vault-refused", "mcp-failed", "stale-clone", "skills-missing"] },
  parameters: { screenshot: false },
  play: async ({ canvas, canvasElement, userEvent }) => {
    const all = await canvas.findByRole("button", { name: "Dismiss all 3" });
    await userEvent.click(all);
    await waitFor(() => expect(rowsOf(canvasElement).warnings).toHaveLength(1));
    expect(canvas.getByRole("status", { name: "Vault" })).toBeVisible();
    await waitFor(() =>
      expect(bellOf(canvas)).toHaveAccessibleName("Pane notices: No notices, 1 asking"),
    );
    await untilFocused(
      () => canvas.getByRole("button", { name: "Fold the band" }),
      "the keyboard stays on the band's head",
    );
    atRest();
  },
};

/**
 * **A full band** (#4754): six notices, more than the band's half of the pane holds. Every one
 * is in the band, which scrolls inside; the header is where it is in an empty pane, so nothing
 * that arrives moves it; the terminal keeps the rest.
 */
export const FullBand: Story = {
  args: {
    warnings: [
      "vault-refused",
      "sandbox-ask",
      "mcp-failed",
      "skills-missing",
      "stale-clone",
      "sandbox-long",
    ],
  },
  play: async ({ canvas, canvasElement }) => {
    await canvas.findByRole("status", { name: "Vault" });
    await waitFor(() => expect(rowsOf(canvasElement).warnings).toHaveLength(6));
    await heldInThePane(canvasElement);
    const { frame, head, band, terminal } = rowsOf(canvasElement);
    expect(boxOf(head).top - boxOf(frame).top).toBeLessThanOrEqual(12);
    const stack = band?.querySelector('[data-part="stack"]') as HTMLElement;
    expect(stack.scrollHeight).toBeGreaterThan(stack.clientHeight);
    expect(boxOf(terminal).height).toBeGreaterThan(boxOf(frame).height * 0.45);
    expect(canvas.queryByRole("button", { name: /more/ })).toBeNull();
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
    await untilFocused(() => next, "Esc moves the keyboard to Update the clone");
    await userEvent.keyboard("{Escape}");
    await untilFocused(
      () => canvas.getByLabelText("Terminal of steward 2"),
      "Esc gives the keyboard back to the terminal",
    );
    expect(canvas.queryAllByRole("status")).toHaveLength(0);
    atRest();
  },
};

/**
 * **A consent that moved holds its Allow** (#1693, #1695): the warning above it is put away, the
 * consents move up under the pointer, and a press on an Allow at once does nothing and says
 * why: the vault's, held because it grants, and the sandbox's. The keyboard is landed on the
 * consent's line, never on its Allow. Measured in the real engine, where the warning's box is
 * positioned for its mark: the guard reads where each Allow's own box is drawn (#3167), not an
 * `offsetTop`.
 */
export const ConsentMoved: Story = {
  args: { warnings: ["skills-missing", "vault-refused", "sandbox-ask"] },
  play: async ({ canvas, userEvent }) => {
    allowed.length = 0;
    const open = await canvas.findByRole("button", { name: "Open in Skill studio" });
    open.focus();
    await userEvent.keyboard("{Escape}");
    // The vault's card moved up into the first place, and the sandbox's under it: both moved,
    // and neither is a cause the guard lists by name for the vault.
    const vault = await canvas.findByRole("status", { name: "Vault" });
    await untilFocused(() => vault, "Esc lands on the vault's card");

    await userEvent.click(
      within(vault).getByRole("button", { name: "Allow steward to use this vault" }),
    );
    expect(allowed).toEqual([]);
    expect(await canvas.findByText(NOTICE_MOVED_JUST_NOW)).toBeVisible();

    const ask = canvas.getByRole("status", { name: "Sandbox block" });
    await userEvent.click(within(ask).getByRole("button", { name: "Allow registry.npmjs.org" }));
    expect(allowed).toEqual([]);
    atRest();
  },
};

/** Two frames: whatever the last change asked to read again has been read. */
const twoFrames = () =>
  new Promise<void>((done) => requestAnimationFrame(() => requestAnimationFrame(() => done())));

/** The band, with a way to raise its warnings again once they are put away. */
function RaisedAgain(props: Parameters<typeof PaneBand>[0]) {
  const [round, setRound] = useState(0);
  return (
    <div style={{ display: "grid", gap: "var(--space-3)", justifyItems: "start" }}>
      <Button size="sm" onPress={() => setRound((was) => was + 1)}>
        Raise the warnings again
      </Button>
      <PaneBand {...props} round={round} />
    </div>
  );
}

/**
 * **The band put away in one fit** (#3437): Dismiss all takes the last notes, and the band's head
 * and room go in the same layout as they do, so the terminal is fitted once, to its full room,
 * and its chat's harness is told one size. Raised again, the band starts at its top, though it
 * was scrolled to its end when it went.
 */
export const PutAwayInOneFit: Story = {
  args: { warnings: ["sandbox-long", "mcp-failed", "stale-clone", "skills-missing"], height: 320 },
  render: (args) => <RaisedAgain {...args} />,
  // No picture: what it holds is a count of fits and a scroll; the band at rest is pictured in
  // its other stories.
  parameters: { screenshot: false },
  play: async ({ canvas, canvasElement, userEvent }) => {
    const all = await canvas.findByRole("button", { name: "Dismiss all 4" });
    await heldInThePane(canvasElement);
    const { band, terminal } = rowsOf(canvasElement);
    const stack = band?.querySelector<HTMLElement>('[data-part="stack"]');
    if (!band || !terminal || !stack) throw new Error("no band, stack or terminal drawn");
    // Scrolled to its end, as a person who read the last note leaves it.
    stack.scrollTop = stack.scrollHeight;
    await waitFor(() => expect(stack.scrollTop).toBeGreaterThan(0));

    // Every size the terminal is laid out at, one a frame, as its fit reads it.
    const fits: number[] = [];
    const sizes = new ResizeObserver((entries) => {
      for (const entry of entries) fits.push(entry.contentRect.height);
    });
    sizes.observe(terminal);
    await twoFrames();
    // The first is where it stands now.
    fits.splice(0);
    await userEvent.click(all);
    await waitFor(() => expect(band).toHaveAttribute("data-empty"));
    await twoFrames();
    await twoFrames();
    sizes.disconnect();
    // One fit, not one a row short and then another.
    expect(fits).toHaveLength(1);

    await userEvent.click(canvas.getByRole("button", { name: "Raise the warnings again" }));
    const first = await canvas.findByRole("status", { name: "Sandbox block" });
    expect(stack.scrollTop).toBe(0);
    expect(first.getBoundingClientRect().top).toBeGreaterThanOrEqual(
      stack.getBoundingClientRect().top - 1,
    );
    atRest();
  },
};

/**
 * **A consent whose Allow its words moved holds it** (#3167): the window is narrowed, the
 * vault's words wrap onto one more line and its ways out go down under them, while the line's
 * own top stays where it was. A press on the Allow at once does nothing and says why; once it
 * has stood still for the settle time, the same press allows.
 */
export const ConsentRewraps: Story = {
  args: { warnings: ["vault-refused"] },
  play: async ({ canvas, canvasElement, userEvent }) => {
    allowed.length = 0;
    const vault = await canvas.findByRole("status", { name: "Vault" });
    const allow = within(vault).getByRole("button", { name: "Allow steward to use this vault" });
    await twoFrames();
    const was = { line: boxOf(vault).top, allow: boxOf(allow) };

    const layout = canvasElement.querySelector(".editor") as HTMLElement;
    layout.style.inlineSize = "420px";
    await waitFor(() => {
      const now = boxOf(allow);
      expect(
        Math.abs(now.top - was.allow.top) + Math.abs(now.left - was.allow.left),
      ).toBeGreaterThan(4);
    });
    // The line stood still: only the Allow's own box says it moved.
    expect(boxOf(vault).top).toBeCloseTo(was.line, 0);

    await userEvent.click(allow);
    expect(allowed).toEqual([]);
    expect(await canvas.findByText(NOTICE_MOVED_JUST_NOW)).toBeVisible();

    await new Promise((done) => setTimeout(done, SETTLE_MS + 100));
    await userEvent.click(allow);
    expect(allowed).toEqual(["forge"]);
    atRest();
  },
};

/**
 * **Holds the page's `performance.now()` where it is now** until the returned function lets it
 * go (#4685): what the press-settle guard reads as the time, so a press that took long to arrive
 * from outside the page is read as one made at once. Letting go twice is letting go once.
 */
function holdTheClock(): () => void {
  const real = performance.now;
  const at = real.call(performance);
  performance.now = () => at;
  return () => {
    performance.now = real;
  };
}

/**
 * **A consent pressed with the pointer, as a person does** (#3399): the real engine's own mouse,
 * driven from outside the page (Playwright's), never an event the page makes for itself. A press
 * on the Allow just after the warning above it was put away, and the consent moved up under the
 * pointer, does nothing and says why; once it has stood still for the settle time, the same
 * press allows. Every end-to-end press of a grant goes through the keyboard (`pressGrant`), so
 * this is where a pointer's press on one is held and let through in a real engine.
 *
 * **The page's clock is held still while the engine's press travels** (#4685): the press goes
 * out of the page to the test runner and Playwright, which waits for the Allow to be stable, and
 * comes back as the engine's own mouse. On a loaded runner that trip took longer than the settle
 * time, so the guard rightly let the press through: the consent had stood still for 1.5 s by
 * the clock the guard reads (`performance.now()`). Held, that clock says the press came at once,
 * however long the trip; let go, the second press waits the settle time for real.
 */
export const ConsentPressedByThePointer: Story = {
  args: { warnings: ["skills-missing", "vault-refused"] },
  play: async ({ canvas, userEvent }) => {
    allowed.length = 0;
    // The engine's own mouse: imported only where a story runs in it.
    const { userEvent: mouse } = await import("vitest/browser");
    const open = await canvas.findByRole("button", { name: "Open in Skill studio" });
    open.focus();
    await userEvent.keyboard("{Escape}");
    const vault = await canvas.findByRole("status", { name: "Vault" });
    const allow = within(vault).getByRole("button", { name: "Allow steward to use this vault" });
    const letGo = holdTheClock();
    try {
      await mouse.click(allow);
      letGo();
      expect(allowed).toEqual([]);
      expect(await canvas.findByText(NOTICE_MOVED_JUST_NOW)).toBeVisible();

      await new Promise((done) => setTimeout(done, SETTLE_MS + 100));
      await mouse.click(allow);
    } finally {
      letGo();
      // **This story's picture is taken after its `play`, and of the band at rest**: the engine's
      // mouse is taken off the Allow it pressed, which would otherwise be drawn hovered.
      await mouse.hover(document.body, { position: { x: 0, y: 0 } });
    }
    expect(allowed).toEqual(["forge"]);
    atRest();
  },
};

/**
 * **A consent the band was made to scroll back holds its Allow** (#3167): the band is scrolled
 * to its end, a long warning under the consent is put away, and the band, holding less, can no
 * longer stay scrolled: the consent slides down on screen though its place in the band is the
 * same, and the Allow as wide. A press on its Allow at once does nothing. Scrolling the band
 * yourself is no move: the Allow pressed just after that scroll allows.
 */
export const ConsentClamped: Story = {
  args: { warnings: ["vault-refused", "mcp-failed", "skills-missing", "sandbox-long"], width: 400 },
  play: async ({ canvas, userEvent }) => {
    allowed.length = 0;
    const vault = await canvas.findByRole("status", { name: "Vault" });
    const allow = within(vault).getByRole("button", { name: "Allow steward to use this vault" });
    const stack = vault.closest('[data-scope="notice-row"][data-part="stack"]') as HTMLElement;
    // The clamp alone, and no re-wrap: the band keeps its scrollbar's room whether it overflows
    // or not, so its words are as wide before the dismiss as after it.
    stack.style.scrollbarGutter = "stable";
    await waitFor(() => expect(stack.scrollHeight).toBeGreaterThan(stack.clientHeight + 24));
    // Start from a consent that has stood still for the settle time, and is pressed as it is.
    await new Promise((done) => setTimeout(done, SETTLE_MS + 100));
    await userEvent.click(allow);
    expect(allowed).toEqual(["forge"]);
    allowed.length = 0;

    // The person scrolls the band to its end: a scroll, and no move. Their wheel is heard first
    // (#3217); a scroll with nothing from the person before it would be one.
    stack.dispatchEvent(new WheelEvent("wheel", { bubbles: true, deltaY: stack.scrollHeight }));
    stack.scrollTop = stack.scrollHeight;
    await waitFor(() => expect(stack.scrollTop).toBeGreaterThan(24));
    await twoFrames();
    await userEvent.click(allow);
    expect(allowed).toEqual(["forge"]);
    allowed.length = 0;

    const inBand = () => ({
      top: boxOf(allow).top - boxOf(stack).top + stack.scrollTop,
      left: boxOf(allow).left - boxOf(stack).left,
      width: boxOf(allow).width,
    });
    const was = { inBand: inBand(), onScreen: boxOf(allow).top };
    const long = canvas.getByRole("status", { name: "Sandbox block" });
    await userEvent.click(within(long).getByRole("button", { name: "Dismiss" }));
    await waitFor(() => expect(canvas.queryByRole("status", { name: "Sandbox block" })).toBeNull());
    // Clamped: the Allow as wide and at the same place in the band, only lower on screen.
    const now = inBand();
    expect(now.top).toBeCloseTo(was.inBand.top, 0);
    expect(now.left).toBeCloseTo(was.inBand.left, 0);
    expect(now.width).toBeCloseTo(was.inBand.width, 0);
    expect(boxOf(allow).top - was.onScreen).toBeGreaterThan(24);

    await userEvent.click(allow);
    expect(allowed).toEqual([]);
    expect(await canvas.findByText(NOTICE_MOVED_JUST_NOW)).toBeVisible();
    stack.style.scrollbarGutter = "";
    atRest();
  },
};

/**
 * **A narrow pane's band says it has more** (#3168): two warnings are taller than the band's
 * room, so it scrolls. At rest a fade at its bottom says more is below; scrolled to its end, the
 * fade is at its top alone; partway, at both. The rows fade into the band's ground they
 * stand on, and the fade is never in the way of a press. The picture is the band partway, with
 * both.
 */
export const NarrowScrolled: Story = {
  args: { warnings: ["vault-refused", "mcp-failed", "skills-missing", "sandbox-long"], width: 400 },
  play: async ({ canvas, canvasElement }) => {
    const vault = await canvas.findByRole("status", { name: "Vault" });
    const stack = vault.closest('[data-scope="notice-row"][data-part="stack"]') as HTMLElement;
    await waitFor(() => expect(stack.scrollHeight).toBeGreaterThan(stack.clientHeight + 24));
    // The band's own ground, which the rows stand on (#5215).
    const ground = getComputedStyle(rowsOf(canvasElement).band as Element).backgroundColor;
    const fade = (edge: "::before" | "::after") => getComputedStyle(stack, edge);
    const shows = async (above: boolean, below: boolean) => {
      await waitFor(() => {
        expect(stack.hasAttribute("data-more-above")).toBe(above);
        expect(stack.hasAttribute("data-more-below")).toBe(below);
      });
      expect(fade("::before").opacity).toBe(above ? "1" : "0");
      expect(fade("::after").opacity).toBe(below ? "1" : "0");
    };
    const scrollTo = (top: number) => {
      // The person's wheel first (#3217), so the scroll is theirs and no move.
      stack.dispatchEvent(
        new WheelEvent("wheel", { bubbles: true, deltaY: top - stack.scrollTop }),
      );
      stack.scrollTop = top;
    };

    // Into the ground the warnings stand on, and never what a press lands on.
    for (const edge of ["::before", "::after"] as const) {
      expect(fade(edge).backgroundImage).toContain(ground);
      expect(fade(edge).pointerEvents).toBe("none");
    }
    scrollTo(0);
    await shows(false, true);
    // The fade lies over the band and takes none of its room: the first warning is at its top.
    expect(boxOf(vault.parentElement).top - boxOf(stack).top).toBeLessThanOrEqual(1);

    scrollTo(stack.scrollHeight);
    await shows(true, false);
    // The last warning ends at the band's end, under the fade and not pushed past it.
    const last = canvas.getByRole("status", { name: "Sandbox block" }).parentElement;
    expect(boxOf(stack).bottom - boxOf(last).bottom).toBeLessThanOrEqual(1);

    // The keyboard landing on a way out from below stops clear of the fade (WCAG 2.4.11).
    const allow = within(vault).getByRole("button", { name: "Allow steward to use this vault" });
    allow.focus();
    await untilFocused(() => allow, "the vault's Allow, from below");
    // Wherever the band came to rest, no fade lies over the way out.
    if (stack.hasAttribute("data-more-above"))
      expect(boxOf(allow).top - boxOf(stack).top).toBeGreaterThanOrEqual(
        parseFloat(fade("::before").height) - 1,
      );
    atRest();

    // **A fade never dims a way out**: a consent's ways, resting in the fade at either edge, read
    // at full strength. The fade takes no pointer, so a hit test would pass through it: for this
    // one reading it takes the pointer, and the hit test then answers in paint order.
    const ways = within(vault).getAllByRole("button");
    const looks = (way: Element) => {
      const { color, backgroundColor } = getComputedStyle(way);
      let opacity = 1;
      for (let at: Element | null = way; at !== null; at = at.parentElement)
        opacity *= Number(getComputedStyle(at).opacity);
      return { color, backgroundColor, opacity };
    };
    scrollTo(0);
    const undimmed = ways.map(looks);
    const fullStrength = (edge: "::before" | "::after", from: (way: Element) => number) => {
      const reach = parseFloat(fade(edge).height);
      const hittable = document.createElement("style");
      hittable.textContent = `[data-scope="notice-row"][data-part="stack"]::before,
        [data-scope="notice-row"][data-part="stack"]::after { pointer-events: auto; }`;
      document.head.append(hittable);
      try {
        ways.forEach((way, at) => {
          expect(from(way), "the way out rests in the fade").toBeLessThan(reach);
          const box = boxOf(way);
          const top = document.elementFromPoint(box.left + box.width / 2, box.top + box.height / 2);
          expect(way.contains(top), `${way.textContent} is on top of the fade`).toBe(true);
          expect(looks(way)).toEqual({ ...undimmed[at], opacity: 1 });
        });
      } finally {
        hittable.remove();
      }
    };
    // At the top edge, with more above.
    scrollTo(stack.scrollTop + boxOf(ways[0]).top - boxOf(stack).top - 2);
    await waitFor(() => expect(stack.hasAttribute("data-more-above")).toBe(true));
    fullStrength("::before", (way) => boxOf(way).top - boxOf(stack).top);
    // At the bottom edge, with more below: the band held to just under the consent's ways.
    scrollTo(0);
    stack.style.maxBlockSize = `${boxOf(ways[0]).bottom - boxOf(stack).top + 2}px`;
    await waitFor(() => expect(stack.hasAttribute("data-more-below")).toBe(true));
    fullStrength("::after", (way) => boxOf(stack).bottom - boxOf(way).bottom);
    stack.style.maxBlockSize = "";

    scrollTo(Math.round((stack.scrollHeight - stack.clientHeight) / 2));
    await shows(true, true);
    atRest();
  },
};

/**
 * **The pane's actions** (`PaneActions`, shared with a view's pane, #3147): split side by side,
 * split stacked and close, after a divider from the chat's actions, at the pane's top right, drawn by the catalogue's own
 * rows, and always drawn (#5318).
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
    expect(canvas.getByRole("button", { name: "Close this pane" })).toHaveFocus();
    expectFocusInSight();
    // At the pane's top right, in the header's row.
    const { frame, head } = rowsOf(canvasElement);
    const actions = boxOf(head?.querySelector(".pane-actions"));
    expect(boxOf(frame).right - actions.right).toBeLessThanOrEqual(16);
    expect(actions.top - boxOf(frame).top).toBeLessThanOrEqual(16);
    // Taken off them, so the picture is the head at rest: drawn with no keyboard and no tooltip.
    (document.activeElement as HTMLElement | null)?.blur();
    await waitFor(() => expect(document.querySelector('[role="tooltip"]')).toBeNull());
    expect(canvas.getByRole("button", { name: "Split right" })).toBeVisible();
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
    const words = boxOf(block.querySelector('[data-part="words"]'));
    const stack = block.parentElement?.parentElement as HTMLElement;
    const ways = within(block).getAllByRole("button");
    for (const way of ways) {
      expect(boxOf(way).top).toBeGreaterThanOrEqual(words.bottom - 1);
      expect(boxOf(way).right).toBeLessThanOrEqual(pane.right + 1);
    }
    for (const way of ways) {
      // Whole, once the band is scrolled to it: never cut by its own warning (rule 6).
      way.scrollIntoView({ block: "nearest" });
      expect(boxOf(way).bottom).toBeLessThanOrEqual(boxOf(stack).bottom + 1);
      expect(boxOf(way).top).toBeGreaterThanOrEqual(boxOf(stack).top - 1);
    }
    stack.scrollTop = 0;
    expect(boxOf(block.parentElement).right).toBeLessThanOrEqual(pane.right + 1);
    // **Its last row whole** (#5443): what the row says and its ways out stand inside its own
    // edge, with the row's room around them, its ways never on its bottom edge.
    roomInsideTheRow(block);
  },
};

/**
 * **A band row keeps its room inside its edge** (#5443): its mark, its words and its ways out
 * each stand at least 1px inside the row's border on every side, so the row's last line is never
 * drawn on its edge, cut (it stood at 0px when the row's own padding was lost). Measured inside the row, so no runner's fonts or window decide it.
 */
function roomInsideTheRow(line: HTMLElement) {
  const row = line.closest('[data-scope="notice"][data-part="root"]') as HTMLElement;
  const edge = boxOf(row);
  const border = parseFloat(getComputedStyle(row).borderTopWidth) || 0;
  for (const part of ["mark", "words", "ways"]) {
    const at = boxOf(line.querySelector(`:scope > [data-part="${part}"]`));
    const said = `the row's ${part} at ${JSON.stringify(at)} in its edge at ${JSON.stringify(edge)}`;
    expect(at.top - (edge.top + border), said).toBeGreaterThanOrEqual(1);
    expect(edge.bottom - border - at.bottom, said).toBeGreaterThanOrEqual(1);
    expect(at.left - (edge.left + border), said).toBeGreaterThanOrEqual(1);
    expect(edge.right - border - at.right, said).toBeGreaterThanOrEqual(1);
  }
}

/** The panes the stories below draw: each a pane never drawn before. */
let drawnPanes = 0;

/**
 * **A pane whose band is drawn in steps** (#3302): "Next" brings the vault's consent into the
 * empty band (`arrives`), or draws the pane anew in another frame with the same consent where it
 * stood (`redrawn`), as a task's pane is once its task ended.
 */
function InSteps({ how }: { how: "arrives" | "redrawn" }) {
  const [band] = useState(() => paneBand("stories", drawnPanes++));
  const [step, setStep] = useState(0);
  const warnings: Warning[] = how === "redrawn" || step > 0 ? ["vault-refused"] : [];
  const pane = <Pane name="steward 2" warnings={warnings} focused band={band} />;
  return (
    <div className="editor" style={{ inlineSize: 760, blockSize: 460 }}>
      <button type="button" onClick={() => setStep((was) => was + 1)}>
        Next
      </button>
      {how === "redrawn" && step % 2 === 1 ? (
        <section style={{ blockSize: "100%" }}>{pane}</section>
      ) : (
        <div style={{ blockSize: "100%" }}>{pane}</div>
      )}
    </div>
  );
}

/**
 * **A pane's first Notice is pressed at once** (#3302, D-3167-5): drawn into an empty band,
 * where nothing else was aimed at, the vault's Allow allows on the first press. Read in the real
 * engine, where the empty band is not drawn until it holds a Notice.
 */
export const FirstConsentStands: Story = {
  args: { warnings: [] },
  parameters: { screenshot: false },
  render: () => <InSteps how="arrives" />,
  play: async ({ canvas, userEvent }) => {
    allowed.length = 0;
    await twoFrames();
    await userEvent.click(canvas.getByRole("button", { name: "Next" }));
    const allow = await canvas.findByRole("button", { name: "Allow steward to use this vault" });
    await userEvent.click(allow);
    expect(allowed).toEqual(["forge"]);
    expect(canvas.queryByText(NOTICE_MOVED_JUST_NOW)).toBeNull();
    atRest();
  },
};

/**
 * **A consent that stood still while its pane was drawn anew is pressed at once** (#3302,
 * D-3167-10): the pane is drawn in another frame, its band with it, and the vault's Allow is
 * where it was: the first press allows. Read in the real engine, where the band drawn anew is
 * not drawn until it has counted what it holds.
 */
export const ConsentRedrawn: Story = {
  args: { warnings: [] },
  parameters: { screenshot: false },
  render: () => <InSteps how="redrawn" />,
  play: async ({ canvas, userEvent }) => {
    allowed.length = 0;
    await canvas.findByRole("button", { name: "Allow steward to use this vault" });
    // It stands longer than the settle time, then its pane is drawn anew.
    await new Promise((done) => setTimeout(done, SETTLE_MS + 100));
    await userEvent.click(canvas.getByRole("button", { name: "Next" }));
    await twoFrames();
    const allow = canvas.getByRole("button", { name: "Allow steward to use this vault" });
    await userEvent.click(allow);
    expect(allowed).toEqual(["forge"]);
    expect(canvas.queryByText(NOTICE_MOVED_JUST_NOW)).toBeNull();
    atRest();
  },
};
