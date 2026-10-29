// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Explorer opens straight to the files** (#3343, the sidebar-views prototype's Q2 A, "Repos as
 * folders"), in a side panel 276px wide as the window lays a view out: the region's head is the
 * view's one title, the workspace, and the view is one scroll whose top (the filter, the scope,
 * the Search link) is held at the top. One repo is its tree, open at once, its branch beside the
 * title; several are a closed row each, with how many files it changed and its branch.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { expect, waitFor } from "storybook/test";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { Explorer, type ExplorerPart } from "./Explorer";
import { ChatsHere, fixedChats, nothingKnown } from "./chatState";
import type { FileScope, FolderEntry, Panels as PanelsModel, PlaneId } from "./bindings";
import type { WorkspaceState } from "./workspaceState";
import { Panel, PanelTitle } from "./ui/panel/Panel";
import { HeadActions } from "./viewHead";

const PLANE = "/plane" as unknown as PlaneId;

const entry = (name: string, folder = false): FolderEntry => ({
  name,
  kind: folder ? "folder" : "file",
  ignored: false,
  refused: null,
});

/** Each repo's own folder, as the prototype's tree draws `purlis`'s. */
const TOP: FolderEntry[] = [
  entry("app", true),
  entry("crates", true),
  entry("docs", true),
  entry(".gitignore"),
  entry("Cargo.toml"),
  entry("README.md"),
];
const UNDER: Record<string, FolderEntry[]> = {
  app: [entry("src", true), entry("package.json"), entry("vite.config.ts")],
  "app/src": [entry("ui", true), entry("App.tsx"), entry("App.css"), entry("main.tsx")],
};

/** The core, as the view asks it: folders, what `purlis` changed, and files found by name. */
function core() {
  mockIPC(
    (cmd, args) => {
      const a = (args ?? {}) as Record<string, unknown>;
      if (cmd === "branch_tree") {
        const folder = String(a.folder ?? "");
        return { entries: folder === "" ? TOP : (UNDER[folder] ?? []), more: 0 };
      }
      if (cmd === "branch_status") {
        const count = a.repo === "purlis" ? 3 : a.repo === "theme-lab" ? 1 : 0;
        return {
          refused: [],
          changes: [],
          folders: count === 0 ? [] : [{ folder: "", mark: "changed", count }],
          more: 0,
          base: "main",
        };
      }
      if (cmd === "find_files") {
        const scope = a.scope as FileScope;
        const repos = scope.kind === "branch" ? [scope.repo] : ["purlis", "theme-lab"];
        return {
          files: repos.map((repo) => ({
            plane: PLANE,
            workspace: "smart-ide",
            repo,
            piece: null,
            path: "app/src/ui/titlebar/titlebar.css",
            matched: [16, 17, 18],
          })),
          branches: repos.length,
          refused: [],
          partial: [],
        };
      }
      return null;
    },
    { shouldMockEvents: true },
  );
}

function stateOf(repos: string[], absent: string[] = []): WorkspaceState {
  const panels: PanelsModel = {
    workspace: "smart-ide",
    repos,
    paths: {},
    absent,
    refused: [],
    todos: [],
    done_todos: [],
    todos_refused: null,
    personas: [],
    persona: null,
    sessions: [],
    contributed: [],
  };
  const branches: Record<string, string> = {
    purlis: "main",
    "theme-lab": "m64/engine",
    "k8s-operator": "main",
  };
  return {
    panels,
    repos: {
      workspace: "smart-ide",
      repos: repos.map((name) => ({ name, branch: branches[name] ?? "main" })),
      cache_refused: null,
    } as unknown as WorkspaceState["repos"],
    pieces: Object.fromEntries(repos.map((repo) => [repo, []])),
    piecesRefused: {},
    reading: false,
  };
}

/** The view in a left side panel, its head the view's title and its actions, as `RegionFrame`
 *  draws it (#3330), and the view under it the one scroll. */
function View({
  repos,
  absent = [],
  part = "explorer",
}: {
  repos: string[];
  absent?: string[];
  /** The Explorer's part: `repos` is the Repos view, where a repo not cloned is listed. */
  part?: ExplorerPart;
}) {
  const [head, setHead] = useState<HTMLElement | null>(null);
  return (
    <Panel variant="side" aria-label="Explorer" role="region">
      <div
        style={{
          display: "flex",
          flexDirection: "column",
          blockSize: "26rem",
          inlineSize: "276px",
        }}
      >
        <div className="side-head" data-side="left">
          <PanelTitle className="side-title">smart-ide</PanelTitle>
          <span className="view-actions" ref={setHead} />
        </div>
        <div className="region-view" style={{ flex: "1 1 0" }}>
          <HeadActions.Provider value={head}>
            <ChatsHere.Provider value={fixedChats(nothingKnown)}>
              <Explorer
                part={part}
                plane={PLANE}
                workspace="smart-ide"
                state={stateOf(repos, absent)}
                chats={[]}
                spot={undefined}
                onPick={() => {}}
                offers={new Map()}
                onPress={() => {}}
                onReadAgain={() => {}}
                onOpenFile={() => {}}
                onSearchContents={() => {}}
              />
            </ChatsHere.Provider>
          </HeadActions.Provider>
        </div>
      </div>
    </Panel>
  );
}

const meta = {
  component: View,
  beforeEach: () => {
    core();
    return () => clearMocks();
  },
} satisfies Meta<typeof View>;

export default meta;
type Story = StoryObj<typeof meta>;

/** Two frames on, so the held top's height is measured and drawn before the picture. */
const settled = () =>
  new Promise((done) => requestAnimationFrame(() => requestAnimationFrame(done)));

/** One repo: its tree, open at once, the repo and its branch beside the title. */
export const OneRepo: Story = {
  args: { repos: ["purlis"] },
  play: async ({ canvas, canvasElement }) => {
    await canvas.findByRole("row", { name: /^README\.md/ });
    await expect(canvasElement.querySelector(".side-head")).toHaveTextContent("purlis · main");
    await expect(canvas.queryByRole("button", { name: /^Files/ })).toBeNull();
    await expect(canvas.getByText(/One repo, so the filter covers all of it/)).toBeVisible();
    await settled();
  },
};

/** Several repos: a closed row each, with its changes and its branch; one opened in place. */
export const SeveralRepos: Story = {
  args: { repos: ["purlis", "theme-lab", "k8s-operator"] },
  play: async ({ canvas, userEvent }) => {
    const purlis = await canvas.findByRole("row", { name: /^purlis/ });
    await waitFor(() => expect(purlis).toHaveAccessibleName(/3 changes/));
    await expect(canvas.getByRole("radio", { name: "Selected: purlis" })).toBeChecked();
    await userEvent.click(purlis);
    await canvas.findByRole("row", { name: /^Cargo\.toml/ });
    (document.activeElement as HTMLElement | null)?.blur();
    await settled();
  },
};

/** The filter in all repos: the files found by name, under each repo's name. */
export const FilterAllRepos: Story = {
  args: { repos: ["purlis", "theme-lab", "k8s-operator"] },
  play: async ({ canvas, userEvent }) => {
    await canvas.findByRole("row", { name: /^purlis/ });
    await userEvent.click(canvas.getByRole("radio", { name: "All repos" }));
    await userEvent.type(canvas.getByRole("searchbox", { name: "Filter files" }), "tit");
    const found = await canvas.findByRole("grid", { name: "Files found" });
    await waitFor(() => expect(found.querySelectorAll(".found-file")).toHaveLength(2));
    await expect(
      canvas.getByRole("button", { name: /Search contents in Workspace/ }),
    ).toBeVisible();
    (document.activeElement as HTMLElement | null)?.blur();
    await settled();
  },
};

/**
 * **A repo the workspace names that is not cloned here** (#3790): in the Repos view, under the
 * cloned repos, in its own section, its name quieter than a cloned repo's (the muted tier, never a fade).
 */
export const ARepoNotClonedHere: Story = {
  args: { repos: ["purlis", "theme-lab"], absent: ["k8s-operator"], part: "repos" },
  play: async ({ canvas }) => {
    const absent = await canvas.findByTestId("absent");
    await expect(absent).toHaveTextContent("Not cloned here");
    await expect(absent).toHaveTextContent("k8s-operator");
    await settled();
  },
};
