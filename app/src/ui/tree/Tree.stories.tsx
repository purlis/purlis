// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **The tree family** (#2911): a branch's files as a side view draws them (folders that open,
 * twists that lead, a folder still being read), the Changes view's tree that never folds, and a
 * tree whose twists trail, as a settings page's groups. Each is walked by the keyboard, held to
 * axe and compared with its baseline in Chromium and WebKit (`docs/component-stories.md`).
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { useState } from "react";
import { expect, waitFor, within } from "storybook/test";
import { FileCode, FileText, Folder, FolderGit2, GitBranch } from "lucide-react";
import { expectFocusInSight, untilFocused } from "../../stories.testkit";
import { RowLabel, RowLead, RowTrail } from "../list/Row";
import { Tree, TreePending, TreeRow, type Key } from "./Tree";

/** A file's row: its icon and its name. */
function file(id: string, name: string, mark?: string) {
  return (
    <TreeRow
      key={id}
      id={id}
      textValue={name}
      content={
        <>
          <RowLead>
            {name.endsWith(".md") ? (
              <FileText aria-hidden="true" />
            ) : (
              <FileCode aria-hidden="true" />
            )}
          </RowLead>
          <RowLabel>{name}</RowLabel>
          {mark !== undefined && <RowTrail>{mark}</RowTrail>}
        </>
      }
    />
  );
}

/** A folder's row, with what it holds under it. */
function folder(id: string, name: string, children: React.ReactNode) {
  return (
    <TreeRow
      key={id}
      id={id}
      textValue={name}
      content={
        <>
          <RowLead>
            <Folder aria-hidden="true" />
          </RowLead>
          <RowLabel>{name}</RowLabel>
        </>
      }
    >
      {children}
    </TreeRow>
  );
}

/** **A branch's files**: `src` open, `docs` closed and read when it opens, `README.md` changed. */
function Files() {
  const [open, setOpen] = useState<Set<Key>>(new Set(["src", "ui"]));
  const [picked, setPicked] = useState<Set<Key>>(new Set(["tree"]));
  return (
    <div style={{ inlineSize: "16rem" }}>
      <Tree
        aria-label="Files of main"
        size="sm"
        selectionMode="single"
        selectedKeys={picked}
        onSelectionChange={(keys) => setPicked(new Set(keys === "all" ? [] : keys))}
        expandedKeys={open}
        onExpandedChange={setOpen}
      >
        {folder("src", "src", [
          folder("ui", "ui", [file("tree", "Tree.tsx", "M"), file("list", "List.tsx")]),
          file("main", "main.tsx"),
        ])}
        {folder(
          "docs",
          "docs",
          <TreePending>
            <RowLabel>Reading…</RowLabel>
          </TreePending>,
        )}
        {file("readme", "README.md", "M")}
      </Tree>
    </div>
  );
}

const meta = {
  component: Files,
} satisfies Meta<typeof Files>;

export default meta;
type Story = StoryObj<typeof meta>;

/**
 * **A branch's files**: one Tab stop on the file picked; ↑ ↓ walk the rows drawn; → opens a
 * closed folder and moves into an open one; ← closes it and moves to its parent; typing finds a
 * row; a folder still read says so in its place, and the keyboard passes it. The picture is the picked file under the
 * keyboard, its ring inside its wash.
 */
export const BranchFiles: Story = {
  play: async ({ canvas, userEvent }) => {
    const tree = canvas.getByRole("treegrid", { name: "Files of main" });
    const row = (name: string) => within(tree).getByRole("row", { name });
    await userEvent.tab();
    await untilFocused(() => row("Tree.tsx"), "Files: Tab lands on the picked file");
    await userEvent.keyboard("{ArrowLeft}");
    await untilFocused(() => row("ui"), "Files: Left moves to the folder it is in");
    await userEvent.keyboard("{ArrowLeft}");
    await waitFor(() => expect(row("ui")).toHaveAttribute("aria-expanded", "false"));
    await userEvent.keyboard("{ArrowRight}");
    await waitFor(() => expect(row("ui")).toHaveAttribute("aria-expanded", "true"));
    await userEvent.keyboard("{ArrowRight}");
    await untilFocused(() => row("Tree.tsx"), "Files: Right moves into an open folder");
    await userEvent.keyboard("{End}");
    await untilFocused(() => row("README.md"), "Files: End moves to the last row");
    expectFocusInSight();
    await userEvent.keyboard("{ArrowUp}");
    await untilFocused(() => row("docs"), "Files: Up moves to docs");
    await userEvent.keyboard("{ArrowRight}");
    // A folder being read says so in its place, and the keyboard stays on the folder.
    await waitFor(() =>
      expect(canvas.getByText("Reading…").closest('[role="row"]')).toHaveAttribute(
        "aria-level",
        "2",
      ),
    );
    await userEvent.keyboard("{ArrowRight}");
    await untilFocused(() => row("docs"), "Files: Right stays on a folder being read");
    await userEvent.keyboard("{Home}m");
    await untilFocused(() => row("main.tsx"), "Files: typing finds main.tsx");
    await userEvent.keyboard("{ArrowUp}{ArrowUp}");
    await untilFocused(() => row("Tree.tsx"), "Files: Up walks back to the picked file");
  },
};

/** **The Changes view's tree**: a repo and what is true of it, every row always open. */
function Changes() {
  return (
    <div style={{ inlineSize: "18rem" }}>
      <Tree aria-label="Repos of alpha" size="sm" fixed expandedKeys={["repo:svc", "branches:svc"]}>
        <TreeRow
          id="repo:svc"
          textValue="svc main"
          content={
            <>
              <RowLead>
                <FolderGit2 aria-hidden="true" />
              </RowLead>
              <RowLabel>svc</RowLabel>
              <RowTrail>main</RowTrail>
            </>
          }
        >
          <TreeRow
            id="changes:svc"
            textValue="3 changed, changes"
            content={<RowLabel>3 changed</RowLabel>}
          />
          <TreeRow
            id="branches:svc"
            textValue="2 branches"
            content={
              <>
                <RowLead>
                  <GitBranch aria-hidden="true" />
                </RowLead>
                <RowLabel>2 branches</RowLabel>
              </>
            }
          >
            <TreeRow id="piece:one" textValue="one" content={<RowLabel>one</RowLabel>} />
            <TreeRow id="piece:two" textValue="two, unwired" content={<RowLabel>two</RowLabel>} />
          </TreeRow>
          <TreeRow
            id="ci:svc"
            textValue="success · 2m ago, pipeline"
            content={<RowLabel>success · 2m ago</RowLabel>}
          />
        </TreeRow>
      </Tree>
    </div>
  );
}

/**
 * **A tree that never folds** (the Changes view): every row open, no twist and nothing to
 * press; → moves into a row's first child, ← back to its parent, and Enter folds nothing.
 */
export const NeverFolds: Story = {
  render: () => <Changes />,
  play: async ({ canvas, userEvent }) => {
    const tree = canvas.getByRole("treegrid", { name: "Repos of alpha" });
    const row = (name: string) => within(tree).getByRole("row", { name });
    await expect(tree.querySelectorAll("button")).toHaveLength(0);
    await userEvent.tab();
    await untilFocused(() => row("svc main"), "Never folds: Tab lands on the first repo");
    await userEvent.keyboard("{ArrowRight}");
    await untilFocused(() => row("3 changed, changes"), "Never folds: Right moves into it");
    await userEvent.keyboard("{ArrowDown}");
    await untilFocused(() => row("2 branches"), "Never folds: Down to its branches");
    await userEvent.keyboard("{Enter}{ArrowLeft}");
    await untilFocused(() => row("svc main"), "Never folds: Left moves to the parent");
    await expect(row("2 branches")).toHaveAttribute("aria-expanded", "true");
    await userEvent.keyboard("{End}");
    await untilFocused(
      () => row("success · 2m ago, pipeline"),
      "Never folds: End moves to the last row",
    );
  },
};

/** **A tree whose twists trail**: a settings page's groups, where a row is itself a page. */
function Groups() {
  const [open, setOpen] = useState<Set<Key>>(new Set());
  const [page, setPage] = useState("General");
  return (
    <div style={{ inlineSize: "14rem", display: "grid", gap: "var(--space-3)" }}>
      <Tree
        aria-label="Settings"
        size="md"
        twist="trail"
        expandedKeys={open}
        onExpandedChange={setOpen}
        onAction={(key) => setPage(String(key))}
      >
        <TreeRow id="General" textValue="General" content={<RowLabel>General</RowLabel>} />
        <TreeRow id="Saving" textValue="Saving" content={<RowLabel>Saving</RowLabel>}>
          <TreeRow id="Branches" textValue="Branches" content={<RowLabel>Branches</RowLabel>} />
          <TreeRow id="Forges" textValue="Forges" content={<RowLabel>Forges</RowLabel>} />
        </TreeRow>
      </Tree>
      <p role="status" style={{ margin: 0 }}>
        {page}
      </p>
    </div>
  );
}

/**
 * **Twists that trail** (#2799 rule 1): Enter on a group runs its page and never folds it; its
 * twist at the row's end folds it, and so does →. The picture is the open group, its twist
 * turned down.
 */
export const TwistsTrail: Story = {
  render: () => <Groups />,
  play: async ({ canvas, userEvent }) => {
    const tree = canvas.getByRole("treegrid", { name: "Settings" });
    const row = (name: string) => within(tree).getByRole("row", { name });
    await userEvent.tab();
    await userEvent.keyboard("{ArrowDown}");
    await untilFocused(() => row("Saving"), "Twists trail: Down moves to Saving");
    await userEvent.keyboard("{Enter}");
    await waitFor(() => expect(canvas.getByRole("status")).toHaveTextContent("Saving"));
    await expect(row("Saving")).toHaveAttribute("aria-expanded", "false");
    await userEvent.click(within(row("Saving")).getByRole("button", { name: /Expand/ }));
    await waitFor(() => expect(row("Saving")).toHaveAttribute("aria-expanded", "true"));
    await userEvent.keyboard("{ArrowRight}");
    await untilFocused(() => row("Branches"), "Twists trail: Right moves into the group");
  },
};

/** **An empty tree** says why where its rows would be. */
export const Empty: Story = {
  render: () => (
    <div style={{ inlineSize: "16rem" }}>
      <Tree aria-label="Files of main" size="sm" empty={<p style={{ margin: 0 }}>Nothing here</p>}>
        {[]}
      </Tree>
    </div>
  ),
  play: async ({ canvas }) => {
    await expect(canvas.getByText("Nothing here")).toBeVisible();
  },
};
