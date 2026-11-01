// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useId, useState } from "react";
import { commands, type GrantLevel, type PlaneId, type SeenBlock, type Shown } from "./bindings";
import { Notice, type NoticeAction } from "./Notice";
import { sandboxCommandReturned } from "./sandboxAsked";
import { RestartEndsLine } from "./RestartEnds";
import { asksOf, inertly } from "./sandboxBlocks";
import { SETTLE_MS } from "./noticeSettle";
import { listed, type Member, type TaskBlockGroup } from "./taskAsks";
import { Button, ButtonRow } from "./ui/button/Button";

// How long a press on a question just formed or changed does nothing: `noticeSettle.ts` holds
// it, beside the Notice's own settle, so a Notice imports nothing of this file's.
export { SETTLE_MS };

/**
 * The time a list is drawn at. Read in the render that draws a changed list, once per change
 * (the state it is kept in changes with it), so that render and every press on it know it.
 */
const drawnAt = () => performance.now();

/** The time a press lands at, read in the handler that answers it. */
const pressedAt = () => performance.now();

/** What the window sends back for each task: the block it showed for it, exactly. */
function seenOf(members: readonly Member[]): SeenBlock[] {
  return members.map((one) => ({
    task: one.session,
    operation: one.block.operation,
    kind: one.block.kind,
    what: one.block.offer === "write" ? "write" : "host",
    target: one.block.target ?? "",
  }));
}

/**
 * **One question for the tasks of one session that hit the same block** (#1508, V100-57), on
 * the session's tab: "3 tasks want to reach registry.npmjs.org", each task named by its whole
 * path, and one answer that applies to each task listed and to no other chat.
 *
 * - **Its scopes in the single-chat Notice's order** (#1709): for a host, the main button allows
 *   it for this project on this machine, and **Other scopes…** offers only these tasks, or
 *   everyone in the project; for a folder, only these tasks first, then **Always allow…** for
 *   this project on this machine. "Only these tasks" grants each task listed its own grant
 *   ("this chat" for each of them), never the session that asked them: a session's permission
 *   and its tasks' are apart. A wider scope keeps one grant for every chat here, as a block's own
 *   does. Every task listed that is not held takes it at its restart. **Keep blocked** answers
 *   each task listed. Only the scopes policy leaves open are offered (#1343).
 * - **What it says** (#1709), as the single-chat Notice does: a task whose connection is held
 *   while the person answers (`held`) is said to wait, and an Allow lets its command carry on;
 *   what an administrator's policy ruled out here is said (`ruled`).
 * - **An answer is to what was shown** (D-1508-9). Every press sends each task with the block
 *   shown for it, and the core refuses the whole answer unless each task is held on exactly
 *   that block now and is recorded as a task of this session. A task that hits the same block
 *   later joins the question visibly ("probe joined this question"). For {@link SETTLE_MS}
 *   after the question forms or changes, a press does nothing and says why.
 */
export function TaskBlocksNotice({
  plane,
  group,
  asks,
  onAnswered,
  onKeepBlocked,
}: {
  plane: PlaneId;
  group: TaskBlockGroup;
  /**
   * **The project's asks, as the registry lists them** (#1695): each task's block is an ask of
   * its own there. The question is drawn while the registry lists any of them, and its wider
   * answers say the registry's words (#1700); its narrowest, "only these tasks", is its own,
   * since it grants each task listed and no registry ask is several tasks'. Nothing before the
   * registry is first read.
   */
  asks?: readonly Shown[];
  /** The core answered `members` (all those listed, or those it allowed before keeping failed
   *  part way): they are owed a restart, and the core said `said`. */
  onAnswered: (members: readonly Member[], said: string) => void;
  /** The core let `members`' blocks go, kept blocked. */
  onKeepBlocked: (members: readonly Member[]) => void;
}) {
  const id = useId();
  const [alwaysOpen, setAlways] = useState(false);
  const [said, setSaid] = useState<string>();
  const [busy, setBusy] = useState(false);
  const count = group.members.length;
  const listing = group.members.map((one) => `${one.session}:${one.block.target ?? ""}`).join(",");
  /**
   * **When this list was drawn**, set in the very render that draws it, so no press can land on
   * a list whose time is not yet known: at the question's forming, and at every change of who
   * it lists, with who joined. React's way to keep what a previous render showed.
   */
  const [drawn, setDrawn] = useState(() => ({
    listing,
    at: drawnAt(),
    sessions: new Set(group.members.map((one) => one.session)),
    joined: [] as string[],
  }));
  if (drawn.listing !== listing) {
    setDrawn({
      listing,
      at: drawnAt(),
      sessions: new Set(group.members.map((one) => one.session)),
      joined: group.members
        .filter((one) => !drawn.sessions.has(one.session))
        .map((one) => one.whose),
    });
    setSaid(undefined);
  }
  const joined = drawn.joined;
  /** Whether a press now is too soon after what it is on was drawn. */
  const tooSoon = () => {
    if (pressedAt() - drawn.at < SETTLE_MS) {
      setSaid(
        "This question was drawn or changed just now, so nothing was answered. Read the tasks " +
          "it lists and answer again.",
      );
      return true;
    }
    return false;
  };

  const allow = (level: GrantLevel) => {
    if (busy || tooSoon()) return;
    const members = group.members;
    setBusy(true);
    setSaid(undefined);
    void commands
      .allowSandboxBlockForTasks(plane, group.session, seenOf(members), level)
      .then((done) => {
        if (done.status === "error") setSaid(done.error);
        else
          onAnswered(
            members.filter((one) => done.data.answered.includes(one.session)),
            done.data.said,
          );
      })
      .catch((err: unknown) => setSaid(`purlis could not allow it: ${String(err)}`))
      .finally(() => {
        setBusy(false);
        sandboxCommandReturned();
      });
  };
  const keepBlocked = () => {
    if (busy || tooSoon()) return;
    const members = group.members;
    setBusy(true);
    setSaid(undefined);
    void commands
      .keepSandboxBlockForTasks(plane, group.session, seenOf(members), null)
      .then((done) => {
        if (done.status === "error") setSaid(done.error);
        else onKeepBlocked(members.filter((one) => done.data.includes(one.session)));
      })
      .catch((err: unknown) => setSaid(`purlis could not keep it blocked: ${String(err)}`))
      .finally(() => setBusy(false));
  };
  /** The registry's asks for the tasks listed: what the answers read from. */
  const its = group.members.flatMap((one) => asksOf(one.block, asks ?? []));
  const offered = its[0]?.options ?? [];
  const allowsAt = (level: GrantLevel) => group.levels.includes(level);
  /** The main button, and the rest under a menu, in the registry's order (the single-chat
   *  Notice's), at the levels policy leaves open for every task. */
  const open = offered
    .map((option) => option.id)
    .filter((id): id is GrantLevel => (["chat", "you", "project"] as string[]).includes(id))
    .filter(allowsAt);
  const main = open[0];
  const others = open.slice(1);
  const labelOf = (id: string) => offered.find((option) => option.id === id)?.label ?? "";
  const scope: Readonly<Record<GrantLevel, string>> = {
    once: labelOf("once"),
    you: labelOf("you"),
    machine: labelOf("machine"),
    chat: `Allow only for these ${count} tasks`,
    project: labelOf("project"),
  };
  const keepSaid = offered.find((option) => option.id === "keep")?.label;
  // The folder or host as the tasks' sandbox met it, drawn safely (I-1): a task named it.
  const target = <code className="block-allow-target">{inertly(group.target)}</code>;
  /** The tasks whose connection is held while the person answers (#1666): set by the app. */
  const held = group.members.filter((one) => one.block.held);
  /** What policy ruled out here, once each (#1666). */
  const ruled = [
    ...new Set(group.members.flatMap((one) => (one.block.ruled ? [one.block.ruled] : []))),
  ];

  const under = (
    <div className="block-allow" id={id}>
      <ul aria-label="Tasks this answers">
        {group.members.map((one) => (
          <li key={one.session}>{one.whose}</li>
        ))}
      </ul>
      {/* A folder reaches each task only by its restart (#1986): what that ends, each said. */}
      {group.offer === "write" && (
        <ul aria-label="What restarting each task ends">
          {group.members.map((one) => (
            <li key={one.session}>
              <RestartEndsLine plane={plane} session={one.session} chat={one.whose} />
            </li>
          ))}
        </ul>
      )}
      {alwaysOpen && (
        <ButtonRow size="sm" className="block-allow-actions">
          {others.map((level) => (
            <Button
              size="sm"
              variant="grants"
              key={level}
              isDisabled={busy}
              onPress={() => allow(level)}
            >
              {scope[level]}
            </Button>
          ))}
        </ButtonRow>
      )}
    </div>
  );
  // Listed nowhere: every task's block was answered, or the registry is not read yet.
  if (keepSaid === undefined) return null;
  const keep: NoticeAction = { label: keepSaid, onPress: keepBlocked };
  const allows: NoticeAction[] = [
    ...(main !== undefined
      ? [{ label: scope[main], onPress: () => allow(main), allows: true }]
      : []),
    ...(others.length > 0
      ? [
          {
            label: group.offer === "host" ? "Other scopes…" : "Always allow…",
            onPress: () => setAlways((was) => !was),
            opens: { id, open: alwaysOpen },
          },
        ]
      : []),
  ];
  const [first, ...rest] = [...allows, keep];
  const fixes: readonly [NoticeAction, ...NoticeAction[]] = [first ?? keep, ...rest];
  /** What allowing a held folder means, which its block says. */
  const means = group.members[0]?.block.route;
  return (
    <Notice
      cause={`sandbox-blocked-tasks:${group.session}:${group.key}`}
      at="pane"
      tone="trouble"
      label={`Sandbox block for ${count} tasks`}
      fixes={fixes}
      under={under}
    >
      {count} tasks want to {group.offer === "host" ? "reach " : "write "}
      {target}
      {group.offer === "write" && " and everything in it"}:{" "}
      {listed(group.members.map((one) => one.whose))}.{" "}
      {group.offer === "write" && means != null && `${means} `}
      {held.length === 0 ? (
        <>The sandbox blocked {group.said} in each.</>
      ) : (
        <>
          {held.length === count
            ? "Each one's command is waiting"
            : `${listed(held.map((one) => one.whose))} ${held.length === 1 ? "is" : "are"} waiting`}{" "}
          on {group.said}: purlis holds the connection while you answer, and an Allow lets the same
          command carry on. If nobody answers within a minute it is refused, and an Allow after that
          tells the task to run it again.
          {held.length < count && ` The sandbox blocked it in the others.`}
        </>
      )}{" "}
      One answer applies to each task listed, and to no other chat.
      {ruled.length > 0 && ` ${ruled.join(" ")}`}
      {joined.length > 0 && ` ${listed(joined)} joined this question after it was first shown.`}
      {said !== undefined && ` ${said}`}
    </Notice>
  );
}

/** What one answer to several tasks allowed, until it is put away. */
export function TaskBlocksAnswered({
  target,
  said,
  onDismiss,
}: {
  target: string;
  said: string;
  onDismiss: () => void;
}) {
  return (
    <Notice
      cause={`sandbox-blocked-tasks-allowed:${target}`}
      at="pane"
      tone="news"
      label="Sandbox block"
      onDismiss={onDismiss}
    >
      <code className="block-allow-target">{target}</code>: {said}
    </Notice>
  );
}
