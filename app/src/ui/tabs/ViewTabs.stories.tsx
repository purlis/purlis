// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **A view tab and its view, as main draws them before #3147**: the
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
import { Archive, Brain, KeyRound, Settings2, UserRound, X } from "lucide-react";
import { PaneDoing } from "../../LayoutPanes";
import { HeadingOffer } from "../../PanelSection";
import type { Offer } from "../../actions";
import { ChatMark } from "../../NeedsYou";
import { SettingGroup, SettingsLayout } from "../../settings/components";
import { useTabIds, StripTablist } from "../../StripTablist";
import { Button } from "../button/Button";
import { panelRoot } from "../panel/Panel";
import { TabCell, tabPart, tabsPart } from "./Tabs";

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

/** The view's heading, as main's `ViewPane` and `VaultTab` draw it: its mark, its title, and
 *  what can be done to what it is about. */
function FirstRow({ shown }: { shown: Shown }): ReactNode {
  const pressed = () => undefined;
  const { title, mark: Mark } = TABS[shown];
  const heading = (
    <h2>
      <Mark className="tab-mark" aria-hidden="true" />
      {title}
      {shown === "vault" && <span className="panel-from">{" · Keychain · 3 secrets"}</span>}
    </h2>
  );
  switch (shown) {
    case "settings":
      return <header className="view-head">{heading}</header>;
    case "vault":
      return (
        <header className="view-head">
          {heading}
          <Button size="sm">Delete vault…</Button>
        </header>
      );
    case "persona":
      return (
        <header className="view-head">
          {heading}
          <Button size="sm">Edit persona.md</Button>
          <Button size="sm">Profile…</Button>
          <Button size="sm">Delete persona…</Button>
        </header>
      );
    case "memory":
      return (
        <header className="view-head">
          {heading}
          <HeadingOffer offer={NEW_MEMORY} onPress={pressed} />
          <HeadingOffer offer={OPEN_ARCHIVE} onPress={pressed} mark={Archive} />
        </header>
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
        <div className="pane-frame" data-focused="" data-testid="view frame">
          <div className="pane-corner at-end">
            <PaneDoing
              pane={0}
              offerFor={(id) => PANE_OFFERS[id]}
              closeOf={() => PANE_OFFERS["pane.close"]}
              onPaneDoes={() => undefined}
            />
          </div>
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

export const Settings: Story = { args: { shown: "settings" } };
export const Vault: Story = { args: { shown: "vault" } };
export const Persona: Story = { args: { shown: "persona" } };
export const Memory: Story = {
  args: { shown: "memory" },
  // Main's look, kept as the before picture: its `+` and archive stand 4px apart in the compact
  // density, under the 24px target spacing (WCAG 2.5.8). #3147's first row spaces them.
  parameters: { a11y: { options: { rules: { "target-size": { enabled: false } } } } },
};
export const Narrow: Story = { args: { shown: "persona", width: 400 } };
