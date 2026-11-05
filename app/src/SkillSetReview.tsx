// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useState } from "react";
import {
  commands,
  type InstalledCopy,
  type SkillLevel,
  type SkillSetAct,
  type SkillSetMember,
  type SkillSetReview as Review,
} from "./bindings";
import { counted } from "./dispatches";
import { sameLevel } from "./skillChanges";
import { Held, useRead } from "./SkillSources";
import { Badge } from "./ui/badge/Badge";
import { Button } from "./ui/button/Button";
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
 *   naming one that goes, which does not block and offers to take that skill in too; how many open
 *   chats keep them until they restart; and each skill that cannot apply, taken out with its
 *   reason before anything is written.
 */

/** What a set's words are, by what is asked of it. */
const WORDS: Record<
  SkillSetAct,
  {
    /** On its main action: "Delete 3 skills". */
    verb: string;
    /** After a name: "which you're deleting". */
    doing: string;
    /** On the warning's offer: "Also delete to-spec". */
    also: string;
    /** On the box of each skill. */
    box: string;
    /** At the foot, before the main action. */
    summary: string;
    /** What stands in the way when none is left. */
    none: string;
  }
> = {
  delete: {
    verb: "Delete",
    doing: "which you're deleting",
    also: "Also delete",
    box: "Delete",
    summary: "Nothing is deleted until you press Delete.",
    none: "Tick at least one skill.",
  },
  "turn-off": {
    verb: "Turn off",
    doing: "which you're turning off",
    also: "Also turn off",
    box: "Turn off",
    summary: "Nothing is turned off until you press Turn off.",
    none: "Tick at least one skill.",
  },
  "turn-on": {
    verb: "Turn on",
    doing: "which stays off",
    also: "Also turn on",
    box: "Turn on",
    summary: "Nothing is turned on until you press Turn on.",
    none: "Tick at least one skill.",
  },
};

/** What the review's lead says happens. */
function leadOf(act: SkillSetAct, workspace: string | null, alsoRemoves: string | undefined) {
  const where = workspace === null ? "" : ` in ${workspace}`;
  switch (act) {
    case "delete":
      return `They go to Deleted for 30 days, as one commit, and Undo is offered right after.${alsoRemoves === undefined ? "" : ` ${alsoRemoves} stops being a source in the same commit.`}`;
    case "turn-off":
      return `They turn off${where} as one commit, and Undo is offered right after.`;
    case "turn-on":
      return `They turn on${where} as one commit, and Undo is offered right after.`;
  }
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
        isDisabled: count === 0,
        tooltip: count === 0 ? words.none : undefined,
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
                    aria-label={`${words.box} ${copy.member.name}, ${label}`}
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
  alsoRemoves,
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
  /** The source the same commit takes out, as it is named. */
  alsoRemoves?: string;
  busy: boolean;
  onCancel: () => void;
  onApply: (members: SkillSetMember[]) => void;
}) {
  const [all, setAll] = useState<readonly SkillSetMember[]>(named);
  const [keys, setKeys] = useState<ReadonlySet<string>>(new Set(ticked));
  const going = all.filter((one) => keys.has(memberKey(one)));
  const { read, again } = useRead<Review>(
    going.length === 0 ? undefined : `${act}\u0000${[...keys].join("\u0001")}`,
    () => commands.skillSetReview(plane, act, going, workspace),
  );
  const review = read?.data;
  const words = WORDS[act];
  const cannot = review?.skills.filter((one) => one.cannot !== null) ?? [];
  const can = going.filter(
    (one) => !cannot.some((other) => memberKey(other.member) === memberKey(one)),
  );
  const leftOut = all.filter((one) => !keys.has(memberKey(one)));
  const set = (key: string, on: boolean) =>
    setKeys((was) => {
      const next = new Set(was);
      if (on) next.add(key);
      else next.delete(key);
      return next;
    });
  const why =
    going.length === 0
      ? words.none
      : read === undefined
        ? "The review is being read."
        : review === undefined
          ? "The review could not be read."
          : can.length === 0
            ? "None of these can change."
            : busy
              ? "Another change is being made."
              : undefined;
  const main = `${words.verb} ${counted(can.length, "skill", "skills")}`;
  return (
    <ReviewPage
      label={title}
      data-testid="set-review"
      summary={words.summary}
      onCancel={onCancel}
      main={{
        children: main,
        ...(act === "delete" ? { variant: "danger" as const } : { byReturn: false }),
        isDisabled: why !== undefined,
        tooltip: why,
        onPress: () => onApply(can),
      }}
    >
      <h2 className="ui-text-strong">{title}</h2>
      <p className="honest" data-testid="set-review-lead">
        {leadOf(act, workspace, alsoRemoves)}
      </p>
      {going.length > 0 && (
        <Held read={read} what="the review of these skills" again={again}>
          {(data) => (
            <>
              {data.open_chats > 0 && (
                <p className="honest" data-testid="set-open-chats">
                  {counted(data.open_chats, "open chat uses", "open chats use")} these. They keep
                  the skills until they restart, and each offers a restart once.
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
                  <Button
                    size="sm"
                    isDisabled={busy}
                    onPress={() => {
                      const key = memberKey(one.add);
                      setAll((was) =>
                        was.some((other) => memberKey(other) === key) ? was : [...was, one.add],
                      );
                      set(key, true);
                    }}
                  >
                    {words.also} {one.add.name}
                  </Button>
                </div>
              ))}
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
                        onChange={() => set(key, false)}
                        data-testid={`set-keep-${one.member.name}`}
                        description={one.changed ? "Changed since install" : undefined}
                      >
                        {words.box} {one.member.name} ({one.member.label})
                      </Checkbox>
                    );
                  })}
              </section>
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
              onChange={() => set(memberKey(one), true)}
              data-testid={`set-left-out-${one.name}`}
            >
              {words.box} {one.name} ({one.label})
            </Checkbox>
          ))}
        </section>
      )}
    </ReviewPage>
  );
}
