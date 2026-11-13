// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useState } from "react";
import {
  commands,
  type SkillGained,
  type SkillSetMember,
  type SkillSetUpdating,
  type SkillUpdateOffer,
  type UpdateReview,
} from "./bindings";
import { counted } from "./dispatches";
import { capitalised } from "./chatEnded";
import { Changes, LOOK_CLOSELY, Summary } from "./SkillChangesShown";
import type { Change } from "./SkillPage";
import { BUSY, Held, useRead } from "./SkillRead";
import { memberKey, saidOf, type Stays } from "./SkillSetReview";
import { sameLevel } from "./skillChanges";
import { reviewUpdate } from "./studioSources";
import { Checkbox } from "./ui/choice/Checkbox";
import { Fact, Facts } from "./ui/card/Facts";
import { Disclosure } from "./ui/disclosure/Disclosure";
import { ReviewPage } from "./ui/review-page/ReviewPage";

/**
 * **Review updates for several skills** (#5354), from the Library's Select mode and from a
 * source's page: one page of every chosen skill's update, those that add something new first,
 * then one commit that fully applies or changes nothing, with one Undo. Chats keep the version
 * they have until it is accepted.
 *
 * **The security ruling** (operator, 2026-11-05): an update that only edits a skill's text starts
 * ticked. One that newly adds a script, a hook, a tool, network access or anything else the core
 * reads as new ({@link SkillGained}) starts unticked, and its box can be ticked only once its
 * changes have been opened. Nothing ticks them all at once, and the core refuses an update that
 * gains a kind it was not ticked with.
 */

/** What a skill newly gains, as the end of "Adds …". */
const GAINED_SAID: Record<SkillGained, string> = {
  script: "a new or changed script",
  command: "a command it runs as it loads",
  hook: "a hook",
  tool: "a tool it pre-approves",
  network: "network access",
  "writes-outside": "a write outside the workspace",
  hidden: "characters no reader sees",
  "not-shown": "a file the review can't show",
  unread: "a frontmatter that can't be read",
};

/** "Adds a hook and network access." */
export function gainedSaid(gained: readonly SkillGained[]): string {
  const words = gained.map((one) => GAINED_SAID[one]);
  const listed =
    words.length <= 1
      ? (words[0] ?? "")
      : `${words.slice(0, -1).join(", ")} and ${words[words.length - 1]}`;
  return `Adds ${listed}.`;
}

/** One skill whose update is offered, with its review. */
export type UpdateRead = {
  member: SkillSetMember;
  offer: SkillUpdateOffer;
  review: UpdateReview;
};

/** What the page reads: each update and its review, and each chosen skill with none, and why. */
export type SetUpdates = { ready: UpdateRead[]; none: Stays[] };

/** An update's own key on the page: its skill, the commit it goes to and what it gains. A read
 *  again that finds it gaining more is a new row, unticked and unopened. */
function rowKey(update: UpdateRead): string {
  return [memberKey(update.member), update.review.newer.id, ...update.review.gained].join("\u0001");
}

/** Whether an update starts ticked: only one that adds nothing new. */
export function startsTicked(update: UpdateRead): boolean {
  return update.review.gained.length === 0;
}

/** **Reads the updates of `members`**: the updates there are, then each one's review. */
export async function readSetUpdates(
  plane: string,
  members: readonly SkillSetMember[],
): Promise<{ status: "ok"; data: SetUpdates } | { status: "error"; error: string }> {
  const updates = await commands.skillUpdates(plane);
  if (updates.status === "error") return updates;
  const { offers, unasked, unconfirmed } = updates.data;
  const none: Stays[] = [];
  const asked: Promise<UpdateRead | undefined>[] = [];
  for (const member of members) {
    const offer = offers.find(
      (one) => one.name === member.name && sameLevel(one.level, member.level),
    );
    const stays = (why: string) => none.push({ name: member.name, label: member.label, why });
    if (offer === undefined) {
      const not = unasked.find((one) => one.name === member.name && one.label === member.label);
      if (not !== undefined) stays(`${capitalised(not.why)}.`);
      else if (unconfirmed.some((one) => one.skills.includes(member.name)))
        stays("Its source isn't checked on this machine until you confirm it on its page.");
      else stays("No update is waiting.");
      continue;
    }
    // An update from an extension's source is reviewed through it, and the core holds what that
    // review read for the accept to copy (#5760).
    asked.push(
      reviewUpdate(plane, offer).then((review) => {
        if (review.status === "ok") return { member, offer, review: review.data };
        stays(`Its update could not be read: ${review.error}`);
        return undefined;
      }),
    );
  }
  const ready = (await Promise.all(asked)).filter((one) => one !== undefined);
  return { status: "ok", data: { ready, none } };
}

/** **The change the review asks**: one commit, and one Undo right after. */
export function applySetUpdate({
  plane,
  updates,
  change,
  undo,
}: {
  plane: string;
  updates: readonly SkillSetUpdating[];
  change: Change;
  undo: Change;
}) {
  const count = counted(updates.length, "skill", "skills");
  let id: number | undefined;
  change(
    "The skills were not updated",
    () =>
      commands.skillSetUpdate(plane, [...updates]).then((answer) => {
        if (answer.status === "ok") id = answer.data.undo;
        return answer;
      }),
    async (notes) =>
      saidOf("Skills updated", `${count} updated.`, notes, () => {
        if (id === undefined) return;
        const held = id;
        undo(
          "The update was not undone",
          () => commands.skillSetUpdateUndo(plane, held),
          async (undone) => saidOf("Update undone", `${count} back as they were.`, undone),
        );
      }),
  );
}

/** A test's name for an update's row. */
function rowId(member: SkillSetMember): string {
  return `update-set-${member.label.split(" ").join("-")}-${member.name}`;
}

/** **One update**: its box, then what changes, folded. A box of one that adds something new
 *  waits until its changes are opened. */
function UpdateRow({
  update,
  ticked,
  opened,
  busy,
  onTick,
  onOpen,
}: {
  update: UpdateRead;
  ticked: boolean;
  opened: boolean;
  busy: boolean;
  onTick: (on: boolean) => void;
  onOpen: () => void;
}) {
  const { member, review } = update;
  const gains = review.gained.length > 0;
  const waits = gains && !opened;
  const description = gains
    ? `${gainedSaid(review.gained)}${waits ? " Open what changes to tick it." : ""}`
    : "Edits its text only.";
  return (
    <div className="skill-review" data-testid={rowId(member)}>
      <Checkbox
        isSelected={ticked}
        isDisabled={busy || waits}
        onChange={onTick}
        description={description}
        data-testid={`${rowId(member)}-box`}
      >
        Update {member.name} ({member.label})
      </Checkbox>
      <Disclosure
        size="sm"
        label={`What changes in ${member.name}`}
        trail={gains ? LOOK_CLOSELY : undefined}
        onExpandedChange={(open) => {
          if (open) onOpen();
        }}
        data-testid={`${rowId(member)}-changes`}
      >
        <Facts aria-label={`The update of ${member.name}`}>
          <Fact label="Pinned now">
            <code>{review.pinned.short}</code>
          </Fact>
          <Fact label="Newer">
            <code>{review.newer.short}</code>
          </Fact>
        </Facts>
        {!review.added.empty && (
          <>
            <h4 className="ui-text-small ui-text-strong">New in what it adds</h4>
            <Summary summary={review.added} />
          </>
        )}
        {!review.removed.empty && (
          <>
            <h4 className="ui-text-small ui-text-strong">Gone from what it adds</h4>
            <Summary summary={review.removed} />
          </>
        )}
        {review.brought.map((skill) => (
          <p key={skill.name} className="honest">
            <code>{skill.name}</code> comes along, new here.
          </p>
        ))}
        <Changes files={review.files} />
        {review.edited.length > 0 && (
          <p className="honest">
            Accepting replaces your edits since the install: {review.edited.join(", ")}
          </p>
        )}
      </Disclosure>
    </div>
  );
}

/** **The review**: see the file's head. `onApply` gets the updates ticked, each with what it
 *  was ticked with. */
export function SetUpdateReviewPage({
  plane,
  title,
  members,
  stays = [],
  busy,
  onCancel,
  onApply,
}: {
  plane: string;
  title: string;
  /** The skills whose updates are asked for, in the list's order. */
  members: readonly SkillSetMember[];
  /** Those chosen that the review leaves out before anything is read, each with why. */
  stays?: readonly Stays[];
  busy: boolean;
  onCancel: () => void;
  onApply: (updates: SkillSetUpdating[]) => void;
}) {
  const { read, again } = useRead(
    members.length === 0 ? undefined : members.map(memberKey).join("\u0001"),
    () => readSetUpdates(plane, members),
  );
  const [ticks, setTicks] = useState<ReadonlyMap<string, boolean>>(new Map());
  const [opened, setOpened] = useState<ReadonlySet<string>>(new Set());
  const ready = read?.data?.ready ?? [];
  // Those that add something new first, each group in the list's order.
  const rows = [
    ...ready.filter((one) => !startsTicked(one)),
    ...ready.filter((one) => startsTicked(one)),
  ];
  const isTicked = (one: UpdateRead) => ticks.get(rowKey(one)) ?? startsTicked(one);
  const going = rows.filter(isTicked);
  const waiting = rows.filter((one) => !startsTicked(one)).length;
  const why =
    read === undefined
      ? "The updates are being read."
      : read.data === undefined
        ? "The updates could not be read."
        : rows.length === 0
          ? "None of these has an update waiting."
          : going.length === 0
            ? "Tick at least one update."
            : busy
              ? BUSY
              : undefined;
  const left = [...stays, ...(read?.data?.none ?? [])];
  return (
    <ReviewPage
      label={title}
      data-testid="update-set-review"
      summary="Chats keep the current version until you accept."
      onCancel={onCancel}
      cancel={{ children: "Not now" }}
      main={{
        children:
          going.length === 0
            ? "Accept updates"
            : `Accept ${counted(going.length, "update", "updates")}`,
        reserve: [`Accept ${counted(Math.max(rows.length, 2), "update", "updates")}`],
        byReturn: false,
        isDisabled: why !== undefined,
        tooltip: why,
        onPress: () =>
          onApply(
            going.map((one) => ({
              level: one.offer.level,
              name: one.offer.name,
              reviewed: one.review.accepted,
              grant: one.review.grant,
              allowed: one.review.gained,
            })),
          ),
      }}
    >
      <h2 className="ui-text-strong">{title}</h2>
      <Held read={read} what="the updates of these skills" again={again}>
        {() => (
          <>
            <p className="honest" data-testid="update-set-lead">
              {rows.length === 0
                ? "Nothing here has an update waiting."
                : [
                    `${counted(rows.length, "update", "updates")} waiting.`,
                    waiting > 0 &&
                      `${counted(waiting, "adds", "add")} something new and ${waiting === 1 ? "starts" : "start"} unticked: open ${waiting === 1 ? "it" : "each"} to tick it.`,
                    "It is one change, and Undo is offered right after.",
                  ]
                    .filter(Boolean)
                    .join(" ")}
            </p>
            {rows.length > 0 && (
              <section
                aria-labelledby="update-set-going"
                className="skill-review"
                data-testid="update-set-list"
              >
                <h3 id="update-set-going" className="ui-text-small ui-text-strong">
                  Updates
                </h3>
                {rows.map((one) => {
                  const key = rowKey(one);
                  return (
                    <UpdateRow
                      key={key}
                      update={one}
                      ticked={isTicked(one)}
                      opened={opened.has(key)}
                      busy={busy}
                      onTick={(on) => setTicks((was) => new Map(was).set(key, on))}
                      onOpen={() => setOpened((was) => new Set(was).add(key))}
                    />
                  );
                })}
              </section>
            )}
          </>
        )}
      </Held>
      {left.length > 0 && (
        <section
          aria-labelledby="update-set-stays"
          className="skill-review"
          data-testid="set-stays"
        >
          <h3 id="update-set-stays" className="ui-text-small ui-text-strong">
            Stay as they are
          </h3>
          {left.map((one) => (
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
    </ReviewPage>
  );
}
