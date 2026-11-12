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
 * among them: they never go. The workspace is not either, being the foot's (#5206).
 *
 * - `number`: the chat's number beside its name (#6212), first to go: its tab's hover says it too,
 *   and the name needs the room more. Measured on the longest head (`ChatFoot.stories.tsx`): with
 *   it, a head at 470 px is 14 px over, so it goes under 500.
 * - `markWord`: the state mark's word, its dot staying.
 * - `using`: what the chat uses now, its skill run and its browsers (#5524, #5614), together. Each
 *   is a glance with another home (the run in the Skill studio's Runs, each browser a row under
 *   its chat and in the Browsers grid), so it goes before anything the head presses or says the
 *   chat by: after the state's word, before the button's word and the mark.
 * - `buttonWord`: the state button's word, its icon staying with the word as its name.
 * - `mark`: the state mark, dot and all.
 * - `splitDown`: the Split down button.
 */
export type HeadPart = "number" | "markWord" | "using" | "buttonWord" | "mark" | "splitDown";

/** **The ruled order**, and the pane width in CSS pixels under which each part goes (#4585). */
export const HEAD_DROPS: readonly { part: HeadPart; below: number }[] = [
  { part: "number", below: 500 },
  { part: "markWord", below: 470 },
  { part: "using", below: 450 },
  { part: "buttonWord", below: 420 },
  { part: "mark", below: 380 },
  { part: "splitDown", below: 300 },
];

/** Under this width a pane with no foot drops its details from its head, as it always has. */
const FOOTLESS_DETAILS_BELOW = 520;

/**
 * **The foot's order** (spec #5191 stories 30 to 32), and the width under which each part gives
 * way. Measured (#5206) on the default theme at the comfortable density, with the longest case the
 * prototype draws: `Claude Code · smart-ide · ↳ from steward 3 · ops · ⚠ 40 skills · 🛡` and `ctx 100%
 * cache 100%`, which needs 518 px, then 445 with no `cache`, 399 with the warning its ⚠, and 263 with
 * no `↳ from`. A longer name is cut at its end, so nothing but these widths moves the gauge.
 *
 * - `cache`: the number with no bar.
 * - `skills`: the warning's words, its ⚠ staying.
 * - `from`: where it came from, with what the studio started it for and its work item.
 * - `workspace`: the harness's name stays.
 */
export const FOOT_DROPS: readonly {
  part: "cache" | "skills" | "from" | "workspace";
  below: number;
}[] = [
  { part: "cache", below: 520 },
  { part: "skills", below: 450 },
  { part: "from", below: 400 },
  { part: "workspace", below: 270 },
];

/** The parts a pane `width` pixels wide drops, in the ruled order: none where it is not
 *  measured, as in a test with no layout. */
export function droppedAt(width: number | undefined): ReadonlySet<HeadPart> {
  if (width === undefined) return new Set();
  return new Set(HEAD_DROPS.filter((drop) => width < drop.below).map((drop) => drop.part));
}

/** **What a pane shows**, as its two lines tell it apart (#5202, #5205, spec #5191): a chat, a task
 *  shown in a chat's pane (#1486), either of them ended (#5019, #4757), a pane folded to a rail or
 *  a bar (#4766), a view, nothing, a shell, and a one-job program (#4761). */
export type PaneKind =
  "chat" | "task" | "ended" | "endedTask" | "folded" | "view" | "empty" | "shell" | "oneJob";

/**
 * **A chip of the head's line** (the **pane line**, `.pane-chips`), in its drawn order: who it
 * runs as, which chat it is (its name, or the path to the task it shows), its state mark and
 * the mark's word, and what it uses now (`using`, #5524). A pane with no foot keeps its details
 * here too (spec #5191, Out of Scope): its sandbox, harness · workspace (`where`), where it came
 * from and its work item.
 */
export type LinePart =
  | "persona"
  | "name"
  | "crumbs"
  | "number"
  | "sandbox"
  | "mark"
  | "markWord"
  | "using"
  | "where"
  | "from"
  | "workItem";

/**
 * **A part of the pane foot** (#5202, #5204, spec #5191, look A), in its drawn order: at its start
 * the harness and the workspace, where the chat came from (`from`, then what the Skill studio
 * started it for, `studio`, then its work item), its skills warning and its sandbox; at its end
 * `ctx`, its fill a thin bar under its number, and `cache`. The gauge's parts are drawn once the
 * chat's first usage arrives; the foot keeps its row either way, so the terminal never moves.
 */
export type FootPart =
  | "harness"
  | "workspace"
  | "from"
  | "studio"
  | "workItem"
  | "skills"
  | "skillsMark"
  | "sandbox"
  | "ended"
  | "ctx"
  | "cache";

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
  skillsMark: chatSkillsId,
  ctx: activityId,
} as const satisfies Partial<Record<FootPart, (session: number) => string>>;

/** **What a part of the foot presses**: its row of the chat's list, by that row's title. */
export type FootPress = { title: string; press: () => void };

/** **The parts of the foot that are facts alone**, with nothing to press: the sandbox and the work
 *  item, whose rows change them rather than show them, `cache`, whose words are `ctx`'s, and the
 *  mark of a chat that has ended, whose ways on are its ended band's. */
export const FOOT_FACTS: readonly FootPart[] = ["workItem", "sandbox", "ended", "cache"];

/**
 * **A control of the head's right side**, in its drawn order (#5203, #5206): the chat's actions,
 * its state button and that button's word, the bell and ⋯ last; then the pane's controls, Split
 * right, Split down and Close last.
 */
export type ActionPart =
  "state" | "stateWord" | "bell" | "menu" | "splitRight" | "splitDown" | "close";

/** The head's actions, whole: a chat's pane with room for them. */
export const WHOLE_ACTIONS: readonly ActionPart[] = [
  "state",
  "stateWord",
  "bell",
  "menu",
  "splitRight",
  "splitDown",
  "close",
];

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
  /** It may use something now, its skill run or its browsers, which its head says after its
   *  state (#5524) until a narrow pane drops it (#5614). */
  using?: boolean;
  /** False where the pane draws no close: a pane showing a task of its session (#1486). */
  closes?: boolean;
  /**
   * **The only pane of its tab** (#5318), which draws no Close, the tab's own × doing the same. A
   * pane zoomed to fill its tab is not alone: its tab still has the others.
   */
  alone?: boolean;
};

/** **Both lines of a pane**: the head's chips and its actions, and the foot's parts, or `null` for a pane that
 *  draws no foot. */
export type PaneLines = { head: LinePart[]; actions: ActionPart[]; foot: FootPart[] | null };

/** **A pane's kind, by what it shows**: a task's path says it first, whatever runs there. */
export function paneKindOf(shows: {
  task: boolean;
  oneJob: boolean;
  shell: boolean;
  ended: boolean;
}): PaneKind {
  if (shows.task) return shows.ended ? "endedTask" : "task";
  if (shows.oneJob) return "oneJob";
  if (shows.shell) return "shell";
  return shows.ended ? "ended" : "chat";
}

/**
 * **What a pane of `kind`, `width` pixels wide (none where not measured), draws on its two lines**
 * (#5202, #5204): the head says which chat this is and what it is doing; the foot, under the
 * terminal, where it runs, where it came from and how healthy it is. On a chat's or a task's pane
 * the chat's facts are the foot's, never the head's, at any width. A shell and a one-job program
 * draw no foot and keep their heads as they were (spec #5191), `where` among them, as does a
 * view's pane, an empty pane and a folded one. What has ended keeps its foot, marked `ended`, its
 * gauge frozen (#5205). A narrow pane drops parts of both lines in their orders (#5206). A tab's
 * only pane draws no Close, and a view alone in its tab draws neither split either (#5318).
 */
export function paneLinesOf(
  kind: PaneKind,
  width: number | undefined,
  facts: LineFacts,
): PaneLines {
  // A view's and an empty pane's head is its controls alone, which never drop, and a view alone in
  // its tab has none; a folded pane is a rail, which says who it runs as, its name and its bell.
  const alone = facts.alone === true;
  if (kind === "view" && alone) return { head: [], actions: [], foot: null };
  if (kind === "view" || kind === "empty") {
    const controls: ActionPart[] = ["splitRight", "splitDown"];
    if (!alone) controls.push("close");
    return { head: [], actions: controls, foot: null };
  }
  if (kind === "folded") return { head: ["persona", "name"], actions: ["bell"], foot: null };
  const drops = droppedAt(width);
  const pathed = kind === "task" || kind === "endedTask";
  const ended = kind === "ended" || kind === "endedTask";
  const footed = kind === "chat" || pathed || ended;
  const details = width === undefined || width >= FOOTLESS_DETAILS_BELOW;

  const head: LinePart[] = ["persona", pathed ? "crumbs" : "name"];
  // Its number beside its name (#6212), but on a one-job program's, which no one finds again.
  if (kind !== "oneJob" && !drops.has("number")) head.push("number");
  if (!footed && details) head.push("sandbox");
  // A task's path says its state; a chat's head says it with its mark.
  if (!pathed && !drops.has("mark")) {
    head.push("mark");
    if (!drops.has("markWord")) head.push("markWord");
  }
  if (facts.using === true && !drops.has("using")) head.push("using");
  if (!footed && details) {
    head.push("where");
    if (facts.from === true) head.push("from");
    if (facts.workItem === true) head.push("workItem");
  }

  const actions: ActionPart[] = ["state"];
  if (!drops.has("buttonWord")) actions.push("stateWord");
  actions.push("bell", "menu", "splitRight");
  if (!drops.has("splitDown")) actions.push("splitDown");
  if (kind !== "oneJob" && facts.closes !== false && !alone) actions.push("close");

  if (!footed) return { head, actions, foot: null };
  return { head, actions, foot: footOf(ended, width, facts) };
}

/** The parts a foot draws, by what the chat has and what its width leaves it (#5204, #5206). */
function footOf(ended: boolean, width: number | undefined, facts: LineFacts): FootPart[] {
  const gives = new Set(
    width === undefined
      ? []
      : FOOT_DROPS.filter((drop) => width < drop.below).map((drop) => drop.part),
  );
  const foot: FootPart[] = ["harness"];
  if (!gives.has("workspace")) foot.push("workspace");
  if (!gives.has("from")) {
    if (facts.from === true) foot.push("from");
    if (facts.studio === true) foot.push("studio");
    if (facts.workItem === true) foot.push("workItem");
  }
  // What has ended can do nothing about its skills.
  if (facts.skills === true && !ended) foot.push(gives.has("skills") ? "skillsMark" : "skills");
  if (facts.sandbox === true) foot.push("sandbox");
  if (ended) foot.push("ended");
  foot.push("ctx");
  if (!gives.has("cache")) foot.push("cache");
  return foot;
}
