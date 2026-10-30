// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **Rename, Move to another level and Delete** (#3685, spec #3679 S4): each a dialog that says
 * what changes and who is affected before the person confirms, as a short choice is (S2). What
 * the core refuses is said in the dialog, beside what the person is still choosing; what is done
 * ends in the studio's result notice, with Undo, or Bring it back after a delete.
 *
 * Who uses the skill is the core's answer (`skill_users`), read as the dialog opens: the chats it
 * reaches, and the personas, skills and pins that name it.
 */
import { useEffect, useState } from "react";
import {
  commands,
  type SkillGrant,
  type SkillLevel,
  type SkillPinner,
  type SkillUsers,
  type StudioLevel,
  type StudioSkill,
} from "./bindings";
import { counted } from "./dispatches";
import { NAME_RESERVED } from "./SkillCopy";
import { GrantPicker } from "./SkillLevelPickers";
import { Choice, Field, SettingRow } from "./settings/components";
import { labelOf, levelKey, sameLevel } from "./skillChanges";
import { Button } from "./ui/button/Button";
import { Dialog } from "./ui/dialog/Dialog";

/** What a dialog says of a skill's history across a rename or a move: History follows the folder
 *  across the renames and moves git finds, and Grades the names and levels it had (#3918). */
export const HISTORY_MOVES = "Its history and grades move with it.";

/** What it says where the skill is, or is to be, only on this machine: git holds no history of
 *  it to follow (#3918). */
export const HISTORY_RESTARTS_LOCAL =
  "A skill only on this machine keeps no history, so its History and Grades start again from this change.";

/** What a rename or a move says of the skill's history, by whether it is on this machine alone
 *  before or after. */
export function historySaid(local: boolean): string {
  return local ? HISTORY_RESTARTS_LOCAL : HISTORY_MOVES;
}

/** How long a deleted skill can be brought back, as Delete says it before it deletes. */
export const DELETED_FOR = "It moves to Deleted for 30 days, where you can bring it back.";

// --- Who uses a skill, in words -----------------------------------------------------------------

/** The chats a skill reaches now, each as a person reads it. */
export function chatsOf(users: SkillUsers): string[] {
  return reachOfUsers(users).map(reachSaid);
}

/** A pinner, as a person reads it: `workspace ops`, `persona steward`. */
export function pinnerSaid(by: SkillPinner): string {
  return `${by.at} ${by.name}`;
}

/** `words`, first letter up. */
function upperFirst(words: string): string {
  return words.charAt(0).toUpperCase() + words.slice(1);
}

/** **What changes with a rename** (S4): the personas and skills that name it, its pins, each by
 *  name, or that nothing else names it. */
export function renameReach(users: SkillUsers, local: boolean): string[] {
  // Pins are written in the project's files, which a local skill's rename leaves alone.
  const pins = local ? [] : users.pinned_by;
  const lines = [
    ...(users.personas.length === 0
      ? []
      : [
          `${counted(users.personas.length, "persona that names it", "personas that name it")}: ${users.personas.join(", ")}`,
        ]),
    ...(users.skills.length === 0
      ? []
      : [
          `${counted(users.skills.length, "skill that names it", "skills that name it")}: ${users.skills.join(", ")}`,
        ]),
    ...(pins.length === 0 ? [] : [`Its pins, by ${pins.map(pinnerSaid).join(", ")}`]),
  ];
  return lines.length === 0 ? ["No persona, skill or pin names it."] : lines;
}

/** **Who gets a skill at `level`** (S4), as Move says it beside each level. */
export function whoGets(level: SkillLevel): string {
  const who =
    level.at === "project"
      ? "Chats its grant names get it: the project's root, every workspace, or the workspaces you name."
      : level.at === "workspace"
        ? `Only chats in ${level.name} get it.`
        : `Only chats as ${level.name} get it, in every workspace.`;
  return level.local ? `${who} Only on this machine: it is never saved to the project.` : who;
}

/** A chat a skill reaches, by a key that compares: `root`, `ws:<name>`, `as:<persona>`. */
type Reach = string;

function reachOfUsers(users: SkillUsers): Reach[] {
  return [
    ...(users.root ? ["root"] : []),
    ...users.workspaces.map((ws) => `ws:${ws}`),
    ...(users.persona === null ? [] : [`as:${users.persona}`]),
  ];
}

/** The chats a skill moved from `from` to `to` (with `grant`, into the project) reaches. A
 *  persona's own skill moved into the project is added to that persona's skills (SK-32), so its
 *  chats keep it. */
function reachAt(
  now: readonly Reach[],
  from: SkillLevel,
  to: SkillLevel,
  grant: SkillGrant | null,
  workspaces: readonly string[],
): readonly Reach[] {
  if (to.at === "workspace") return [`ws:${to.name}`];
  if (to.at === "persona") return [`as:${to.name}`];
  // Between the project's shared and local skills it keeps its grant: the same chats.
  if (from.at === "project") return now;
  const reached =
    grant === null || grant.to === "root" ? [] : grant.to === "all" ? workspaces : grant.workspaces;
  return [
    "root",
    ...reached.map((ws) => `ws:${ws}`),
    ...(from.at === "persona" ? [`as:${from.name}`] : []),
  ];
}

/** A reach, as a person reads it. */
function reachSaid(reach: Reach): string {
  if (reach === "root") return "chats at the project's root";
  if (reach.startsWith("ws:")) return `chats in ${reach.slice(3)}`;
  return `chats as ${reach.slice(3)}, in every workspace`;
}

/** **What changes for this move** (S4): who stops getting it, who starts, and what leaving or
 *  joining the project's shared skills means. */
export function moveChanges(
  users: SkillUsers,
  from: SkillLevel,
  to: SkillLevel,
  grant: SkillGrant | null,
  workspaces: readonly string[],
): string[] {
  const now = reachOfUsers(users);
  const then = reachAt(now, from, to, grant, workspaces);
  const lost = now.filter((one) => !then.includes(one));
  const gained = then.filter((one) => !now.includes(one));
  return [
    ...(lost.length === 0
      ? []
      : [`${upperFirst(lost.map(reachSaid).join(", "))} stop getting it.`]),
    ...(gained.length === 0
      ? []
      : [`${upperFirst(gained.map(reachSaid).join(", "))} start getting it.`]),
    ...(to.local && !from.local
      ? ["It leaves the project: it stays on this machine, and teammates no longer get it."]
      : []),
    ...(from.local && !to.local ? ["It is saved to the project: teammates get it too."] : []),
    ...(lost.length + gained.length === 0 && to.local === from.local
      ? ["The same chats get it."]
      : []),
  ];
}

// --- Reading who uses it ------------------------------------------------------------------------

/** Whether `data` is who uses a skill, as this window reads it. */
function usersShaped(data: SkillUsers): boolean {
  const lists = [data.workspaces, data.personas, data.skills, data.pinned_by] as unknown[];
  return lists.every(Array.isArray);
}

/** Who uses the skill `name` at `level`, read as the dialog opens: the answer, or why not. */
function useUsers(plane: string, level: SkillLevel, name: string) {
  const [asked, setAsked] = useState(0);
  const key = `${plane}\n${levelKey(level)}\n${name}\n${asked}`;
  // What was read for another ask is not this one's: none until this one's is.
  const [read, setRead] = useState<{ key: string; users?: SkillUsers; refused?: string }>();
  useEffect(() => {
    let live = true;
    void commands
      .skillUsers(plane, level, name)
      .then((answer) => {
        if (!live) return;
        // Taken only in its own shape: a core that answers another leaves it unread.
        if (answer.status === "ok" && usersShaped(answer.data))
          setRead({ key, users: answer.data });
        else if (answer.status === "ok") setRead({ key, refused: "it answered in another shape" });
        else setRead({ key, refused: answer.error });
      })
      .catch((err: unknown) => {
        if (live) setRead({ key, refused: String(err) });
      });
    return () => {
      live = false;
    };
    // `level` is read by its key: a new object of the same level is not a new read.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key]);
  const now = read?.key === key ? read : undefined;
  return { users: now?.users, refused: now?.refused, again: () => setAsked((n) => n + 1) };
}

/** A list of what the dialog says changes, under its heading, or the read on its way or failed. */
function Reach({
  heading,
  lines,
  refused,
  again,
  testid,
}: {
  heading: string;
  lines: readonly string[] | undefined;
  refused: string | undefined;
  again: () => void;
  testid: string;
}) {
  return (
    <section className="skill-change-reach" aria-label={heading} data-testid={testid}>
      <h3>{heading}</h3>
      {refused !== undefined ? (
        <p className="trouble">
          {`purlis could not read who uses it: ${refused} `}
          <Button size="sm" onPress={again}>
            Try again
          </Button>
        </p>
      ) : lines === undefined ? (
        <p className="pending" aria-busy="true">
          Reading who uses it…
        </p>
      ) : (
        <ul>
          {lines.map((line) => (
            <li key={line}>{line}</li>
          ))}
        </ul>
      )}
    </section>
  );
}

/** The core's refusal of the last ask, said in the dialog. */
function Refused({ refused, testid }: { refused: string | undefined; testid: string }) {
  return refused === undefined ? null : (
    <p className="trouble" role="alert" data-testid={testid}>
      {refused}
    </p>
  );
}

// --- Rename -------------------------------------------------------------------------------------

/** **Why `to` can't be the skill's new name**, as it is typed (S4, S12): its shape, a built-in
 *  skill's name, or a name its level has. The core checks again when asked. */
export function nameProblem(
  to: string,
  name: string,
  taken: readonly string[],
  builtIns: readonly string[],
): string | undefined {
  if (to === "" || to === name) return undefined;
  if (!/^[a-z0-9]+(-[a-z0-9]+)*$/.test(to))
    return "Use lowercase letters, digits and hyphens, with no hyphen at either end.";
  if (builtIns.includes(to)) return NAME_RESERVED;
  if (taken.includes(to)) return `There is already a skill named ${to} here.`;
  return undefined;
}

/** **Rename** (S4): the new name, checked as it is typed, and what changes with it. */
export function RenameDialog({
  plane,
  skill,
  level,
  taken,
  builtIns,
  busy,
  refused,
  onEdit,
  onRename,
  onClose,
}: {
  plane: string;
  skill: StudioSkill;
  level: SkillLevel;
  /** The names of the other skills at its level, which it can't take. */
  taken: readonly string[];
  /** Every built-in skill's name, which no skill can take (S12). */
  builtIns: readonly string[];
  busy: boolean;
  /** Why the core did not rename it, the last time it was asked. */
  refused?: string;
  /** The name changed: a refusal of the last one no longer stands. */
  onEdit: () => void;
  onRename: (to: string) => void;
  onClose: () => void;
}) {
  const name = skill.name;
  const [to, setTo] = useState(name);
  const { users, refused: unread, again } = useUsers(plane, level, name);
  const wanted = to.trim();
  const problem = nameProblem(wanted, name, taken, builtIns);
  const ready = !busy && wanted !== "" && wanted !== name && problem === undefined;
  return (
    <Dialog
      onClose={onClose}
      title={`Rename ${name}`}
      titleId="skill-rename"
      description={`Every persona and skill that names it${level.local ? "" : ", and its pins,"} are changed with it, in one commit.`}
      initialFocus="field"
      main={{
        children: busy ? "Renaming…" : "Rename",
        reserve: ["Rename", "Renaming…"],
        isDisabled: !ready,
        onPress: () => {
          if (ready) onRename(wanted);
        },
      }}
      data-testid="skill-rename"
    >
      <SettingRow
        label="New name"
        help="Lowercase letters, digits and hyphens."
        control={(ids) => (
          <Field
            kind="text"
            ids={ids}
            value={to}
            onChange={(next) => {
              setTo(next);
              onEdit();
            }}
          />
        )}
      />
      {problem !== undefined && (
        <p className="trouble" role="alert" data-testid="skill-rename-problem">
          {problem}
        </p>
      )}
      <Refused refused={refused} testid="skill-rename-refused" />
      <Reach
        heading="Changed with it"
        lines={users === undefined ? undefined : renameReach(users, level.local)}
        refused={unread}
        again={again}
        testid="skill-rename-reach"
      />
      <p className="honest">{historySaid(level.local)}</p>
      <p className="honest">Chats already running keep the old name until they restart.</p>
    </Dialog>
  );
}

// --- Move ---------------------------------------------------------------------------------------

/** **Move to another level** (S4): each level with who gets it there, and what changes for the
 *  move picked; into the project's skills, where it reaches (decision 14). */
export function MoveDialog({
  plane,
  skill,
  from,
  levels,
  workspaces,
  busy,
  refused,
  onEdit,
  onMove,
  onClose,
}: {
  plane: string;
  skill: StudioSkill;
  from: SkillLevel;
  /** The levels the studio offers from its scope, the core's words for each. */
  levels: readonly StudioLevel[];
  workspaces: readonly string[];
  busy: boolean;
  refused?: string;
  onEdit: () => void;
  onMove: (to: SkillLevel, grant: SkillGrant | null) => void;
  onClose: () => void;
}) {
  const name = skill.name;
  const [to, setTo] = useState<SkillLevel>();
  const [grant, setGrant] = useState<SkillGrant>(
    from.at === "workspace" ? { to: "only", workspaces: [from.name] } : { to: "root" },
  );
  // Whether the move picked makes it a project skill, which asks where it reaches: the core's.
  const [asksGrant, setAsksGrant] = useState<{ key: string; asks?: boolean; refused?: string }>();
  const [askedAgain, setAskedAgain] = useState(0);
  const { users, refused: unread, again } = useUsers(plane, from, name);
  const toKey = to === undefined ? undefined : levelKey(to);
  useEffect(() => {
    if (to === undefined || toKey === undefined) return;
    let live = true;
    void commands
      .skillMoveNeedsGrant(from, to)
      .then((asks) => {
        if (live) setAsksGrant({ key: toKey, asks });
      })
      .catch((err: unknown) => {
        if (live) setAsksGrant({ key: toKey, refused: String(err) });
      });
    return () => {
      live = false;
    };
    // The levels are read by their keys.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [levelKey(from), toKey, askedAgain]);
  const grantRead = asksGrant !== undefined && asksGrant.key === toKey ? asksGrant : undefined;
  const grantAsked = grantRead?.asks;
  const others = levels.filter((one) => !sameLevel(one.level, from));
  const ready = !busy && to !== undefined && grantAsked !== undefined;
  const fromLabel = labelOf(levels, from);
  return (
    <Dialog
      onClose={onClose}
      title={`Move ${name} to another level`}
      titleId="skill-move"
      description={`Who gets it changes with where it lives. It is a ${fromLabel} skill now. ${whoGets(from)}`}
      summary={
        to === undefined
          ? undefined
          : `${name} becomes a ${labelOf(levels, to)} skill, in one commit.`
      }
      main={{
        children: busy ? "Moving…" : "Move",
        reserve: ["Move", "Moving…"],
        isDisabled: !ready,
        onPress: () => {
          if (ready && to !== undefined) onMove(to, grantAsked ? grant : null);
        },
      }}
      data-testid="skill-move"
    >
      <SettingRow
        label="Move it to"
        grouped
        control={(ids) => (
          <Choice
            kind="radio"
            ids={ids}
            options={others.map((one) => ({
              value: levelKey(one.level),
              label: one.label,
              says: whoGets(one.level),
            }))}
            value={toKey}
            onValueChange={(key) => {
              setTo(others.find((one) => levelKey(one.level) === key)?.level);
              onEdit();
            }}
            disabled={busy}
          />
        )}
      />
      {to !== undefined && grantAsked === true && (
        <>
          <p className="honest">A project skill reaches only the chats its grant names.</p>
          <GrantPicker
            label={`Where ${name} will reach`}
            workspaces={workspaces}
            now={grant}
            onPick={(next) => {
              setGrant(next);
              onEdit();
            }}
          />
          {from.at === "persona" && (
            <p className="honest" data-testid="persona-keeps">
              {from.name} keeps it in every workspace: it is added to {from.name}'s skills.
            </p>
          )}
        </>
      )}
      {grantRead?.refused !== undefined && (
        <p className="trouble" data-testid="skill-move-grant-unread">
          {`purlis could not tell whether this move asks where it reaches: ${grantRead.refused} `}
          <Button size="sm" onPress={() => setAskedAgain((n) => n + 1)}>
            Try again
          </Button>
        </p>
      )}
      <Refused refused={refused} testid="skill-move-refused" />
      {to !== undefined && (
        <>
          <Reach
            heading="What changes"
            lines={
              users === undefined
                ? undefined
                : moveChanges(users, from, to, grantAsked ? grant : null, workspaces)
            }
            refused={unread}
            again={again}
            testid="skill-move-changes"
          />
          <p className="honest">{historySaid(from.local || to.local)}</p>
        </>
      )}
    </Dialog>
  );
}

// --- Delete -------------------------------------------------------------------------------------

/** **Delete** (S4): who uses it now, and that it waits in Deleted for 30 days. */
export function DeleteDialog({
  plane,
  skill,
  level,
  busy,
  refused,
  onDelete,
  onClose,
}: {
  plane: string;
  skill: StudioSkill;
  level: SkillLevel;
  busy: boolean;
  refused?: string;
  onDelete: () => void;
  onClose: () => void;
}) {
  const name = skill.name;
  const { users, refused: unread, again } = useUsers(plane, level, name);
  const chats = users === undefined ? [] : chatsOf(users);
  const lines =
    users === undefined
      ? undefined
      : [
          chats.length === 0 ? "No chat gets it now." : `${upperFirst(chats.join(", "))}.`,
          ...(users.personas.length === 0
            ? []
            : [
                `${counted(users.personas.length, "persona names", "personas name")} it: ${users.personas.join(", ")}.`,
              ]),
          ...(users.skills.length === 0
            ? []
            : [
                `${counted(users.skills.length, "skill names", "skills name")} it: ${users.skills.join(", ")}.`,
              ]),
          ...(users.pinned_by.length === 0
            ? []
            : [`Pinned by ${users.pinned_by.map(pinnerSaid).join(", ")}.`]),
        ];
  return (
    <Dialog
      onClose={onClose}
      title={`Delete ${name}?`}
      titleId="skill-delete"
      description={`${DELETED_FOR} Chats started after this don't get it.`}
      main={{
        children: busy ? "Deleting…" : "Delete",
        reserve: ["Delete", "Deleting…"],
        variant: "danger",
        isDisabled: busy,
        onPress: onDelete,
      }}
      data-testid="skill-delete"
    >
      <Reach
        heading="Who uses it now"
        lines={lines}
        refused={unread}
        again={again}
        testid="skill-users"
      />
      <Refused refused={refused} testid="skill-delete-refused" />
    </Dialog>
  );
}
