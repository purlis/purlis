// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useRef, useState, type RefObject } from "react";
import { X } from "lucide-react";
import {
  commands,
  type DeletedSkill,
  type SkillGrant,
  type SkillLevel,
  type SkillPinner,
  type SkillSetAct,
  type SkillSetMember,
  type StudioLevel,
  type StudioSkill,
} from "./bindings";
import { counted } from "./dispatches";
import { Choice, SettingRow } from "./settings/components";
import { whoGets } from "./SkillChangeDialogs";
import { GrantPicker } from "./SkillLevelPickers";
import { useRead } from "./SkillRead";
import { SetReviewPage, memberKey, setVerb, type Stays } from "./SkillSetReview";
import { NAME_TAKEN_STAYS } from "./SkillStudioDeleted";
import { MAKE_ROOM_ACTS } from "./makeRoom";
import { levelKey, levelOf, sameLevel } from "./skillChanges";
import type { StudioScope } from "./skillStudio";
import { Badge } from "./ui/badge/Badge";
import { Button, ButtonRow } from "./ui/button/Button";
import { EmptyState } from "./ui/empty-state/EmptyState";
import { List, ListRow } from "./ui/list/List";
import { RowBelow, RowLabel, RowTrail } from "./ui/list/Row";
import { ReviewPage } from "./ui/review-page/ReviewPage";
import { ViewPageHeader } from "./ui/panel/ViewFrame";

/**
 * **The Library's Select mode** (#5332; `prototype/library-select`, variant B, the selection
 * page): Select puts a box on each row, and the page beside the list becomes the selection: how
 * many are chosen and how many of them the list does not show, every action with how many it
 * reaches, and the names, those not shown first. An action's review takes that page's place
 * (`SetReviewPage`, the one review #5331 made), and its change is one commit with one Undo, after
 * which Select ends.
 *
 * - **Selecting** is the list family's: a click or Space ticks a row, Shift-click and Shift+↑ ↓ a
 *   range, Esc leaves. ⌘A takes every skill the search and the filters keep, drawn or not: the
 *   Library draws its rows a screen at a time (#4351). A selection stays across searches.
 * - **Built-in skills can be chosen**, and every action leaves them as they are and says so. A
 *   built-in folded away is one of those not shown.
 * - **Off, it costs nothing**: the Library draws exactly what it drew before, and nothing here
 *   is read until Select is pressed.
 *
 * {@link SELECT_ACTS} is the one list of what can be done, in its order: Grade's review and its
 * grades running (#5356) are `SkillLibraryGrade.tsx`'s, and Review updates (#5354) joins it as a
 * row of its own. **Make room** (#5355) is this mode opened least used first, offering only
 * `MAKE_ROOM_ACTS` of it (`makeRoom.ts`).
 */

/** **What Select can do**: a change to the set (#5331's), or Grade (#5356), which changes no
 *  skill. */
export type SelectAct = SkillSetAct | "grade";

/** **What Select can do to the skills chosen**, in the page's order, each with the line under
 *  it. */
export const SELECT_ACTS: readonly { act: SelectAct; label: string; says: string }[] = [
  { act: "turn-off", label: "Turn off", says: "Chats started after this don't get them." },
  { act: "turn-on", label: "Turn on", says: "Chats started after this get them again." },
  { act: "pin", label: "Pin", says: "Always in the chat's list, whatever the budget." },
  { act: "unpin", label: "Unpin", says: "The router decides again." },
  { act: "archive", label: "Archive", says: "They never load until they are restored." },
  { act: "unarchive", label: "Unarchive", says: "They load again." },
  { act: "move", label: "Move to another level", says: "Skills keep their history and grades." },
  { act: "grade", label: "Grade", says: "A few at a time, under a limit you set." },
  { act: "export", label: "Export", says: "A zip or a folder." },
  { act: "delete", label: "Delete", says: "They stay in Deleted for 30 days." },
];

/** What a press on Select keeps, while it is on: the rows chosen, by their row's id, and what is
 *  being done to them. */
export type Selecting = {
  keys: ReadonlySet<string>;
  /** The list shows only what is chosen: "3 not shown" was pressed. */
  only: boolean;
  /** The action whose review is open in the selection page's place. */
  acting?: SelectAct;
  /** **Make room** (#5355): opened from an over budget, least used first, with only
   *  {@link MAKE_ROOM_ACTS}. */
  room?: boolean;
};

/** **What an action reaches of those chosen**: the skills it changes, as a set's members, and
 *  each it leaves as it is, with why. */
export type Reach = {
  will: readonly SkillSetMember[];
  stays: readonly Stays[];
};

/** Who the change is for: the workspace a turn is in and whose pins a pin changes. */
export type SelectFor = {
  scope: StudioScope;
  workspace: string | null;
  pinner: SkillPinner | null;
};

/** Whether `by` is among `skill`'s pinners as written. */
function pinnedBy(skill: StudioSkill, by: SkillPinner): boolean {
  return skill.pinned_by.some((one) => one.at === by.at && one.name === by.name);
}

/** Why `skill` stays as it is under `act`, or `null` where `act` changes it. */
function staysWhy(act: SelectAct, skill: StudioSkill, at: SelectFor): string | null {
  if (skill.level === "built-in")
    return act === "export"
      ? "Built-in skills come with purlis, so there is nothing to export."
      : act === "grade"
        ? "Built-in skills come with purlis, so they are not graded here."
        : "Built-in skills can't be changed.";
  if (levelOf(skill, at.scope) === null)
    return act === "grade"
      ? "It comes from outside the project, so it is not graded here."
      : "It comes from outside the project, so its program changes it.";
  switch (act) {
    case "turn-off":
    case "turn-on":
      if (skill.level !== "project") return "Only a project skill turns off or on in a workspace.";
      if (act === "turn-off" && skill.turned_off) return "Off already.";
      if (act === "turn-on" && !skill.turned_off) return "On already.";
      if (skill.not_granted !== null) return skill.not_granted;
      return null;
    case "pin":
      return at.pinner !== null && pinnedBy(skill, at.pinner) ? "Pinned already." : null;
    case "unpin":
      return at.pinner !== null && !pinnedBy(skill, at.pinner) ? "Not pinned." : null;
    case "archive":
      return skill.archived ? "Archived already." : null;
    case "unarchive":
      return skill.archived ? null : "Not archived.";
    default:
      return null;
  }
}

/** **What `act` reaches of `chosen`**: those it changes, and those it leaves, with why. */
export function reachOf(act: SelectAct, chosen: readonly StudioSkill[], at: SelectFor): Reach {
  const will: SkillSetMember[] = [];
  const stays: Stays[] = [];
  for (const skill of chosen) {
    const why = staysWhy(act, skill, at);
    const level = levelOf(skill, at.scope);
    if (why === null && level !== null) will.push({ level, name: skill.name, label: skill.label });
    else stays.push({ name: skill.name, label: skill.label, why: why ?? "" });
  }
  return { will, stays };
}

/** Whether `act` can be asked at all from where the studio is: a turn needs a workspace, a pin a
 *  workspace or a persona. */
function askable(act: SelectAct, at: SelectFor): boolean {
  if (act === "turn-off" || act === "turn-on") return at.workspace !== null;
  if (act === "pin" || act === "unpin") return at.pinner !== null;
  return true;
}

/**
 * **⌘A while Select is on** (Ctrl+A off macOS): every skill the search and filters keep, drawn
 * or not, wherever the keyboard is in the studio but a field, where it selects the text. Drawn
 * only while Select is on, so it listens to nothing otherwise.
 */
export function SelectAllKeys({
  within,
  onAll,
}: {
  within: RefObject<HTMLElement | null>;
  onAll: () => void;
}) {
  const latest = useRef(onAll);
  useEffect(() => {
    latest.current = onAll;
  });
  useEffect(() => {
    const heard = (event: KeyboardEvent) => {
      if (!(event.metaKey || event.ctrlKey) || event.altKey || event.key.toLowerCase() !== "a")
        return;
      const at = event.target;
      if (!(at instanceof HTMLElement) || within.current?.contains(at) !== true) return;
      if (at.closest('input, textarea, select, [contenteditable="true"]') !== null) return;
      event.preventDefault();
      event.stopPropagation();
      latest.current();
    };
    document.addEventListener("keydown", heard, true);
    return () => document.removeEventListener("keydown", heard, true);
  }, [within]);
  return null;
}

/** "14 selected", or "Nothing selected". */
export function selectedSaid(count: number): string {
  return count === 0 ? "Nothing selected" : `${count} selected`;
}

/** **A name chosen, as a button that takes it out** of the selection. */
function NameOut({ name, label, onOut }: { name: string; label: string; onOut: () => void }) {
  return (
    <Button
      size="sm"
      variant="ghost"
      icon={<X aria-hidden="true" />}
      aria-label={`Take ${name} (${label}) out`}
      data-testid={`select-name-${name}`}
      onPress={onOut}
    >
      {name}
    </Button>
  );
}

/** The names a page lists, under their heading. */
function Names({
  title,
  skills,
  testid,
  onOut,
}: {
  title: string;
  skills: readonly { key: string; name: string; label: string }[];
  testid: string;
  onOut: (key: string) => void;
}) {
  return (
    <section aria-label={title} className="skill-select-names" data-testid={testid}>
      <h3 className="ui-text-small ui-text-strong">{title}</h3>
      <ButtonRow size="sm" wraps>
        {skills.map((one) => (
          <NameOut key={one.key} name={one.name} label={one.label} onOut={() => onOut(one.key)} />
        ))}
      </ButtonRow>
    </section>
  );
}

/** The page's heading line: how many are chosen, and how many of them the list does not show. */
function Counted({
  count,
  notShown,
  only,
  onOnly,
}: {
  count: number;
  notShown: number;
  only: boolean;
  onOnly: () => void;
}) {
  return (
    <>
      {selectedSaid(count)}
      {notShown > 0 && (
        <>
          {", "}
          <Button
            variant="quiet-link"
            tooltip={only ? "Show the list as it was" : "Show only what is selected"}
            data-testid="select-not-shown"
            onPress={onOnly}
          >
            {`${notShown} not shown`}
          </Button>
        </>
      )}
    </>
  );
}

/** **One row of the actions**: what it does, how many of those chosen it reaches, and its
 *  button, which opens its review. */
function ActionsList<A extends string>({
  acts,
  busy,
  onAct,
}: {
  acts: readonly { act: A; label: string; says: string; count: string; danger?: boolean }[];
  busy: boolean;
  onAct: (act: A) => void;
}) {
  return (
    <>
      <h3 className="ui-text-small ui-text-strong">What to do with them</h3>
      <List
        aria-label="What to do with them"
        className="skill-select-acts"
        data-testid="select-acts"
        onAction={(key) => {
          const act = acts.find((one) => one.act === String(key));
          if (!busy && act !== undefined) onAct(act.act);
        }}
      >
        {acts.map((one) => (
          <ListRow
            key={one.act}
            id={one.act}
            textValue={`${one.label}, ${one.count}`}
            data-testid={`select-act-${one.act}`}
          >
            <RowLabel>{one.label}</RowLabel>
            <RowTrail>
              <Badge>{one.count}</Badge>
              <Button
                size="sm"
                variant={one.danger === true ? "danger" : "secondary"}
                isDisabled={busy}
                onPress={() => onAct(one.act)}
              >
                {`${one.label}…`}
              </Button>
            </RowTrail>
            <RowBelow clamp={1}>{one.says}</RowBelow>
          </ListRow>
        ))}
      </List>
    </>
  );
}

/**
 * **The selection page**, beside the list while Select is on: the count, every action with how
 * many of those chosen it reaches ("12 of 14"), then the names, those the list does not show
 * first. Nothing chosen, it says how to choose.
 */
export function SelectionPage({
  chosen,
  notShown,
  only,
  at,
  busy,
  onAct,
  onOut,
  onClear,
  onOnly,
  room = false,
}: {
  /** Make room (#5355): only its actions are offered. */
  room?: boolean;
  /** Every skill chosen, the list's order. */
  chosen: readonly { key: string; skill: StudioSkill }[];
  /** Those of them the list does not draw now: filtered out, or a built-in folded away. */
  notShown: ReadonlySet<string>;
  only: boolean;
  at: SelectFor;
  busy: boolean;
  onAct: (act: SelectAct) => void;
  onOut: (key: string) => void;
  onClear: () => void;
  onOnly: () => void;
}) {
  if (chosen.length === 0)
    return (
      <EmptyState
        headline="Nothing selected"
        body={
          room
            ? "The skills least used in the last 30 days are listed first. Tick what goes, to turn it off or archive it, or what stays, to pin it. Esc leaves Make room."
            : "Click a row, or press Space on it; Shift-click takes a range. ⌘A takes everything the search and filters match, and Esc leaves Select."
        }
        testid="select-none"
      />
    );
  const skills = chosen.map((one) => one.skill);
  const acts = SELECT_ACTS.filter((one) => askable(one.act, at))
    .filter((one) => !room || MAKE_ROOM_ACTS.includes(one.act))
    .map((one) => ({ ...one, reach: reachOf(one.act, skills, at).will.length }))
    .filter((one) => one.reach > 0)
    .map((one) => ({
      ...one,
      count: `${one.reach} of ${chosen.length}`,
      danger: one.act === "delete",
    }));
  const named = (list: readonly { key: string; skill: StudioSkill }[]) =>
    list.map(({ key, skill }) => ({ key, name: skill.name, label: skill.label }));
  const hidden = chosen.filter((one) => notShown.has(one.key));
  const drawn = chosen.filter((one) => !notShown.has(one.key));
  return (
    <div className="skill-studio-item" data-testid="select-page">
      <ViewPageHeader
        name={
          <Counted count={chosen.length} notShown={hidden.length} only={only} onOnly={onOnly} />
        }
        actions={
          <Button size="sm" variant="ghost" isDisabled={busy} onPress={onClear}>
            Clear
          </Button>
        }
      />
      <div className="skill-select-body">
        {acts.length === 0 ? (
          <p className="honest" data-testid="select-acts-none">
            Nothing can change these: built-in skills and skills from outside the project stay as
            they are.
          </p>
        ) : (
          <ActionsList acts={acts} busy={busy} onAct={onAct} />
        )}
        {hidden.length > 0 && (
          <Names
            title={`Not shown in the list (${hidden.length})`}
            skills={named(hidden)}
            testid="select-names-not-shown"
            onOut={onOut}
          />
        )}
        <Names
          title={`${hidden.length > 0 ? "In the list" : "Selected"} (${drawn.length})`}
          skills={named(drawn)}
          testid="select-names"
          onOut={onOut}
        />
      </div>
    </div>
  );
}

/** What the review of `act` on `count` skills is named: "Delete 12 skills?". */
export function reviewTitle(act: SkillSetAct, count: number): string {
  const verb = setVerb(act);
  const skills = counted(count, "skill", "skills");
  return act === "export" ? `Export ${skills}` : `${verb} ${skills}?`;
}

/** **What a move asks**: the level they go to, and where they reach when that is the project's. */
function MoveFields({
  levels,
  workspaces,
  to,
  grant,
  busy,
  onTo,
  onGrant,
}: {
  levels: readonly StudioLevel[];
  workspaces: readonly string[];
  to: SkillLevel | null;
  grant: SkillGrant;
  busy: boolean;
  onTo: (to: SkillLevel | null) => void;
  onGrant: (grant: SkillGrant) => void;
}) {
  return (
    <>
      <SettingRow
        label="Move them to"
        grouped
        control={(ids) => (
          <Choice
            kind="radio"
            ids={ids}
            options={levels.map((one) => ({
              value: levelKey(one.level),
              label: one.label,
              says: whoGets(one.level),
            }))}
            value={to === null ? undefined : levelKey(to)}
            onValueChange={(key) =>
              onTo(levels.find((one) => levelKey(one.level) === key)?.level ?? null)
            }
            disabled={busy}
          />
        )}
      />
      {to?.at === "project" && (
        <GrantPicker
          label="Where they will reach"
          workspaces={workspaces}
          now={grant}
          onPick={onGrant}
        />
      )}
    </>
  );
}

/** **What an export asks and says**: a zip or a folder, and what stays behind. */
function ExportFields({
  plane,
  members,
  zip,
  busy,
  onZip,
}: {
  plane: string;
  members: readonly SkillSetMember[];
  zip: boolean;
  busy: boolean;
  onZip: (zip: boolean) => void;
}) {
  const { read } = useRead(members.map(memberKey).join("\u0001"), () =>
    commands.skillSetExportReview(plane, [...members]),
  );
  return (
    <>
      <SettingRow
        label="Export as"
        grouped
        control={(ids) => (
          <Choice
            kind="radio"
            ids={ids}
            value={zip ? "zip" : "folder"}
            disabled={busy}
            onValueChange={(value) => onZip(value === "zip")}
            options={[
              { value: "zip", label: "A zip", says: "One file to send to someone" },
              { value: "folder", label: "A folder", says: "Skill folders to drop into a project" },
            ]}
          />
        )}
      />
      {read?.refused !== undefined && <p className="honest">{read.refused}</p>}
      {(read?.data?.behind ?? []).map((file) => (
        <p key={file.path} className="honest" data-testid="select-export-behind">
          Stays behind: <code>{file.path}</code>, {file.why}
        </p>
      ))}
      {(read?.data?.broken ?? []).map((one) => (
        <p key={one} className="honest">
          They refer to {one}, which is not there.
        </p>
      ))}
    </>
  );
}

/** What a review of the Library's Select mode hands on: the skills to change, and what the
 *  change asked for on the way. */
export type Chosen = {
  members: SkillSetMember[];
  to: SkillLevel | null;
  grant: SkillGrant | null;
  zip: boolean;
};

/**
 * **An action's review, in the selection page's place**: the one review (`SetReviewPage`), with
 * what the action leaves as it is said under "Stay as they are", and what a move or an export
 * asks first. Cancel goes back to the selection.
 */
export function SelectReviewPage({
  plane,
  act,
  reach,
  workspace,
  levels,
  workspaces,
  busy,
  onCancel,
  onApply,
}: {
  plane: string;
  act: SkillSetAct;
  reach: Reach;
  workspace: string | null;
  /** The levels a move can go to: the studio's own. */
  levels: readonly StudioLevel[];
  workspaces: readonly string[];
  busy: boolean;
  onCancel: () => void;
  onApply: (chosen: Chosen) => void;
}) {
  const [to, setTo] = useState<SkillLevel | null>(null);
  const [grant, setGrant] = useState<SkillGrant>({ to: "root" });
  const [zip, setZip] = useState(true);
  const members = [...reach.will];
  // A move takes out those already at the level picked: they stay as they are.
  const already =
    act === "move" && to !== null ? members.filter((one) => sameLevel(one.level, to)) : [];
  const named = members.filter((one) => !already.includes(one));
  const stays: Stays[] = [
    ...already.map((one) => ({ name: one.name, label: one.label, why: "It is there already." })),
    ...reach.stays,
  ];
  return (
    <SetReviewPage
      plane={plane}
      act={act}
      title={reviewTitle(act, named.length)}
      named={named}
      ticked={named.map(memberKey)}
      workspace={workspace}
      to={act === "move" ? to : null}
      stays={stays}
      main={act === "export" ? "Choose where to save…" : undefined}
      unready={act === "move" && to === null ? "Pick where they go." : undefined}
      fields={
        act === "move" ? (
          <MoveFields
            levels={levels}
            workspaces={workspaces}
            to={to}
            grant={grant}
            busy={busy}
            onTo={setTo}
            onGrant={setGrant}
          />
        ) : act === "export" ? (
          <ExportFields plane={plane} members={members} zip={zip} busy={busy} onZip={setZip} />
        ) : undefined
      }
      busy={busy}
      onCancel={onCancel}
      onApply={(going) =>
        onApply({
          members: going,
          to: act === "move" ? to : null,
          grant: act === "move" && to?.at === "project" ? grant : null,
          zip,
        })
      }
    />
  );
}

// --- The Deleted page's Select -------------------------------------------------------------

/** What Select can do on the Deleted page. */
export type DeletedAct = "bring-back" | "delete-now";

/**
 * **The Deleted page's selection page** (#5332): the count, **Bring back** and **Delete now**,
 * each with how many it reaches, then the names.
 */
export function DeletedSelectionPage({
  chosen,
  busy,
  onAct,
  onOut,
  onClear,
}: {
  chosen: readonly DeletedSkill[];
  busy: boolean;
  onAct: (act: DeletedAct) => void;
  onOut: (id: string) => void;
  onClear: () => void;
}) {
  if (chosen.length === 0)
    return (
      <EmptyState
        headline="Nothing selected"
        body="Click a row, or press Space on it; Shift-click takes a range. ⌘A takes every deleted skill, and Esc leaves Select."
        testid="select-none"
      />
    );
  const back = chosen.filter((one) => !one.reserved).length;
  const count = (n: number) => `${n} of ${chosen.length}`;
  return (
    <div className="skill-studio-item" data-testid="select-page">
      <ViewPageHeader
        name={selectedSaid(chosen.length)}
        actions={
          <Button size="sm" variant="ghost" isDisabled={busy} onPress={onClear}>
            Clear
          </Button>
        }
      />
      <div className="skill-select-body">
        <ActionsList<DeletedAct>
          acts={[
            ...(back > 0
              ? [
                  {
                    act: "bring-back" as const,
                    label: "Bring back",
                    says: "With their history and grades. One Undo right after.",
                    count: count(back),
                  },
                ]
              : []),
            {
              act: "delete-now" as const,
              label: "Delete now",
              says: "It asks first: this one has no undo.",
              count: count(chosen.length),
              danger: true,
            },
          ]}
          busy={busy}
          onAct={onAct}
        />
        <Names
          title={`Selected (${chosen.length})`}
          skills={chosen.map((one) => ({ key: one.id, name: one.name, label: one.label }))}
          testid="select-names"
          onOut={onOut}
        />
      </div>
    </div>
  );
}

/**
 * **The review of Bring back or Delete now**, in the selection page's place. Bring back leaves
 * out a skill whose name belongs to purlis now, which comes back only on its own (#5331's D-7);
 * Delete now says which leave only this list and which leave this machine for good, and is the
 * one change on the page with no undo.
 */
export function DeletedSetReview({
  act,
  chosen,
  busy,
  onCancel,
  onApply,
}: {
  act: DeletedAct;
  chosen: readonly DeletedSkill[];
  busy: boolean;
  onCancel: () => void;
  onApply: (going: DeletedSkill[]) => void;
}) {
  const now = act === "delete-now";
  const going = now ? [...chosen] : chosen.filter((one) => !one.reserved);
  const stay = now ? [] : chosen.filter((one) => one.reserved);
  const skills = counted(going.length, "skill", "skills");
  const kept = going.filter((one) => one.in_history).length;
  const gone = going.length - kept;
  const title = now ? `Delete ${skills} now?` : `Bring back ${skills}?`;
  return (
    <ReviewPage
      label={title}
      data-testid="set-review"
      summary={now ? "This can't be undone." : "Nothing comes back until you press Bring back."}
      onCancel={onCancel}
      main={{
        children: now ? `Delete ${skills} now` : `Bring back ${skills}`,
        ...(now ? { variant: "danger" as const } : { byReturn: false }),
        isDisabled: busy || going.length === 0,
        onPress: () => onApply(going),
      }}
    >
      <h2 className="ui-text-strong">{title}</h2>
      <p className="honest" data-testid="set-review-lead">
        {now
          ? [
              kept > 0 &&
                `${counted(kept, "leaves", "leave")} this list, and the project's history still holds ${kept === 1 ? "it" : "them"}.`,
              gone > 0 && `${counted(gone, "is", "are")} removed from this machine for good.`,
            ]
              .filter(Boolean)
              .join(" ")
          : "They come back with their history and grades, and chats started after that get them again. It is one change, with one Undo right after."}
      </p>
      <section aria-labelledby="set-going" className="skill-review" data-testid="set-going-list">
        <h3 id="set-going" className="ui-text-small ui-text-strong">
          {now ? "Will be deleted now" : "Will come back"}
        </h3>
        {going.map((one) => (
          <p key={one.id} className="honest">
            <code>{one.name}</code> ({one.label})
          </p>
        ))}
      </section>
      {stay.length > 0 && (
        <section aria-labelledby="set-stays" className="skill-review" data-testid="set-stays">
          <h3 id="set-stays" className="ui-text-small ui-text-strong">
            Stay as they are
          </h3>
          {stay.map((one) => (
            <p key={one.id} className="honest" data-testid={`set-stays-${one.name}`}>
              <code>{one.name}</code> {NAME_TAKEN_STAYS}
            </p>
          ))}
        </section>
      )}
    </ReviewPage>
  );
}
