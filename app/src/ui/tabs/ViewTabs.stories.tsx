// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **A view tab and its view, as one piece** (#3147, the operator's ruling of 2026-10-24): the
 * session tabs docked on the content panel, a view's tab chosen, and under it the view's pane
 * with no title of its own. The tab is the view's name; the view starts on the fill the chosen
 * tab joins, with no band of its own; the pane's actions (split side by side, split stacked,
 * close) are the chat pane's own cluster in the chat pane's place (`PaneHead`, `PaneActions`),
 * and a view's first row keeps their room. The content panel is the chosen tab's panel, named
 * by it.
 *
 * Settings, a vault, a persona and the shared memory, each in every cell of the theme matrix
 * (Purlis and Purlis Classic, `docs/component-stories.md`). The frame, the first row and the
 * pane's actions are the window's own components and classes; what each view lists under its
 * first row is a stand-in in the window's own classes and parts, as `PaneBand.stories.tsx`
 * stands in for a terminal: a view's page cannot ask the core for what it reads.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import type { ComponentType, ReactNode } from "react";
import { expect, waitFor, within } from "storybook/test";
import { Archive, Brain, KeyRound, Settings2, UserRound, X } from "lucide-react";
import { PaneActions, PaneHead } from "../../LayoutPanes";
import { HeadingOffer } from "../../PanelSection";
import type { Offer } from "../../actions";
import { ChatMark } from "../../NeedsYou";
import { SettingGroup, SettingsLayout } from "../../settings/components";
import { useTabIds, StripTablist } from "../../StripTablist";
import { Button } from "../button/Button";
import { panelRoot } from "../panel/Panel";
import { TabCell, tabPart, tabsPart } from "./Tabs";
import { expectFocusInSight } from "../../stories.testkit";

/** The view a story shows. */
type Shown = "settings" | "vault" | "persona" | "memory";

/** A view's pane's own rows of the catalogue, as the window's catalogue offers them. */
const PANE_OFFERS: Record<string, Offer> = {
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
    title: "Close this view",
    available: true,
    reason: "",
    does: { verb: "closePane", ends: false },
  },
};

/** The shared store's `+` and archive, as the catalogue offers them on its list's first row. */
const NEW_MEMORY: Offer = {
  id: "memory.new:shared",
  title: "New shared memory",
  available: true,
  reason: "",
  does: { verb: "newMemory", scope: { kind: "shared" } },
};
const OPEN_ARCHIVE: Offer = {
  id: "memory.archived:shared",
  title: "Open the shared archive",
  available: true,
  reason: "",
  does: {
    verb: "openView",
    view: { from: null, view: "memory-archive", key: "shared" },
    title: "Shared archive",
  },
};

/** What each view's tab is called and marked with, as the window names its tabs. */
const TABS: Record<Shown, { title: string; mark: ComponentType<{ className?: string }> }> = {
  settings: { title: "Settings", mark: Settings2 },
  vault: { title: "forge", mark: KeyRound },
  persona: { title: "steward", mark: UserRound },
  memory: { title: "Shared memory", mark: Brain },
};

/** The view's first row, as `ViewPane` and `VaultTab` draw it: what can be done to what it is
 *  about, and the quiet words beside it. Settings draws its own (its levels). */
function FirstRow({ shown }: { shown: Shown }): ReactNode {
  const pressed = () => undefined;
  switch (shown) {
    case "settings":
      return null;
    case "vault":
      return (
        <div className="view-head">
          <span className="view-head-says">Keychain · 3 secrets</span>
          <Button size="sm">Delete vault…</Button>
        </div>
      );
    case "persona":
      return (
        <div className="view-head">
          <Button size="sm">Edit persona.md</Button>
          <Button size="sm">Profile…</Button>
          <Button size="sm">Delete persona…</Button>
        </div>
      );
    case "memory":
      return (
        <div className="view-head">
          <HeadingOffer offer={NEW_MEMORY} onPress={pressed} />
          <HeadingOffer offer={OPEN_ARCHIVE} onPress={pressed} mark={Archive} />
        </div>
      );
  }
}

/** What each view lists under its first row: a stand-in in the window's own classes. */
function Body({ shown }: { shown: Shown }): ReactNode {
  switch (shown) {
    case "settings":
      return (
        <SettingsLayout
          levels={[
            { id: "you", label: "You" },
            { id: "project", label: "Project" },
            { id: "workspace", label: "Workspace" },
          ]}
          level="project"
          onLevelChange={() => undefined}
          about="This project, for everyone who opens it. Never put a secret in its files: keep it in a vault and name it as vault:<vault>/<key>."
          groups={[
            { id: "general", label: "General" },
            { id: "chats", label: "Chats" },
            { id: "theme", label: "Theme" },
          ]}
          group="general"
          onGroupChange={() => undefined}
          filter=""
          onFilterChange={() => undefined}
        >
          <SettingGroup
            label="General"
            help="What every chat in this project starts with."
            kept="Kept in purlis.toml, which your team sees."
          >
            <p className="ui-setting-help">The default persona is steward.</p>
          </SettingGroup>
        </SettingsLayout>
      );
    case "vault":
      return (
        <table className="ui-table">
          <thead>
            <tr>
              <th scope="col">Key</th>
              <th scope="col">Value</th>
            </tr>
          </thead>
          <tbody>
            {["GITHUB_TOKEN", "NPM_TOKEN", "SENTRY_DSN"].map((key) => (
              <tr key={key}>
                <td>
                  <code>{key}</code>
                </td>
                <td>••••••••</td>
              </tr>
            ))}
          </tbody>
        </table>
      );
    case "persona":
      return (
        <>
          <p>
            Keeps the project in order: routes work to the right persona, and closes what is done.
          </p>
          <dl>
            <dt>Profile</dt>
            <dd>default</dd>
            <dt>Memory</dt>
            <dd>12 memories</dd>
          </dl>
        </>
      );
    case "memory":
      return (
        <ul className="panel-rows">
          {["Charter defects go upstream", "Never pkill by name", "Separate PRs, not trains"].map(
            (name) => (
              <li key={name} className="panel-row">
                <span className="row">
                  <span className="row-text">{name}</span>
                </span>
              </li>
            ),
          )}
        </ul>
      );
  }
}

/** **The strip, docked on the content panel, a chat's tab and the chosen view's tab, and the
 *  view's pane under it**, as `LayoutPanes` and `ViewPane` draw it. */
function ViewTab({ shown, width = 760 }: { shown: Shown; width?: number }) {
  const idOf = useTabIds();
  const { title, mark: Mark } = TABS[shown];
  const tabs = [
    { name: "steward 11", view: false },
    { name: title, view: true },
  ];
  const chosen = 1;
  return (
    <div className="editor" {...tabsPart("layout")} style={{ inlineSize: width, blockSize: 460 }}>
      <header className="bar" {...tabsPart("strip")}>
        <div className="tabs" data-strip="Chats">
          <StripTablist name="Chats" ids={tabs.map((_, at) => idOf(at))} />
          {tabs.map((tab, at) => (
            <TabCell
              key={tab.name}
              className="tab"
              variant={tab.view ? "view" : "chat"}
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
                {tab.view ? (
                  <Mark className="tab-mark" aria-hidden="true" />
                ) : (
                  <ChatMark state="waiting" />
                )}
                <span className="tab-name" {...tabPart("label")}>
                  {tab.name}
                </span>
              </button>
              <button
                type="button"
                className="closer"
                {...tabPart("closer")}
                tabIndex={-1}
                aria-label={`Close ${tab.name}`}
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
        <div className="pane-frame pane-viewed" data-focused="" data-testid="view frame">
          <PaneHead
            doing={
              <PaneActions
                pane={0}
                offerFor={(id) => PANE_OFFERS[id]}
                closeOf={() => PANE_OFFERS["pane.close"]}
                onPaneDoes={() => undefined}
              />
            }
          />
          <div className="pane view focused">
            <section className="view-pane" aria-label={title}>
              <FirstRow shown={shown} />
              <div className="view-body">
                <Body shown={shown} />
              </div>
            </section>
          </div>
        </div>
      </div>
    </div>
  );
}

const meta = {
  component: ViewTab,
} satisfies Meta<typeof ViewTab>;

export default meta;
type Story = StoryObj<typeof meta>;

const boxOf = (el: Element | null | undefined): DOMRect => (el as Element).getBoundingClientRect();

/** Whether the tabs are docked on the panel in the cell's theme (Purlis), not on a strip of
 *  their own (Purlis Classic): only then does the chosen tab join what is under it. */
function docked(canvas: HTMLElement): boolean {
  const layout = canvas.querySelector('[data-scope="tabs"][data-part="layout"]');
  return (
    layout !== null && getComputedStyle(layout).getPropertyValue("--tabs-joined").trim() === "1"
  );
}

/** Whether `el` paints a fill of its own: a fill other than an element's with none. */
function filled(el: Element): boolean {
  const probe = document.createElement("div");
  document.body.append(probe);
  const none = getComputedStyle(probe).backgroundColor;
  probe.remove();
  return getComputedStyle(el).backgroundColor !== none;
}

/**
 * **What every view tab is held to** (#3147):
 *
 * - the view says neither its tab's name nor anything as a heading of its own;
 * - the content panel is the chosen tab's panel, named by it;
 * - nothing between the view's top and its body has a fill of its own, and where the tabs are
 *   docked the chosen tab joins the view on the view's fill, its bottom on the pane's top;
 * - the pane's actions are the chat pane's cluster in the chat pane's place, the pane's top
 *   right, and nothing in the view's first row is under them.
 */
async function oneWithItsTab(canvasElement: HTMLElement, title: string) {
  const canvas = within(canvasElement);
  const tab = canvas.getByRole("tab", { name: title });
  const panel = canvas.getByRole("tabpanel");
  expect(panel).toHaveAttribute("aria-labelledby", tab.id);
  expect(panel).toHaveAccessibleName(title);

  const frame = canvasElement.querySelector('[data-testid="view frame"]') as HTMLElement;
  const view = frame.querySelector(".view-pane") as HTMLElement;
  // No title: not as a heading, and not as words of its own anywhere in the view.
  expect(within(view).queryByRole("heading", { level: 1 })).toBeNull();
  expect(within(view).queryByRole("heading", { level: 2 })).toBeNull();
  expect(within(view).queryByText(title, { exact: true })).toBeNull();

  // No band: the view's first row, and everything between the pane's top and the body, draws
  // on the fill under it.
  const first = view.querySelector(".view-head");
  if (first !== null) expect(filled(first)).toBe(false);
  expect(filled(view)).toBe(false);
  const ground = frame.querySelector(".pane.view") as HTMLElement;
  if (docked(canvasElement)) {
    const cell = tab.closest('[data-part="tab"]') as Element;
    expect(boxOf(cell).bottom).toBeCloseTo(boxOf(frame).top, 0);
    expect(getComputedStyle(cell).backgroundColor).toBe(getComputedStyle(ground).backgroundColor);
  }

  // The actions, drawn as the keyboard reaches them: the chat pane's cluster, at the pane's top
  // right, and clear of everything the first row holds.
  const split = canvas.getByRole("button", { name: "Split right" });
  split.focus();
  await waitFor(() => expect(split).toBeVisible());
  const actions = frame.querySelector(".pane-head > .pane-actions") as HTMLElement;
  expect(actions).not.toBeNull();
  const at = boxOf(actions);
  expect(boxOf(frame).right - at.right).toBeLessThanOrEqual(16);
  expect(at.top - boxOf(frame).top).toBeLessThanOrEqual(16);
  const row = first ?? view.querySelector(".ui-settings-top");
  for (const one of row?.querySelectorAll("button, [role='radio'], .view-head-says") ?? []) {
    const box = boxOf(one);
    if (box.top < at.bottom && box.bottom > at.top) expect(box.right).toBeLessThanOrEqual(at.left);
  }
  expect(
    within(actions)
      .getAllByRole("button")
      .map((one) => one.getAttribute("aria-label")),
  ).toEqual(["Split right", "Split down", "Close this view"]);
}

/** Puts the keyboard down, so the picture shows the view at rest. */
const atRest = () => (document.activeElement as HTMLElement | null)?.blur();

/**
 * **Settings**: its levels are its first row, at the pane's top, the pane's actions at the
 * row's end; no "Settings" above them. The keyboard goes from the actions to the levels.
 */
export const Settings: Story = {
  args: { shown: "settings" },
  play: async ({ canvas, canvasElement, userEvent }) => {
    await oneWithItsTab(canvasElement, "Settings");
    const project = canvas.getByRole("radio", { name: "Project" });
    // The levels share the actions' row.
    const actions = boxOf(canvasElement.querySelector(".pane-actions"));
    expect(boxOf(project).top).toBeLessThan(actions.bottom);
    canvas.getByRole("button", { name: "Close this view" }).focus();
    await userEvent.tab();
    await waitFor(() => expect(project).toHaveFocus());
    expectFocusInSight();
    atRest();
  },
};

/**
 * **A vault**: what kind of vault it is and how much it holds stay as quiet words in its first
 * row, beside Delete vault…; its name is its tab's alone.
 */
export const Vault: Story = {
  args: { shown: "vault" },
  play: async ({ canvas, canvasElement, userEvent }) => {
    await oneWithItsTab(canvasElement, "forge");
    canvas.getByRole("button", { name: "Close this view" }).focus();
    await userEvent.tab();
    await waitFor(() =>
      expect(canvas.getByRole("button", { name: "Delete vault…" })).toHaveFocus(),
    );
    expectFocusInSight();
    atRest();
  },
};

/** **A persona**: its own rows on its first row, no name or mark above them. */
export const Persona: Story = {
  args: { shown: "persona" },
  play: async ({ canvas, canvasElement, userEvent }) => {
    await oneWithItsTab(canvasElement, "steward");
    canvas.getByRole("button", { name: "Close this view" }).focus();
    for (const name of ["Edit persona.md", "Profile…", "Delete persona…"]) {
      await userEvent.tab();
      await waitFor(() => expect(canvas.getByRole("button", { name })).toHaveFocus());
      expectFocusInSight();
    }
    atRest();
  },
};

/** **The shared memory**: its `+` and its archive on its first row, the list under them. */
export const Memory: Story = {
  args: { shown: "memory" },
  play: async ({ canvas, canvasElement, userEvent }) => {
    await oneWithItsTab(canvasElement, "Shared memory");
    canvas.getByRole("button", { name: "Close this view" }).focus();
    await userEvent.tab();
    await waitFor(() =>
      expect(canvas.getByRole("button", { name: "New shared memory" })).toHaveFocus(),
    );
    expectFocusInSight();
    atRest();
  },
};

/** **A narrow pane**: the persona's first row wraps under the actions' room, never under them. */
export const Narrow: Story = {
  args: { shown: "persona", width: 400 },
  play: async ({ canvasElement }) => {
    await oneWithItsTab(canvasElement, "steward");
    atRest();
  },
};
