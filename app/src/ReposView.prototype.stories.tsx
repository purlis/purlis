// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * PROTOTYPE (#7997, map #7990; never merged). **The one Repos view**, in three variants, at the
 * size of the Panel's left half, on 168 branch folders across two repos. ← and → (or the
 * floating bar) switch variants in every story; the bar's Panel / Tall / Sidebar switch the frame.
 * The edge stories open on variant A and switch the same way.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { FOLDERS, REMOVED_TODAY } from "./ReposView.prototype.fixture";
import type { FolderRow, RepoHead, ReposData, RowState } from "./ReposView.prototype.model";
import { ReposIcon, ReposViewPrototype } from "./ReposView.prototype";
import { ActivityGroup } from "./ui/activity/ActivityItem";

const PURLIS: RepoHead = {
  name: "purlis",
  branch: "main",
  ahead: 0,
  behind: 3,
  ci: "passing",
  uncommitted: 2,
};
const SECOND: RepoHead = {
  name: "purlis-forge-lock-verify-1",
  branch: "main",
  ahead: 0,
  behind: 0,
  ci: "none",
  uncommitted: 0,
};

const base: ReposData = {
  workspace: "smart-ide",
  repos: [PURLIS, SECOND],
  rows: FOLDERS,
  removed: REMOVED_TODAY,
  freeGb: 120,
  diskGb: 926,
  workspaceLive: true,
};

const only = (repo: string) => FOLDERS.filter((r) => r.repo === repo);

/** The fixture with some rows moved to other states. */
function withStates(states: RowState[]): FolderRow[] {
  const picked = FOLDERS.filter((r) => r.repo === "purlis" && r.state === "pr-open").slice(
    0,
    states.length,
  );
  return FOLDERS.map((r) => {
    const at = picked.indexOf(r);
    return at < 0
      ? r
      : { ...r, state: states[at], dirty: states[at] === "behind-dirty" ? 7 : r.dirty };
  });
}

const DATA = {
  full: base,
  pressure: { ...base, freeGb: 19, idleCleared: { folders: 4, gb: 96 } },
  empty: { ...base, repos: [], rows: [], removed: [] },
  noFolders: { ...base, repos: [PURLIS], rows: [], removed: [] },
  oneRepo: { ...base, repos: [PURLIS], rows: only("purlis") },
  failedClone: {
    ...base,
    repos: [
      PURLIS,
      {
        ...SECOND,
        clone: {
          kind: "failed",
          why: "git said “Repository not found” (exit 128). Check the remote's address and your access.",
        },
      },
    ],
    rows: only("purlis"),
  },
  notCloned: {
    ...base,
    repos: [PURLIS, { ...SECOND, clone: { kind: "not-cloned" } }],
    rows: only("purlis"),
  },
  unwired: { ...base, rows: withStates(["unwired"]) },
  everyNeedsYou: { ...base, rows: withStates(["behind-dirty", "unwired"]) },
} satisfies Record<string, ReposData>;

const meta = {
  title: "Prototypes/Repos view (#7997)",
  component: ReposViewPrototype,
  parameters: { layout: "fullscreen" },
  args: { data: DATA.full },
} satisfies Meta<typeof ReposViewPrototype>;

export default meta;
type Story = StoryObj<typeof meta>;

/** **A, grouped list**: repo header, foldable groups, the smart action on each group's header. */
export const AGroupedList: Story = { args: { initial: "A" } };
/** **B, ledger**: sortable columns, state chips, a disk bar per repo. */
export const BLedger: Story = { args: { initial: "B" } };
/** **C, triage**: tiles that count and filter the one list, needs you first. */
export const CTriage: Story = { args: { initial: "C" } };

/** Today's disk: 19 GB free, under pressure. The Repos icon turns amber; the notice says why. */
export const DiskPressure: Story = { args: { data: DATA.pressure } };
/** Two repos and the workspace's own row (the default data, named as the ticket asks). */
export const TwoReposAndTheWorkspace: Story = { args: { data: DATA.full } };
/** One repo. */
export const OneRepo: Story = { args: { data: DATA.oneRepo } };
/** The second repo's clone failed. */
export const FailedClone: Story = { args: { data: DATA.failedClone } };
/** The second repo is in the workspace but not cloned here. */
export const NotCloned: Story = { args: { data: DATA.notCloned } };
/** A real "unwired": the plane carries a layer this folder lacks (the proposal on #7997). */
export const Unwired: Story = { args: { data: DATA.unwired } };
/** Every proposed needs-you state at once: conflict, failed check, merge waiting, behind and
 *  dirty, unwired. */
export const EveryNeedsYouState: Story = { args: { data: DATA.everyNeedsYou } };
/** A workspace with no repos. */
export const Empty: Story = { args: { data: DATA.empty } };
/** A repo with no branch folders. */
export const NoFolders: Story = { args: { data: DATA.noFolders } };
/** Sidebar-sized, as when the view is moved (#2885). */
export const MovedToTheSidebar: Story = { args: { frame: "sidebar" } };

/** **The Repos icon's badge**: nothing, needs you, many, disk pressure alone, and both. */
export const TheBadge: Story = {
  render: () => {
    const cases: [string, ReposData][] = [
      [
        "Nothing needs you",
        {
          ...base,
          rows: FOLDERS.filter(
            (r) => !["conflict", "check-failed", "merge-waiting"].includes(r.state),
          ),
        },
      ],
      ["3 need you", base],
      ["12 need you", { ...base, rows: withStates(Array(9).fill("check-failed") as RowState[]) }],
      [
        "Disk pressure",
        {
          ...DATA.pressure,
          rows: FOLDERS.filter(
            (r) => !["conflict", "check-failed", "merge-waiting"].includes(r.state),
          ),
        },
      ],
      ["3 need you, disk pressure", DATA.pressure],
    ];
    return (
      <div style={{ display: "flex", gap: "var(--space-8)", padding: "var(--space-6)" }}>
        {cases.map(([name, data]) => (
          <figure
            key={name}
            style={{ margin: 0, display: "grid", justifyItems: "center", gap: "var(--space-3)" }}
          >
            <ActivityGroup role="toolbar" aria-label={name} aria-orientation="vertical">
              <ReposIcon data={data} selected={false} />
            </ActivityGroup>
            <figcaption className="ui-text-meta">{name}</figcaption>
          </figure>
        ))}
      </div>
    );
  },
};
