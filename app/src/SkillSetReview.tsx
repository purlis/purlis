// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useState, type ReactNode } from "react";
import {
  commands,
  type InstalledCopy,
  type SkillGrant,
  type SkillLevel,
  type SkillPinner,
  type SkillSource,
  type SkillSetAct,
  type SkillSetLifecycle,
  type SkillSetMember,
  type SkillSetMove,
  type SkillSetReview as Review,
} from "./bindings";
import { counted } from "./dispatches";
import { sameLevel } from "./skillChanges";
import type { Change, Said } from "./SkillPage";
import { BUSY, Held, useRead } from "./SkillRead";
import { Badge } from "./ui/badge/Badge";
import { Button, ButtonRow } from "./ui/button/Button";
import { Checkbox } from "./ui/choice/Checkbox";
import { List, ListRow, ListRowSection } from "./ui/list/List";
import { RowLabel, RowLead, RowTrail } from "./ui/list/Row";
import { ReviewPage } from "./ui/review-page/ReviewPage";

/**
 * **A change to several skills at once** (#5331): a tick list grouped by level, then one review,
 * then one commit that fully applies or changes nothing, with one Undo. A source's page reaches
 * them with what is installed from it; neither page knows a source, only a set of skills and the
 * words it is shown under, so the Library's Select mode (#5332) can reach them too.
 *
 * - **The list** ({@link SetChoosePage}): each skill under its level's name. What starts ticked is
 *   {@link startsTicked}'s.
 * - **The review** ({@link SetReviewPage}): each skill, with its box; the warning for a skill left
 *   naming one that goes, which does not block and offers to keep the skill it names; how many
 *   open chats keep them until they restart; and each skill that cannot apply, taken out with its
 *   reason before anything is written.
 */

/** What a page says of a set before anything is ticked. */
const TICK_ONE = "Tick at least one skill.";

/** What a set's words are, by what is asked of it: the one place they are written. */
const WORDS: Record<
  SkillSetAct,
  {
    /** On its main action and each skill's box: "Delete". */
    verb: string;
    /** After "will be", in a review's first line: "deleted". */
    done: string;
    /** In the page's title: "Delete all from". */
    title: string;
    /** After a name: "which you're deleting". */
    doing: string;
    /** At the foot, before the main action. */
    summary: string;
    /** What the change does, after how many skills it holds. */
    does: (workspace: string | null) => string;
  }
> = {
  delete: {
    done: "deleted",
    verb: "Delete",
    title: "Delete all from",
    doing: "which you're deleting",
    summary: "Nothing is deleted until you press Delete.",
    does: () =>
      "Each stays in Deleted for 30 days. It is one change, and Undo is offered right after.",
  },
  "turn-off": {
    done: "turned off",
    verb: "Turn off",
    title: "Turn all off from",
    doing: "which you're turning off",
    summary: "Nothing is turned off until you press Turn off.",
    does: (workspace) =>
      `It turns off in ${workspace ?? "this workspace"} as one change, and Undo is offered right after.`,
  },
  "turn-on": {
    done: "turned on",
    verb: "Turn on",
    title: "Turn all on from",
    doing: "which stays off",
    summary: "Nothing is turned on until you press Turn on.",
    does: (workspace) =>
      `It turns on in ${workspace ?? "this workspace"} as one change, and Undo is offered right after.`,
  },
  pin: {
    done: "pinned",
    verb: "Pin",
    title: "Pin all from",
    doing: "which you're pinning",
    summary: "Nothing is pinned until you press Pin.",
    does: () =>
      "A pinned skill stays in a chat's list, whatever the budget. It is one change, and Undo is offered right after.",
  },
  unpin: {
    done: "unpinned",
    verb: "Unpin",
    title: "Unpin all from",
    doing: "which you're unpinning",
    summary: "Nothing is unpinned until you press Unpin.",
    does: () =>
      "The router decides about them again. It is one change, and Undo is offered right after.",
  },
  archive: {
    done: "archived",
    verb: "Archive",
    title: "Archive all from",
    doing: "which you're archiving",
    summary: "Nothing is archived until you press Archive.",
    does: () =>
      "They never load until they are restored. It is one change, and Undo is offered right after.",
  },
  unarchive: {
    done: "unarchived",
    verb: "Unarchive",
    title: "Unarchive all from",
    doing: "which you're unarchiving",
    summary: "Nothing is unarchived until you press Unarchive.",
    does: () => "They load again. It is one change, and Undo is offered right after.",
  },
  move: {
    done: "moved",
    verb: "Move",
    title: "Move all from",
    doing: "which you're moving",
    summary: "Nothing is moved until you press Move.",
    does: () =>
      "They keep their history and grades. It is one change, and Undo is offered right after.",
  },
  export: {
    done: "exported",
    verb: "Export",
    title: "Export all from",
    doing: "which you're exporting",
    summary: "Nothing is written until you choose where to save it.",
    does: () =>
      "Export writes copies. Nothing in the project changes, so there is nothing to undo.",
  },
};

/** What `act`'s main action says: "Delete", "Turn off". */
export function setVerb(act: SkillSetAct): string {
  return WORDS[act].verb;
}

/** What a page of `act` on what came from `from` is named. */
export function setTitle(act: SkillSetAct, from: string): string {
  return `${WORDS[act].title} ${from}`;
}

/** What the studio says after a change to several skills. */
function saidOf(label: string, text: string, notes: readonly string[], undo?: () => void): Said {
  return {
    tone: "news",
    label,
    text: [text, ...notes].join(" "),
    ...(undo === undefined ? {} : { undo }),
  };
}

/**
 * **The change a review asks** (#5331): one commit, and one Undo right after. `change` makes it,
 * and goes on from the page it was made on; `undo` makes its Undo, from wherever the person is.
 * The Library's Select mode (#5332) asks the rest of its changes the same way: a pin by `by`, a
 * move `to` a level with `grant`, an archive, and an export, which commits nothing and has no Undo.
 */
export function applySet({
  plane,
  act,
  members,
  from,
  removes,
  workspace,
  by = null,
  to = null,
  grant = null,
  zip = true,
  change,
  undo,
}: {
  plane: string;
  act: SkillSetAct;
  members: readonly SkillSetMember[];
  /** What the group is said to be from, in Deleted. */
  from: string | null;
  /** The source the same commit takes out, which its Undo puts back. */
  removes: SkillSource | null;
  workspace: string | null;
  /** Whose pins a pin changes. */
  by?: SkillPinner | null;
  /** Where a move goes, and the grant it takes into the project's skills. */
  to?: SkillLevel | null;
  grant?: SkillGrant | null;
  /** An export as one zip, or as a folder of skill folders. */
  zip?: boolean;
  change: Change;
  undo: Change;
}) {
  const count = counted(members.length, "skill", "skills");
  if (act === "delete") {
    let ids: string[] = [];
    change(
      "The skills were not deleted",
      () =>
        commands
          .skillSetDelete(plane, [...members], from, removes?.remote ?? null)
          .then((answer) => {
            if (answer.status === "ok") ids = answer.data.deleted;
            return answer;
          }),
      async (notes) =>
        saidOf("Skills deleted", `${count} deleted. They stay in Deleted for 30 days.`, notes, () =>
          undo(
            "The delete was not undone",
            () => commands.skillSetRestore(plane, ids, removes),
            async (undone) => saidOf("Delete undone", `${count} back.`, undone),
          ),
        ),
    );
    return;
  }
  if (act === "pin" || act === "unpin") {
    if (by === null) return;
    const on = act === "pin";
    let pinned: string[] = [];
    const saidPinned = (did: string) => `${counted(pinned.length, "skill", "skills")} ${did}.`;
    change(
      on ? "The skills were not pinned" : "The skills were not unpinned",
      () =>
        commands
          .skillSetPin(
            plane,
            by,
            members.map((one) => one.name),
            on,
          )
          .then((answer) => {
            if (answer.status === "ok") pinned = answer.data.changed;
            return answer;
          }),
      async (notes) =>
        saidOf(
          on ? "Skills pinned" : "Skills unpinned",
          saidPinned(on ? "pinned" : "unpinned"),
          notes,
          () =>
            undo(
              "The change was not undone",
              () => commands.skillSetPin(plane, by, pinned, !on),
              async (undone) =>
                saidOf("Change undone", saidPinned(on ? "unpinned" : "pinned"), undone),
            ),
        ),
    );
    return;
  }
  if (act === "archive" || act === "unarchive") {
    const archiving = act === "archive";
    let was: SkillSetLifecycle[] = [];
    const saidState = (did: string) => `${counted(was.length, "skill", "skills")} ${did}.`;
    change(
      archiving ? "The skills were not archived" : "The skills were not unarchived",
      () =>
        commands
          .skillSetLifecycle(
            plane,
            members.map((member) => ({
              member,
              lifecycle: archiving ? { state: "archived" } : { state: "active" },
            })),
          )
          .then((answer) => {
            if (answer.status === "ok") was = answer.data.was;
            return answer;
          }),
      async (notes) =>
        saidOf(
          archiving ? "Skills archived" : "Skills unarchived",
          saidState(archiving ? "archived" : "unarchived"),
          notes,
          () =>
            undo(
              "The change was not undone",
              () => commands.skillSetLifecycle(plane, was),
              async (undone) =>
                saidOf(
                  "Change undone",
                  saidState(archiving ? "unarchived" : "archived again"),
                  undone,
                ),
            ),
        ),
    );
    return;
  }
  if (act === "move") {
    if (to === null) return;
    let back: SkillSetMove[] = [];
    change(
      "The skills were not moved",
      () =>
        commands
          .skillSetMove(
            plane,
            members.map((member) => ({ member, to, grant })),
          )
          .then((answer) => {
            if (answer.status === "ok") back = answer.data.back;
            return answer;
          }),
      async (notes) =>
        saidOf("Skills moved", `${count} moved.`, notes, () =>
          undo(
            "The move was not undone",
            () => commands.skillSetMove(plane, back),
            async (undone) => saidOf("Move undone", `${count} back where they were.`, undone),
          ),
        ),
    );
    return;
  }
  if (act === "export") {
    let to: string | null = null;
    change(
      "The skills were not exported",
      () =>
        commands.skillSetExport(plane, [...members], zip).then((answer) => {
          if (answer.status !== "ok") return answer;
          to = answer.data;
          return { status: "ok" as const, data: { notes: [] } };
        }),
      async () =>
        to === null ? undefined : saidOf("Skills exported", `${count} written to ${to}.`, []),
    );
    return;
  }
  if (workspace === null) return;
  const on = act === "turn-on";
  const state = on ? "on" : "off";
  let turned: string[] = [];
  const saidTurned = (to: string) =>
    `${counted(turned.length, "skill", "skills")} turned ${to} in ${workspace}.`;
  change(
    `The skills were not turned ${state}`,
    () =>
      commands
        .skillSetTurn(
          plane,
          workspace,
          members.map((one) => one.name),
          on,
        )
        .then((answer) => {
          if (answer.status === "ok") turned = answer.data.changed;
          return answer;
        }),
    async (notes) =>
      saidOf(`Skills turned ${state}`, saidTurned(state), notes, () =>
        undo(
          "The change was not undone",
          () => commands.skillSetTurn(plane, workspace, turned, !on),
          async (undone) => saidOf("Change undone", saidTurned(on ? "off" : "on"), undone),
        ),
      ),
  );
}

/** **One of those chosen that a change leaves as it is** (#5332), and why: "Built-in skills can't
 *  be changed." */
export type Stays = { name: string; label: string; why: string };

/** **A review's first line where some stay** (#5332): "12 will be deleted, 2 are built-in and
 *  stay". Those that stay for one reason are counted by it; the built-in ones by their kind. */
function staysSaid(count: number, done: string, stays: readonly Stays[]): string {
  const builtIn = stays.filter((one) => one.label === "built-in").length;
  const rest = stays.length - builtIn;
  return [
    `${count} will be ${done}`,
    builtIn > 0 && `${counted(builtIn, "is", "are")} built-in and stay${builtIn === 1 ? "s" : ""}`,
    rest > 0 &&
      `${counted(rest, "other stays", "others stay")} as ${rest === 1 ? "it is" : "they are"}`,
  ]
    .filter(Boolean)
    .join(", ");
}

/** A skill of a set's own key among the set: no two share a level and a name. */
export function memberKey(member: Pick<SkillSetMember, "level" | "name">): string {
  return `${JSON.stringify(member.level)}\u0000${member.name}`;
}

/** Whether `copy` starts ticked: turned off or on, every one; deleted, those at the level the
 *  studio is at, and never one edited since it was installed. */
export function startsTicked(act: SkillSetAct, copy: InstalledCopy, level: SkillLevel): boolean {
  return act !== "delete" || (sameLevel(copy.member.level, level) && !copy.changed);
}

/** A test's name for a skill of the list, by its level's words. */
function rowId(member: SkillSetMember): string {
  return `set-${member.label.split(" ").join("-")}-${member.name}`;
}

/** Each level's copies under its name, in the order the levels first appear. */
function byLevel(copies: readonly InstalledCopy[]) {
  const levels = new Map<string, InstalledCopy[]>();
  for (const copy of copies) {
    const group = levels.get(copy.member.label);
    if (group === undefined) levels.set(copy.member.label, [copy]);
    else group.push(copy);
  }
  return [...levels.entries()];
}

/** **The tick list**: each skill under its level, what {@link startsTicked} says ticked. */
export function SetChoosePage({
  act,
  title,
  copies,
  level,
  kept,
  onCancel,
  onReview,
}: {
  act: SkillSetAct;
  /** What the page is named: "Delete all from acme-skills". */
  title: string;
  copies: readonly InstalledCopy[];
  /** The level the studio is at. */
  level: SkillLevel;
  /** What was ticked when the person came back from the review, as they left it. */
  kept?: readonly string[];
  onCancel: () => void;
  onReview: (members: SkillSetMember[], ticked: string[]) => void;
}) {
  const [ticked, setTicked] = useState<ReadonlySet<string> | undefined>(
    kept === undefined ? undefined : new Set(kept),
  );
  const chosen =
    ticked ??
    new Set(
      copies.filter((one) => startsTicked(act, one, level)).map((one) => memberKey(one.member)),
    );
  const count = copies.filter((one) => chosen.has(memberKey(one.member))).length;
  const words = WORDS[act];
  return (
    <ReviewPage
      label={title}
      data-testid="set-choose"
      summary={words.summary}
      onCancel={onCancel}
      main={{
        children: count === 0 ? "Review" : `Review ${counted(count, "skill", "skills")}`,
        byReturn: false,
        reserve: [`Review ${counted(copies.length, "skill", "skills")}`, "Review"],
        isDisabled: count === 0,
        tooltip: count === 0 ? TICK_ONE : undefined,
        onPress: () =>
          onReview(
            copies.map((one) => one.member),
            copies.map((one) => memberKey(one.member)).filter((key) => chosen.has(key)),
          ),
      }}
    >
      <h2 className="ui-text-strong">{title}</h2>
      <p className="honest" data-testid="set-lead">
        {act === "delete"
          ? "Skills at this level are ticked, except any changed since install. Those ticked go through one review."
          : "Every skill here is ticked; untick one to leave it out. Those ticked go through one review."}
      </p>
      <List
        aria-label={title}
        selectionMode="multiple"
        selectedKeys={chosen}
        onSelectionChange={(keys) =>
          setTicked(
            new Set(
              keys === "all" ? copies.map((one) => memberKey(one.member)) : [...keys].map(String),
            ),
          )
        }
      >
        {byLevel(copies).map(([label, group]) => (
          <ListRowSection key={label} id={label} title={label} named aria-label={label}>
            {group.map((copy) => (
              <ListRow
                key={memberKey(copy.member)}
                id={memberKey(copy.member)}
                textValue={copy.member.name}
                data-testid={rowId(copy.member)}
              >
                <RowLead>
                  <Checkbox
                    slot="selection"
                    aria-label={`${words.verb} ${copy.member.name}, ${label}`}
                  />
                </RowLead>
                <RowLabel>
                  <code>{copy.member.name}</code>
                </RowLabel>
                {copy.changed && (
                  <RowTrail>
                    <Badge variant="waiting">Changed since install</Badge>
                  </RowTrail>
                )}
              </ListRow>
            ))}
          </ListRowSection>
        ))}
      </List>
    </ReviewPage>
  );
}

/** **The one review**: see the file's head. `onApply` gets the skills to change. */
export function SetReviewPage({
  plane,
  act,
  title,
  named,
  ticked,
  workspace,
  to = null,
  alsoRemoves,
  stays = [],
  fields,
  main,
  unready,
  busy,
  onCancel,
  onApply,
}: {
  plane: string;
  act: SkillSetAct;
  title: string;
  /** Every skill the review may hold, in the list's order. */
  named: readonly SkillSetMember[];
  /** Those ticked, by {@link memberKey}. */
  ticked: readonly string[];
  /** The workspace a turn is in. */
  workspace: string | null;
  /** The level a move goes to (#5332), once one is picked. */
  to?: SkillLevel | null;
  /** The source the same commit takes out, as it is named. */
  alsoRemoves?: string;
  /** Those chosen that this change leaves as they are, each with why (#5332): a built-in skill,
   *  one off already. */
  stays?: readonly Stays[];
  /** What the change asks before it is made, under its title: where a move goes (#5332). */
  fields?: ReactNode;
  /** What the main action says, where it is not the verb: "Choose where to save…". */
  main?: string;
  /** Why the main action cannot be pressed yet, where {@link fields} still asks something. */
  unready?: string;
  busy: boolean;
  onCancel: () => void;
  onApply: (members: SkillSetMember[]) => void;
}) {
  const [all, setAll] = useState<readonly SkillSetMember[]>(named);
  const [keys, setKeys] = useState<ReadonlySet<string>>(new Set(ticked));
  const going = all.filter((one) => keys.has(memberKey(one)));
  const { read, again } = useRead<Review>(
    going.length === 0 || (act === "move" && to === null)
      ? undefined
      : `${act}\u0000${to === null ? "" : JSON.stringify(to)}\u0000${[...keys].join("\u0001")}`,
    () => commands.skillSetReview(plane, act, going, workspace, to),
  );
  const review = read?.data;
  const words = WORDS[act];
  const cannot = review?.skills.filter((one) => one.cannot !== null) ?? [];
  const can = going.filter(
    (one) => !cannot.some((other) => memberKey(other.member) === memberKey(one)),
  );
  const leftOut = all.filter((one) => !keys.has(memberKey(one)));
  const tick = (key: string, on: boolean) =>
    setKeys((was) => {
      const next = new Set(was);
      if (on) next.add(key);
      else next.delete(key);
      return next;
    });
  const why =
    going.length === 0
      ? TICK_ONE
      : unready !== undefined
        ? unready
        : read === undefined
          ? "The review is being read."
          : review === undefined
            ? "The review could not be read."
            : can.length === 0
              ? "None of these can change."
              : busy
                ? BUSY
                : undefined;
  return (
    <ReviewPage
      label={title}
      data-testid="set-review"
      summary={words.summary}
      onCancel={onCancel}
      main={{
        children: main ?? words.verb,
        ...(act === "delete" ? { variant: "danger" as const } : { byReturn: false }),
        isDisabled: why !== undefined,
        tooltip: why,
        onPress: () => onApply(can),
      }}
    >
      <h2 className="ui-text-strong">{title}</h2>
      {fields}
      <p className="honest" data-testid="set-review-lead">
        {can.length === 0
          ? "Nothing is left in this change."
          : stays.length > 0
            ? `${staysSaid(can.length, words.done, stays)}. ${words.does(workspace)}`
            : `${counted(can.length, "skill", "skills")} in this change. ${words.does(workspace)}`}
        {can.length > 0 &&
          alsoRemoves !== undefined &&
          ` ${alsoRemoves} stops being a source in the same change.`}
      </p>
      {going.length > 0 && (
        <Held read={read} what="the review of these skills" again={again}>
          {(data) => (
            <>
              {data.open_chats > 0 && (
                <p className="honest" data-testid="set-open-chats">
                  {counted(data.open_chats, "open chat uses", "open chats use")}{" "}
                  {can.length === 1 ? "this one" : "these"}. Each keeps what it started with until
                  it restarts, and offers a restart once.
                </p>
              )}
              {data.names.map((one) => (
                <div
                  key={`${one.namer}\u0000${one.named}`}
                  className="skill-review"
                  data-testid={`set-names-${one.namer}-${one.named}`}
                >
                  <p className="honest">
                    <code>{one.namer}</code> names <code>{one.named}</code>, {words.doing}.
                  </p>
                  <ButtonRow size="sm">
                    <Button
                      size="sm"
                      isDisabled={busy}
                      onPress={() => {
                        const key = memberKey(one.member);
                        if (act === "turn-on") {
                          setAll((was) =>
                            was.some((other) => memberKey(other) === key)
                              ? was
                              : [...was, one.member],
                          );
                          tick(key, true);
                        } else tick(key, false);
                      }}
                    >
                      {act === "turn-on" ? `Also turn on ${one.named}` : `Keep ${one.named}`}
                    </Button>
                  </ButtonRow>
                </div>
              ))}
              {can.length > 0 && (
                <section
                  aria-labelledby="set-going"
                  className="skill-review"
                  data-testid="set-going-list"
                >
                  <h3 id="set-going" className="ui-text-small ui-text-strong">
                    In this change
                  </h3>
                  {data.skills
                    .filter((one) => one.cannot === null)
                    .map((one) => {
                      const key = memberKey(one.member);
                      return (
                        <Checkbox
                          key={key}
                          isSelected
                          isDisabled={busy}
                          onChange={() => tick(key, false)}
                          data-testid={`set-keep-${one.member.name}`}
                          description={one.changed ? "Changed since install" : undefined}
                        >
                          {words.verb} {one.member.name} ({one.member.label})
                        </Checkbox>
                      );
                    })}
                </section>
              )}
              {cannot.length > 0 && (
                <section aria-labelledby="set-cannot" className="skill-review">
                  <h3 id="set-cannot" className="ui-text-small ui-text-strong">
                    Taken out of this change
                  </h3>
                  {cannot.map((one) => (
                    <p
                      key={memberKey(one.member)}
                      className="honest"
                      data-testid={`set-cannot-${one.member.name}`}
                    >
                      <code>{one.member.name}</code> ({one.member.label}): {one.cannot}
                    </p>
                  ))}
                </section>
              )}
            </>
          )}
        </Held>
      )}
      {stays.length > 0 && (
        <section aria-labelledby="set-stays" className="skill-review" data-testid="set-stays">
          <h3 id="set-stays" className="ui-text-small ui-text-strong">
            Stay as they are
          </h3>
          {stays.map((one) => (
            <p
              key={`${one.name}\u0000${one.label}`}
              className="honest"
              data-testid={`set-stays-${one.name}`}
            >
              <code>{one.name}</code> ({one.label}): {one.why}
            </p>
          ))}
        </section>
      )}
      {leftOut.length > 0 && (
        <section aria-labelledby="set-left-out" className="skill-review">
          <h3 id="set-left-out" className="ui-text-small ui-text-strong">
            Left out of this change
          </h3>
          {leftOut.map((one) => (
            <Checkbox
              key={memberKey(one)}
              isSelected={false}
              isDisabled={busy}
              onChange={() => tick(memberKey(one), true)}
              data-testid={`set-left-out-${one.name}`}
            >
              {words.verb} {one.name} ({one.label})
            </Checkbox>
          ))}
        </section>
      )}
    </ReviewPage>
  );
}
