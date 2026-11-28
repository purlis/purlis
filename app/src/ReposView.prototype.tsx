// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * PROTOTYPE (#7997, map #7990; never merged). **Three variants of the one Repos view**, which
 * replaces the Repos and Changes views, switchable from a floating bar (← and →) and each drawn in
 * a frame the size of the Panel's left half, beside the left activity bar's Repos icon and its
 * needs-you badge. Built from `app/src/ui` and the theme's tokens only. Read-only: every action
 * is a stub that says what it would do in the log under the frame.
 *
 * - **A, grouped list**: each repo's header, then foldable groups (Needs you, Working,
 *   Finished, Dormant) whose smart action stands on the group's header. Rows are title first.
 * - **B, ledger**: a dense table with sortable columns, state chips that filter it, and a disk
 *   bar in each repo's header.
 * - **C, triage**: tiles that count what needs you and what can go, and filter the one list
 *   below them; two-line rows, needs-you first.
 */
import { useCallback, useEffect, useMemo, useState, type ReactNode } from "react";
import {
  ChevronLeft,
  ChevronRight,
  CircleCheck,
  CircleDashed,
  CircleX,
  FolderGit2,
  FolderTree,
  Folders,
  History,
  LoaderCircle,
  MessagesSquare,
  Search,
} from "lucide-react";
import "./ReposView.prototype.css";
import { Button, IconButton, ToggleButton } from "./ui/button/Button";
import {
  ActivityBarFrame,
  ActivityCount,
  ActivityGroup,
  ActivityItem,
} from "./ui/activity/ActivityItem";
import { Panel } from "./ui/panel/Panel";
import { List, ListRow, ListRowSection } from "./ui/list/List";
import { RowBelow, RowLabel, RowLead, RowTrail } from "./ui/list/Row";
import { Twist } from "./ui/disclosure/Disclosure";
import { Tooltip } from "./ui/tooltip/Tooltip";
import { Menu, MenuItem, MenuSeparator, MenuWords } from "./ui/menu/Menu";
import { Meter } from "./ui/meter/Meter";
import { Count } from "./ui/badge/Badge";
import { EmptyState } from "./ui/empty-state/EmptyState";
import {
  Table,
  TableBody,
  TableCell,
  TableColumn,
  TableHeader,
  TableRow,
  type SortDescriptor,
} from "./ui/table/Table";
import { Notice } from "./Notice";
import {
  GROUPS,
  NEEDS_YOU,
  STATES,
  ageSaid,
  byAge,
  canGo,
  diskSaid,
  gbSaid,
  groupOf,
  nameOf,
  pathOf,
  stateSaid,
  sum,
  underPressure,
  type Did,
  type FolderRow,
  type Group,
  type RepoHead,
  type ReposData,
} from "./ReposView.prototype.model";

/* ───────────────────────────── shared bits ───────────────────────────── */

/** **A name cut in the middle**: its start gives way, its last few characters always show. */
export function MiddleName({ text, strong = false }: { text: string; strong?: boolean }) {
  const keep = text.length > 18 ? 10 : 0;
  const head = keep ? text.slice(0, -keep) : text;
  const tail = keep ? text.slice(-keep) : "";
  return (
    <span className="rvp-mid" data-strong={strong || undefined}>
      <span className="sr-only">{text}</span>
      <span className="rvp-mid-head" aria-hidden="true">
        {head}
      </span>
      {tail && (
        <span className="rvp-mid-tail" aria-hidden="true">
          {tail}
        </span>
      )}
    </span>
  );
}

/** **A row's one status icon**, with its tooltip: the state's word, then why. */
export function StatusMark({ row }: { row: FolderRow }) {
  const { icon: Icon, tone, word } = STATES[row.state];
  return (
    <Tooltip tip={`${word}. ${stateSaid(row)}`}>
      <span className="rvp-mark" data-tone={tone} role="img" aria-label={word}>
        <Icon aria-hidden="true" />
      </span>
    </Tooltip>
  );
}

/** The row's tooltip: the full branch and its folder's path. */
const rowTip = (row: FolderRow, workspace: string) => (
  <span className="rvp-tip">
    <span>{nameOf(row)}</span>
    <code>{row.branch}</code>
    <code>{pathOf(row, workspace)}</code>
    {row.buildGb > 0 && (
      <span>
        {gbSaid(row.sizeGb)}, of which build outputs {gbSaid(row.buildGb)}
      </span>
    )}
  </span>
);

/** A row's menu: what one folder offers. Bulk selection is #7999's. */
function rowMenu(row: FolderRow, did: Did) {
  const name = nameOf(row);
  return () => (
    <Menu aria-label={name} onAction={(key) => did(`${String(key)}: ${row.branch}`)}>
      <MenuItem id="Open its files (narrowed file list)" textValue="Open its files">
        <MenuWords said="Open its files" />
      </MenuItem>
      {row.pr !== undefined && (
        <MenuItem id={`Open pull request #${row.pr}`} textValue="Open pull request">
          <MenuWords said={`Open pull request #${row.pr}`} />
        </MenuItem>
      )}
      <MenuItem id="Start a chat in it" textValue="Start a chat in it">
        <MenuWords said="Start a chat in it" />
      </MenuItem>
      <MenuSeparator />
      {row.buildGb > 0 && (
        <MenuItem
          id={`Clear build outputs (${gbSaid(row.buildGb)})`}
          textValue="Clear build outputs"
        >
          <MenuWords said={`Clear build outputs · ${gbSaid(row.buildGb)}`} />
        </MenuItem>
      )}
      <MenuItem id="Put the folder away, keep the branch" textValue="Put the folder away">
        <MenuWords said="Put the folder away" under="Keeps the branch" />
      </MenuItem>
      {row.state === "dormant" && (
        <MenuItem
          id="Delete the branch (asks first)"
          textValue="Delete the branch"
          variant="danger"
        >
          <MenuWords said="Delete the branch…" />
        </MenuItem>
      )}
    </Menu>
  );
}

const CI_MARK = {
  passing: { icon: CircleCheck, tone: "success", said: "Checks pass on main" },
  failing: { icon: CircleX, tone: "failed", said: "A check fails on main" },
  running: { icon: LoaderCircle, tone: "quiet", said: "Checks are running on main" },
  none: { icon: CircleDashed, tone: "quiet", said: "No checks on main" },
} as const;

/** The repo's own checkout, in a line: branch, ahead and behind, CI and what is uncommitted. */
function RepoFacts({ repo }: { repo: RepoHead }) {
  const ci = CI_MARK[repo.ci];
  return (
    <span className="rvp-facts ui-text-meta">
      <span className="rvp-branch">{repo.branch}</span>
      <span aria-label={`${repo.ahead} ahead, ${repo.behind} behind`} role="img">
        <span aria-hidden="true">
          ↑{repo.ahead} ↓{repo.behind}
        </span>
      </span>
      <Tooltip tip={ci.said}>
        <span className="rvp-mark" data-tone={ci.tone} role="img" aria-label={ci.said}>
          <ci.icon aria-hidden="true" />
        </span>
      </Tooltip>
      {repo.uncommitted > 0 && <span>{repo.uncommitted} uncommitted</span>}
    </span>
  );
}

/** What a clone that is not there says, with its one way out. */
function CloneTrouble({ repo, did }: { repo: RepoHead; did: Did }) {
  if (repo.clone?.kind === "not-cloned")
    return (
      <Notice
        cause={`repo-not-cloned:${repo.name}`}
        tone="news"
        fixes={[{ label: "Clone", onPress: () => did(`Clone ${repo.name}`) }]}
      >
        <code>{repo.name}</code> is in this workspace but not cloned on this machine.
      </Notice>
    );
  if (repo.clone?.kind === "failed")
    return (
      <Notice
        cause={`repo-clone-failed:${repo.name}`}
        tone="trouble"
        fixes={[{ label: "Clone again", onPress: () => did(`Clone ${repo.name} again`) }]}
      >
        Cloning <code>{repo.name}</code> failed: {repo.clone.why}
      </Notice>
    );
  return null;
}

/** The disk notices: pressure, and what the idle cleanup cleared. */
function DiskNotices({ data, did }: { data: ReposData; did: Did }) {
  if (!underPressure(data)) return null;
  return (
    <Notice
      cause="disk-pressure"
      tone="attention"
      fixes={[
        {
          label: `Clear ${canGo(data.rows).length} that can go`,
          onPress: () => did("Clear every finished folder and put away every dormant one"),
        },
      ]}
    >
      {gbSaid(data.freeGb)} free on this disk.{" "}
      {data.idleCleared
        ? `Cleared target/ in ${data.idleCleared.folders} idle folders, ${gbSaid(data.idleCleared.gb)}. `
        : ""}
      New task folders wait below 15 GB.
    </Notice>
  );
}

function rowsOf(data: ReposData, repo: string) {
  return data.rows.filter((r) => r.repo === repo);
}

function groupRows(rows: readonly FolderRow[], group: Group) {
  return rows.filter((r) => groupOf(r.state) === group).sort(byAge);
}

/** What a group's header offers: the smart action, or nothing. */
function smartAction(group: Group, rows: readonly FolderRow[], did: Did) {
  if (group === "finished" && rows.length)
    return (
      <Button
        size="sm"
        variant="ghost"
        tooltip="Removes every finished folder no chat holds. Landed branches are deleted; unlanded ones are kept."
        onPress={() =>
          did(`Clear ${rows.length} finished · ${gbSaid(sum(rows))} (shows #5212's list first)`)
        }
      >
        Clear {rows.length} · {gbSaid(sum(rows))}
      </Button>
    );
  if (group === "dormant" && rows.length)
    return (
      <Button
        size="sm"
        variant="ghost"
        tooltip="Removes the folders and keeps every branch."
        onPress={() =>
          did(`Put away ${rows.length} dormant · ${gbSaid(sum(rows))} (branches kept)`)
        }
      >
        Put away {rows.length} · {gbSaid(sum(rows))}
      </Button>
    );
  return null;
}

/* ─────────────────────────────── variant A ─────────────────────────────── */

const A_DEFAULT_OPEN: Record<Group | "removed", boolean> = {
  needs: true,
  working: true,
  finished: false,
  dormant: false,
  removed: false,
};

/** A group's header: its twist and name and count, and its smart action at the end. */
function GroupHead({
  title,
  count,
  gb,
  open,
  onOpen,
  tone,
  action,
}: {
  title: string;
  count: number;
  gb?: number;
  open: boolean;
  onOpen: (open: boolean) => void;
  tone?: "waiting";
  action?: ReactNode;
}) {
  return (
    <div className="rvp-group-head" data-tone={tone}>
      <Button
        size="sm"
        variant="quiet"
        aria-expanded={open}
        icon={<Twist open={open} />}
        onPress={() => onOpen(!open)}
      >
        {title}
        <span className="rvp-count">
          {count}
          {gb !== undefined && gb > 0 ? ` · ${gbSaid(gb)}` : ""}
        </span>
      </Button>
      {action}
    </div>
  );
}

function VariantARepo({ data, repo, did }: { data: ReposData; repo: RepoHead; did: Did }) {
  const rows = rowsOf(data, repo.name);
  const [open, setOpen] = useState(A_DEFAULT_OPEN);
  const removed = data.removed.filter((r) => r.repo === repo.name);
  return (
    <section className="rvp-repo" aria-label={repo.name}>
      <header className="rvp-repo-head">
        <FolderGit2 aria-hidden="true" className="rvp-repo-icon" />
        <span className="ui-text-body ui-text-strong">{repo.name}</span>
        {!repo.clone && <RepoFacts repo={repo} />}
        {!repo.clone && <span className="rvp-disk ui-text-meta">{diskSaid(rows)}</span>}
      </header>
      <CloneTrouble repo={repo} did={did} />
      {!repo.clone && rows.length === 0 && (
        <p className="rvp-none ui-text-secondary">
          No branch folders. Chats cut one as they start.
        </p>
      )}
      {GROUPS.map(({ id, title }) => {
        const these = groupRows(rows, id);
        if (these.length === 0) return null;
        return (
          <div key={id} className="rvp-group">
            <GroupHead
              title={title}
              count={these.length}
              gb={id === "finished" || id === "dormant" ? sum(these) : undefined}
              open={open[id]}
              onOpen={(o) => setOpen({ ...open, [id]: o })}
              tone={id === "needs" ? "waiting" : undefined}
              action={smartAction(id, these, did)}
            />
            {open[id] && (
              <List
                aria-label={`${repo.name}: ${title}`}
                size="sm"
                onAction={(key) => did(`Open ${String(key)} in the narrowed file list`)}
              >
                {these.map((row) => (
                  <ListRow
                    key={row.id}
                    id={row.id}
                    textValue={nameOf(row)}
                    tip={rowTip(row, data.workspace)}
                    menu={rowMenu(row, did)}
                  >
                    <RowLead>
                      <StatusMark row={row} />
                    </RowLead>
                    <RowLabel>
                      <MiddleName text={nameOf(row)} />
                    </RowLabel>
                    <RowTrail>
                      <span className="rvp-trail ui-text-meta">
                        <span className="rvp-owner">{row.owner.label}</span>
                        <span className="rvp-num">{gbSaid(row.sizeGb)}</span>
                        <span className="rvp-age">{ageSaid(row.ageMin)}</span>
                      </span>
                    </RowTrail>
                  </ListRow>
                ))}
              </List>
            )}
          </div>
        );
      })}
      {removed.length > 0 && (
        <div className="rvp-group">
          <GroupHead
            title="Removed today"
            count={removed.length}
            gb={removed.reduce((a, r) => a + r.gb, 0)}
            open={open.removed}
            onOpen={(o) => setOpen({ ...open, removed: o })}
          />
          {open.removed && <RemovedList data={data} repo={repo.name} />}
        </div>
      )}
    </section>
  );
}

/** "Removed today": one line per folder that left by itself, newest first. */
function RemovedList({ data, repo }: { data: ReposData; repo?: string }) {
  const lines = data.removed.filter((r) => repo === undefined || r.repo === repo).reverse();
  return (
    <ol className="rvp-removed ui-text-meta" aria-label="Removed today">
      {lines.map((r) => (
        <li key={r.branch}>
          <span className="rvp-num">{r.at}</span>
          <MiddleName text={r.branch} />
          <span>
            {r.why} #{r.pr}
          </span>
          <span className="rvp-num">{gbSaid(r.gb)}</span>
        </li>
      ))}
    </ol>
  );
}

function WorkspaceLine({ data, did }: { data: ReposData; did: Did }) {
  return (
    <List
      aria-label="The workspace"
      size="sm"
      onAction={() => did("Open the whole workspace's files")}
    >
      <ListRow
        id="workspace"
        textValue={data.workspace}
        tip="Its own folder: the plane's files, its memory and its todos"
      >
        <RowLead>
          <Folders aria-hidden="true" />
        </RowLead>
        <RowLabel>{data.workspace}</RowLabel>
        <RowTrail>
          <span className="rvp-trail ui-text-meta">
            <span className="rvp-owner">the workspace itself</span>
            {data.rows.length > 0 && <span>{diskSaid(data.rows)}</span>}
          </span>
        </RowTrail>
      </ListRow>
    </List>
  );
}

export function VariantA({ data, did }: { data: ReposData; did: Did }) {
  if (data.repos.length === 0) return <NoRepos did={did} />;
  return (
    <div className="rvp-view rvp-a">
      <WorkspaceLine data={data} did={did} />
      <DiskNotices data={data} did={did} />
      {data.repos.map((repo) => (
        <VariantARepo key={repo.name} data={data} repo={repo} did={did} />
      ))}
    </div>
  );
}

function NoRepos({ did }: { did: Did }) {
  return (
    <div className="rvp-view">
      <EmptyState
        variant="panel"
        mark={FolderGit2}
        headline="No repos in this workspace"
        body="Add a repo and its branch folders are listed here."
        action={
          <Button size="sm" onPress={() => did("Open Add a repo")}>
            Add a repo…
          </Button>
        }
      />
    </div>
  );
}

/* ─────────────────────────────── variant B ─────────────────────────────── */

type Chip = Group | "all";

export function VariantB({ data, did }: { data: ReposData; did: Did }) {
  const [chip, setChip] = useState<Chip>("all");
  const [sort, setSort] = useState<SortDescriptor>({ column: "state", direction: "ascending" });
  const counts = useMemo(() => {
    const c: Record<Group, number> = { needs: 0, working: 0, finished: 0, dormant: 0 };
    for (const r of data.rows) c[groupOf(r.state)] += 1;
    return c;
  }, [data.rows]);
  const shown = useMemo(() => {
    const order: Record<Group, number> = { needs: 0, working: 1, finished: 2, dormant: 3 };
    const these = data.rows.filter((r) => chip === "all" || groupOf(r.state) === chip);
    const key = (r: FolderRow): number | string => {
      switch (sort.column) {
        case "name":
          return nameOf(r).toLowerCase();
        case "owner":
          return r.owner.label;
        case "state":
          return order[groupOf(r.state)] * 1e6 + r.ageMin;
        case "ab":
          return r.behind;
        case "size":
          return r.sizeGb;
        default:
          return r.ageMin;
      }
    };
    const dir = sort.direction === "ascending" ? 1 : -1;
    return [...these].sort((a, b) => (key(a) < key(b) ? -dir : key(a) > key(b) ? dir : 0));
  }, [data.rows, chip, sort]);
  if (data.repos.length === 0) return <NoRepos did={did} />;
  const used = sum(data.rows);
  return (
    <div className="rvp-view rvp-b">
      <div className="rvp-b-heads">
        {data.repos.map((repo) => {
          const rows = rowsOf(data, repo.name);
          return (
            <div key={repo.name} className="rvp-b-head">
              <FolderGit2 aria-hidden="true" className="rvp-repo-icon" />
              <span className="ui-text-body ui-text-strong rvp-b-name">{repo.name}</span>
              {repo.clone ? (
                <span className="ui-text-meta">
                  {repo.clone.kind === "failed" ? "clone failed" : "not cloned"}
                </span>
              ) : (
                <RepoFacts repo={repo} />
              )}
              {!repo.clone && (
                <Meter
                  className="rvp-b-meter"
                  label={<span className="sr-only">{repo.name} on disk</span>}
                  value={sum(rows)}
                  maxValue={data.diskGb}
                  valueLabel={diskSaid(rows)}
                />
              )}
            </div>
          );
        })}
        {data.repos.map((repo) => (
          <CloneTrouble key={repo.name} repo={repo} did={did} />
        ))}
        <div className="rvp-b-disk ui-text-meta">
          <span>
            {data.workspace}: {gbSaid(used)} of {gbSaid(data.diskGb)} disk ·{" "}
            <span data-tone={underPressure(data) ? "waiting" : undefined} className="rvp-free">
              {gbSaid(data.freeGb)} free
            </span>
          </span>
          <Button
            size="sm"
            variant="ghost"
            icon={<History aria-hidden="true" />}
            tooltip={<RemovedList data={data} />}
            onPress={() => did("Open Removed today as a popover (9 lines)")}
          >
            Removed today {data.removed.length}
          </Button>
        </div>
      </div>
      <DiskNotices data={data} did={did} />
      <div className="rvp-b-chips" role="toolbar" aria-label="Show">
        <ToggleButton size="sm" isSelected={chip === "all"} onChange={() => setChip("all")}>
          All {data.rows.length}
        </ToggleButton>
        {GROUPS.map(({ id, title }) =>
          counts[id] ? (
            <ToggleButton
              key={id}
              size="sm"
              isSelected={chip === id}
              onChange={() => setChip(chip === id ? "all" : id)}
            >
              {title} {counts[id]}
            </ToggleButton>
          ) : null,
        )}
        <span className="rvp-spacer" />
        {(chip === "finished" || chip === "dormant") &&
          smartAction(chip, groupRows(data.rows, chip), did)}
      </div>
      <Table
        aria-label="Branch folders"
        sortDescriptor={sort}
        onSortChange={setSort}
        rowMenus
        className="rvp-b-table"
        onRowAction={(key) => did(`Open ${String(key)} in the narrowed file list`)}
      >
        <TableHeader>
          <TableColumn id="state" allowsSorting width={36} hideName>
            State
          </TableColumn>
          <TableColumn id="name" isRowHeader allowsSorting cut>
            Name
          </TableColumn>
          <TableColumn id="owner" allowsSorting width={104} cut>
            Owner
          </TableColumn>
          <TableColumn id="ab" allowsSorting align="end" width={72}>
            ↑↓
          </TableColumn>
          <TableColumn id="size" allowsSorting align="end" width={64}>
            Size
          </TableColumn>
          <TableColumn id="age" allowsSorting align="end" width={48}>
            Age
          </TableColumn>
        </TableHeader>
        <TableBody empty={<p className="rvp-none">No folder is {chip}.</p>}>
          {shown.map((row) => (
            <TableRow key={row.id} id={row.id} textValue={nameOf(row)} menu={rowMenu(row, did)}>
              <TableCell>
                <StatusMark row={row} />
              </TableCell>
              <TableCell>
                <Tooltip tip={rowTip(row, data.workspace)}>
                  <span className="rvp-b-cellname">
                    <MiddleName text={nameOf(row)} />
                    {data.repos.length > 1 && row.repo !== data.repos[0].name && (
                      <Count>{row.repo}</Count>
                    )}
                  </span>
                </Tooltip>
              </TableCell>
              <TableCell>{row.owner.label}</TableCell>
              <TableCell align="end">
                ↑{row.ahead} ↓{row.behind}
              </TableCell>
              <TableCell align="end">{gbSaid(row.sizeGb)}</TableCell>
              <TableCell align="end">{ageSaid(row.ageMin)}</TableCell>
            </TableRow>
          ))}
        </TableBody>
      </Table>
    </div>
  );
}

/* ─────────────────────────────── variant C ─────────────────────────────── */

type Tile = "needs" | "finished" | "dormant" | "all" | "removed";

export function VariantC({ data, did }: { data: ReposData; did: Did }) {
  const [tile, setTile] = useState<Tile>("all");
  if (data.repos.length === 0) return <NoRepos did={did} />;
  const needs = groupRows(data.rows, "needs");
  const finished = groupRows(data.rows, "finished");
  const dormant = groupRows(data.rows, "dormant");
  const working = groupRows(data.rows, "working");
  const list =
    tile === "needs"
      ? needs
      : tile === "finished"
        ? finished
        : tile === "dormant"
          ? dormant
          : [...needs, ...working, ...finished, ...dormant];
  const pick = (t: Tile) => setTile(tile === t ? "all" : t);
  const tiles: { id: Tile; big: string; small: string; tone?: "waiting" }[] = [
    {
      id: "needs",
      big: String(needs.length),
      small: "need you",
      tone: needs.length ? "waiting" : undefined,
    },
    { id: "finished", big: String(finished.length), small: `finished · ${gbSaid(sum(finished))}` },
    { id: "dormant", big: String(dormant.length), small: `dormant · ${gbSaid(sum(dormant))}` },
    { id: "all", big: gbSaid(sum(data.rows)), small: `in ${data.rows.length} folders` },
  ];
  return (
    <div className="rvp-view rvp-c">
      <div className="rvp-c-repos">
        {data.repos.map((repo) => (
          <div key={repo.name} className="rvp-c-repo">
            <FolderGit2 aria-hidden="true" className="rvp-repo-icon" />
            <span className="ui-text-body ui-text-strong">{repo.name}</span>
            {repo.clone ? null : <RepoFacts repo={repo} />}
            {!repo.clone && (
              <span className="rvp-disk ui-text-meta">{diskSaid(rowsOf(data, repo.name))}</span>
            )}
          </div>
        ))}
        {data.repos.map((repo) => (
          <CloneTrouble key={repo.name} repo={repo} did={did} />
        ))}
      </div>
      <DiskNotices data={data} did={did} />
      {data.rows.length > 0 && (
        <div className="rvp-c-tiles" role="toolbar" aria-label="Show">
          {tiles.map((t) => (
            <ToggleButton
              key={t.id}
              size="lg"
              isSelected={tile === t.id}
              onChange={() => pick(t.id)}
              className="rvp-c-tile"
              data-tone={t.tone}
            >
              <span className="rvp-c-big">{t.big}</span>
              <span className="rvp-c-small">{t.small}</span>
            </ToggleButton>
          ))}
        </div>
      )}
      <div className="rvp-c-bar">
        <span className="ui-text-meta">
          {tile === "all" ? "Everything, needs you first" : tiles.find((t) => t.id === tile)?.small}
        </span>
        <span className="rvp-spacer" />
        {(tile === "finished" || tile === "dormant") && smartAction(tile, list, did)}
        {data.removed.length > 0 && (
          <ToggleButton
            size="sm"
            variant="ghost"
            isSelected={tile === "removed"}
            onChange={() => pick("removed")}
          >
            {data.removed.length} removed today
          </ToggleButton>
        )}
      </div>
      {tile === "removed" ? (
        <RemovedList data={data} />
      ) : (
        <List
          aria-label="Branch folders"
          size="md"
          onAction={(key) => did(`Open ${String(key)} in the narrowed file list`)}
        >
          {tile === "all"
            ? GROUPS.map(({ id, title }) => {
                const these = groupRows(data.rows, id);
                return these.length ? (
                  <ListRowSection key={id} id={id} title={title} count={these.length}>
                    {these.map((row) => cRow(row, data, did))}
                  </ListRowSection>
                ) : null;
              })
            : list.map((row) => cRow(row, data, did))}
        </List>
      )}
    </div>
  );
}

function cRow(row: FolderRow, data: ReposData, did: Did) {
  const two = data.repos.length > 1;
  return (
    <ListRow
      key={row.id}
      id={row.id}
      textValue={nameOf(row)}
      tip={rowTip(row, data.workspace)}
      menu={rowMenu(row, did)}
    >
      <RowLead>
        <StatusMark row={row} />
      </RowLead>
      <RowLabel>
        <MiddleName text={nameOf(row)} />
      </RowLabel>
      <RowTrail>
        <span className="rvp-trail ui-text-meta">
          <span className="rvp-num">{gbSaid(row.sizeGb)}</span>
          <span className="rvp-age">{ageSaid(row.ageMin)}</span>
        </span>
      </RowTrail>
      <RowBelow clamp={1}>
        {NEEDS_YOU.has(row.state) ? STATES[row.state].word + " · " : ""}
        {row.owner.label}
        {two ? ` · ${row.repo}` : ""}
      </RowBelow>
    </ListRow>
  );
}

/* ─────────────────────────── the frame and the bar ─────────────────────────── */

export const VARIANTS = [
  { key: "A", name: "Grouped list", View: VariantA },
  { key: "B", name: "Ledger", View: VariantB },
  { key: "C", name: "Triage", View: VariantC },
] as const;
export type VariantKey = (typeof VARIANTS)[number]["key"];

export const FRAMES = {
  panel: { name: "Panel, left half (default 30%)", inline: 760, block: 260 },
  tall: { name: "Panel raised to 55%", inline: 760, block: 470 },
  sidebar: { name: "Moved to the left sidebar", inline: 300, block: 720 },
} as const;
export type FrameKey = keyof typeof FRAMES;

/** **The Repos icon** in the left activity bar's bottom group, its badge the needs-you count,
 *  its mark amber under disk pressure. */
export function ReposIcon({ data, selected = true }: { data: ReposData; selected?: boolean }) {
  const needs = data.rows.filter((r) => NEEDS_YOU.has(r.state)).length;
  const pressure = underPressure(data);
  const said = [
    needs ? `${needs} ${needs === 1 ? "branch needs" : "branches need"} you` : "",
    pressure ? `${gbSaid(data.freeGb)} free on this disk` : "",
  ]
    .filter(Boolean)
    .join(", ");
  return (
    <ActivityItem
      variant="bar"
      selected={selected}
      aria-label="Repos"
      aria-pressed={selected}
      aria-describedby={said ? "repos-badge" : undefined}
      tip={said || "Repos and their branch folders"}
      icon={
        <span className="rvp-repos-icon" data-pressure={pressure || undefined}>
          <FolderGit2 />
        </span>
      }
      count={
        said ? (
          <span id="repos-badge" aria-hidden="true">
            {needs > 0 ? (
              <ActivityCount count={needs} said={said} tone="needs-you" />
            ) : (
              <span className="sr-only">{said}</span>
            )}
          </span>
        ) : undefined
      }
    />
  );
}

function LeftBar({ data }: { data: ReposData }) {
  return (
    <ActivityBarFrame
      variant="bar"
      data-side="left"
      top={
        <ActivityGroup role="toolbar" aria-label="Views" aria-orientation="vertical">
          {[
            { n: "Chats", I: MessagesSquare },
            { n: "Explorer", I: FolderTree },
            { n: "Search", I: Search },
          ].map(({ n, I }) => (
            <ActivityItem key={n} variant="bar" selected={false} aria-label={n} icon={<I />} />
          ))}
        </ActivityGroup>
      }
      bottom={
        <ActivityGroup role="toolbar" aria-label="Bottom panel" aria-orientation="vertical">
          <ReposIcon data={data} />
        </ActivityGroup>
      }
    />
  );
}

/** The log of what the stubs would have done. */
function useDid() {
  const [log, setLog] = useState<string[]>([]);
  const did = useCallback<Did>((what) => {
    console.info(`[repos-view prototype] would: ${what}`);
    setLog((was) => [what, ...was].slice(0, 4));
  }, []);
  return { log, did };
}

const readVariant = (): VariantKey => {
  const v = new URLSearchParams(window.location.search).get("variant");
  return v === "B" || v === "C" ? v : "A";
};

/** **The prototype**: the frame, the view in the chosen variant, the log and the floating bar. */
export function ReposViewPrototype({
  data,
  initial,
  frame: initialFrame = "panel",
}: {
  data: ReposData;
  initial?: VariantKey;
  frame?: FrameKey;
}) {
  const [variant, setVariant] = useState<VariantKey>(() => initial ?? readVariant());
  const [frame, setFrame] = useState<FrameKey>(initialFrame);
  const { log, did } = useDid();
  const at = VARIANTS.findIndex((v) => v.key === variant);
  const go = useCallback(
    (step: number) => {
      const next = VARIANTS[(at + step + VARIANTS.length) % VARIANTS.length].key;
      setVariant(next);
      const url = new URL(window.location.href);
      url.searchParams.set("variant", next);
      window.history.replaceState(null, "", url);
    },
    [at],
  );
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key !== "ArrowLeft" && event.key !== "ArrowRight") return;
      const t = event.target as HTMLElement | null;
      // Arrows inside a field, a list, a table or a toolbar are theirs.
      if (
        t?.closest(
          'input, textarea, [contenteditable], [role="grid"], [role="treegrid"], [role="listbox"], [role="toolbar"], [role="menu"]',
        )
      )
        return;
      event.preventDefault();
      go(event.key === "ArrowLeft" ? -1 : 1);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [go]);
  const { View, name } = VARIANTS[at];
  const size = FRAMES[frame];
  return (
    <div className="rvp-stage">
      <div className="rvp-window">
        <LeftBar data={data} />
        <div style={{ inlineSize: size.inline, blockSize: size.block }} className="rvp-frame">
          <Panel variant={frame === "sidebar" ? "side" : "bottom"}>
            <div className="rvp-scroll" role="region" aria-label="Repos" tabIndex={-1}>
              <View data={data} did={did} />
            </div>
          </Panel>
        </div>
      </div>
      <div className="rvp-log ui-text-meta" role="status">
        {log.length ? (
          log.map((l, i) => <div key={i}>Would: {l}</div>)
        ) : (
          <div>Actions are stubs: what one would do is said here.</div>
        )}
      </div>
      <div className="rvp-switcher" role="group" aria-label="Prototype variants">
        <IconButton
          size="sm"
          aria-label="Previous variant"
          tooltip="←"
          icon={<ChevronLeft />}
          onPress={() => go(-1)}
        />
        <span className="rvp-switcher-label">
          {variant} ({name})
        </span>
        <IconButton
          size="sm"
          aria-label="Next variant"
          tooltip="→"
          icon={<ChevronRight />}
          onPress={() => go(1)}
        />
        <span className="rvp-switcher-sep" />
        {(Object.keys(FRAMES) as FrameKey[]).map((k) => (
          <ToggleButton
            key={k}
            size="sm"
            variant="ghost"
            isSelected={frame === k}
            onChange={() => setFrame(k)}
            tooltip={FRAMES[k].name}
          >
            {k === "panel" ? "Panel" : k === "tall" ? "Tall" : "Sidebar"}
          </ToggleButton>
        ))}
      </div>
    </div>
  );
}
