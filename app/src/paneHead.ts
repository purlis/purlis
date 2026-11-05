// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import {
  activityId,
  answerId,
  askerId,
  briefId,
  chatHarnessId,
  chatSkillsId,
  chatStudioId,
  chatWorkspaceId,
  smartCloseCancelId,
  stopId,
} from "./actions";
import type { ListedChat } from "./chatsTree";
import { SAID } from "./menuWords";
import type { SandboxSaid } from "./SandboxMark";

/**
 * **The pane head's model** (#4760, spec #4750 stories 54, 55 and 61; `command-table.md` of
 * #4585, rule 4), **and the pane foot's** (#5202, spec #5191): which one of a chat's rows the
 * head shows as its state button, which parts each of the pane's two lines draws, and which a
 * narrow pane drops. Pure, so the rules are read and tested here, and the pane
 * (`LayoutPanes.PaneFrame`) only draws what these answer.
 */

/** **What a chat's pane head says of it** (#4760): the chat as the Chats list lists it (its
 *  name, harness, workspace and kind), its sandbox, and whether it is wrapping up. */
export type ChatHead = {
  chat: ListedChat;
  sandbox: SandboxSaid | null;
  /** Being smart-closed (ADR 0064): the head breathes, and its state button is Cancel. */
  wrapping: boolean;
  /**
   * **Filed in another workspace than its tab's** (#4982; #4583 rule 2, `tabs.filedElsewhere`):
   * its head names that workspace, as the window words it, with a badge in the workspace's
   * colour, or `null` for one with none. Absent for a chat of the tab's own workspace.
   */
  elsewhere?: { workspace: string; colour: string | null };
};

/** The one state button: a row of the chat's own list, and the short word the head says. */
export type StateButton = { id: string; word: string };

/**
 * **The one state button a pane head shows** (variant A of #4585, accepted 2026-11-02), chosen
 * by the chat's state, first that holds:
 *
 * 1. **Cancel smart close** while it wraps up (ADR 0064): its chip on the strip is gone, and
 *    this is where it is stopped from.
 * 2. **Answer** while a task asks.
 * 3. **Brief** on any other task. A task's own stop is Stop and get its report, which is in its
 *    list (the head's Stop and Close now buttons became ⋯ rows), so a working task shows Brief
 *    as the prototype does.
 * 4. **Stop** while a chat or a shell works.
 * 5. **Nothing** on a quiet chat.
 *
 * `has` is whether the chat's list has the row now: the button is that row, so it is never one
 * the list does not have (a chat that is restoring, say), and it reads the catalogue's offer.
 */
export function stateButtonOf(chat: {
  session: number;
  task: boolean;
  working: boolean;
  has: (id: string) => boolean;
}): StateButton | undefined {
  const { session, task, working, has } = chat;
  // The words are the menus' (`menuWords.ts`), written once.
  const picks: StateButton[] = [
    { id: smartCloseCancelId(session), word: SAID.cancelSmartClose },
    { id: answerId(session), word: SAID.answerButton },
    ...(task ? [{ id: briefId(session), word: SAID.brief }] : []),
    ...(!task && working ? [{ id: stopId(session), word: SAID.stopButton }] : []),
  ];
  return picks.find((pick) => has(pick.id));
}

/**
 * The parts of a pane head a narrow pane can drop. The bell, ⋯, Split right and Close are not
 * among them: they never go.
 *
 * - `where`: the details on the head of a pane with no foot (its sandbox, harness · workspace,
 *   where it came from and its work item). A chat's are on its foot since #5204.
 * - `markWord`: the state mark's word, its dot staying.
 * - `buttonWord`: the state button's word, its icon staying with the word as its name.
 * - `mark`: the state mark, dot and all.
 * - `splitDown`: the Split down button.
 */
export type HeadPart = "where" | "markWord" | "buttonWord" | "mark" | "splitDown";

/** **The ruled order**, and the pane width in CSS pixels under which each part goes (#4585). */
export const HEAD_DROPS: readonly { part: HeadPart; below: number }[] = [
  { part: "where", below: 520 },
  { part: "markWord", below: 470 },
  { part: "buttonWord", below: 420 },
  { part: "mark", below: 380 },
  { part: "splitDown", below: 300 },
];

/** The parts a pane `width` pixels wide drops, in the ruled order: none where it is not
 *  measured, as in a test with no layout. */
export function droppedAt(width: number | undefined): ReadonlySet<HeadPart> {
  if (width === undefined) return new Set();
  return new Set(HEAD_DROPS.filter((drop) => width < drop.below).map((drop) => drop.part));
}

/** **What a pane shows**, as its two lines tell it apart (#5202): a chat, a task shown in a
 *  chat's pane (#1486), a shell, and a one-job program (#4761). */
export type PaneKind = "chat" | "task" | "shell" | "oneJob";

/**
 * **A chip of the head's line** (the **pane line**, `.pane-chips`), in its drawn order: who it
 * runs as, which chat it is (its name, or the path to the task it shows) and its state mark and
 * the mark's word. A pane with no foot keeps its details here too (spec #5191, Out of Scope): its
 * sandbox, harness · workspace (`where`), where it came from and its work item.
 */
export type LinePart =
  "persona" | "name" | "crumbs" | "sandbox" | "mark" | "markWord" | "where" | "from" | "workItem";

/**
 * **A part of the pane foot** (#5202, #5204, spec #5191, look A), in its drawn order: at its start
 * the harness and the workspace, where the chat came from (`from`, then what the Skill studio
 * started it for, `studio`, then its work item), its skills warning and its sandbox; at its end
 * `ctx`, its fill a thin bar under its number, and `cache`. The gauge's parts are drawn once the
 * chat's first usage arrives; the foot keeps its row either way, so the terminal never moves.
 */
export type FootPart =
  "harness" | "workspace" | "from" | "studio" | "workItem" | "skills" | "sandbox" | "ctx" | "cache";

/**
 * **The row of the chat's list each part of the foot presses** (#5204, spec #5191 story 37): a
 * foot's part has no press of its own, so nothing lives on the foot only. `chatList.test.ts` holds
 * each of these to the chat's ⋯ and the palette.
 */
export const FOOT_ROWS = {
  harness: chatHarnessId,
  workspace: chatWorkspaceId,
  from: askerId,
  studio: chatStudioId,
  skills: chatSkillsId,
  ctx: activityId,
} as const satisfies Partial<Record<FootPart, (session: number) => string>>;

/** **The parts of the foot that are facts alone**, with nothing to press: the sandbox and the work
 *  item, whose rows change them rather than show them, and `cache`, whose words are `ctx`'s. */
export const FOOT_FACTS: readonly FootPart[] = ["workItem", "sandbox", "cache"];

/** The details a chat has for its lines: drawn only where it has them. */
export type LineFacts = {
  /** It was handed off from, or asked by, another chat. */
  from?: boolean;
  /** Its set of skills needs the person. */
  skills?: boolean;
  /** The Skill studio started it (#5185). */
  studio?: boolean;
  /** It works on a work item (V60). */
  workItem?: boolean;
  /** Its sandbox is known. */
  sandbox?: boolean;
};

/** **Both lines of a pane**: the head's chips, and the foot's parts, or `null` for a pane that
 *  draws no foot. */
export type PaneLines = { head: LinePart[]; foot: FootPart[] | null };

/** **A pane's kind, by what it shows**: a task's path says it first, whatever runs there. */
export function paneKindOf(shows: { task: boolean; oneJob: boolean; shell: boolean }): PaneKind {
  if (shows.task) return "task";
  if (shows.oneJob) return "oneJob";
  return shows.shell ? "shell" : "chat";
}

/**
 * **What a pane of `kind`, `width` pixels wide (none where not measured), draws on its two lines**
 * (#5202, #5204): the head says which chat this is and what it is doing; the foot, under the
 * terminal, where it runs, where it came from and how healthy it is. On a chat's or a task's pane
 * the chat's facts are the foot's, never the head's, at any width. A shell and a one-job program
 * draw no foot and keep their heads as they were (spec #5191), `where` among them. The head's
 * actions are not here: they are `PaneActions`', by `droppedAt`, until #5206 settles them.
 */
export function paneLinesOf(
  kind: PaneKind,
  width: number | undefined,
  facts: LineFacts,
): PaneLines {
  const drops = droppedAt(width);
  const head: LinePart[] = ["persona", kind === "task" ? "crumbs" : "name"];
  const footed = kind === "chat" || kind === "task";
  if (!footed && !drops.has("where")) head.push("sandbox");
  // A task's path says its state; a chat's head says it with its mark.
  if (kind !== "task" && !drops.has("mark")) {
    head.push("mark");
    if (!drops.has("markWord")) head.push("markWord");
  }
  if (!footed && !drops.has("where")) {
    head.push("where");
    if (facts.from === true) head.push("from");
    if (facts.workItem === true) head.push("workItem");
  }
  if (!footed) return { head, foot: null };
  const foot: FootPart[] = ["harness", "workspace"];
  if (facts.from === true) foot.push("from");
  if (facts.studio === true) foot.push("studio");
  if (facts.workItem === true) foot.push("workItem");
  if (facts.skills === true) foot.push("skills");
  if (facts.sandbox === true) foot.push("sandbox");
  foot.push("ctx", "cache");
  return { head, foot };
}
