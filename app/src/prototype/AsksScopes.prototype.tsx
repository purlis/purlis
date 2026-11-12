// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **PROTOTYPE, NOT FOR MERGE: Asks scoped to Chat, Workspace, Project.** Throwaway code that
 * answers one question: do the new scope chips, their counts and the count's slide look and feel
 * right? Nothing here is wired to the real Inbox; every count is the story's own, in memory.
 *
 * The design under test:
 * - Asks' chips are **Chat, Workspace, Project**. Chat is the focused chat and its whole dispatch
 *   subtree, named in its tooltip ("steward 8 and its 3 tasks"). With no chat focused it is
 *   greyed and Asks uses Workspace (Project where there is one workspace).
 *   Workspace is not drawn when the project has one workspace.
 * - A chip shows its waiting count in the ✋'s colour only above 0; at 0, its name alone.
 *   Chat ≤ Workspace ≤ Project. Every count keeps the room of two digits; above 99, 99+.
 * - The ✋ counts the project in front only, never more than the Project chip; another
 *   project's asks show only on its tab.
 * - A changed count slides once (old number out, new in) on the chips, the ✋ and the project
 *   tabs. No pulse, no repeat. Under reduced motion the number just swaps.
 * - Asks opens on Chat. An empty scope says how many wait in the next wider one, which widens
 *   when pressed. It never switches by itself.
 */
import { useLayoutEffect, useRef, useState, type CSSProperties } from "react";
import { Hand } from "lucide-react";
import { Button, ButtonRow } from "../ui/button/Button";
import { Count } from "../ui/badge/Badge";
import { Segment, SegmentedControl } from "../ui/choice/Radio";
import { Switch } from "../ui/choice/Switch";
import { TabCell, tabPart, tabsPart } from "../ui/tabs/Tabs";
import { StripTab, TabStripRow, useTabIds, useTabStrip } from "../ui/tabs/TabStrip";
import { TIPS_BELOW } from "../ui/tooltip/Tooltip";
import { motionReduced } from "../theme/motion";

/** The scopes, narrowest first. */
type Scope = "chat" | "workspace" | "project";

const SAYS: Record<Scope, string> = { chat: "Chat", workspace: "Workspace", project: "Project" };
const IN: Record<Scope, string> = {
  chat: "this chat",
  workspace: "this workspace",
  project: "this project",
};

/** The project in front, and the two others on the strip. */
const HERE = "purlis";
const OTHERS = ["website", "infra"] as const;

/** The focused chat, and its dispatch subtree, as the Chat chip's tooltip names them. */
const FOCUSED = { name: "steward 8", tasks: 3 };

/** The ease the shell's motions share (`easing.enter`), read from the theme where it is set. */
function enterEase(): string {
  const read = getComputedStyle(document.documentElement)
    .getPropertyValue("--motion-easing-enter")
    .trim();
  return read === "" ? "ease-out" : read;
}

/**
 * **A number that slides once when it changes**: the old one out, the new one in, up when it
 * grows and down when it shrinks, over `ms`. At `ms` 0, or under reduced motion, it swaps.
 * It always takes the room of two digits, so 9 to 10 moves nothing; above 99 it says 99+.
 */
export function Rolling({ value, ms, reduced }: { value: number; ms: number; reduced: boolean }) {
  const [now, setNow] = useState(value);
  const [from, setFrom] = useState<number | null>(null);
  const still = reduced || ms <= 0 || motionReduced();
  if (now !== value) {
    // Under reduced motion there is nothing to slide from: the number just swaps. Nor is there
    // when what is drawn stays the same (99+ to 99+).
    setFrom(still || capped(now) === capped(value) ? null : now);
    setNow(value);
  }
  const incoming = useRef<HTMLSpanElement>(null);
  const outgoing = useRef<HTMLSpanElement>(null);
  useLayoutEffect(() => {
    // A browser without the Web Animations API (jsdom) swaps, as reduced motion does.
    if (from === null || still || typeof incoming.current?.animate !== "function") return;
    const up = value > from;
    const timing: KeyframeAnimationOptions = { duration: ms, easing: enterEase(), fill: "both" };
    const enter = incoming.current?.animate(
      [{ transform: `translateY(${up ? 100 : -100}%)` }, { transform: "translateY(0)" }],
      timing,
    );
    const leave = outgoing.current?.animate(
      [{ transform: "translateY(0)" }, { transform: `translateY(${up ? -100 : 100}%)` }],
      timing,
    );
    enter?.finished.then(() => setFrom(null)).catch(() => undefined);
    return () => {
      enter?.cancel();
      leave?.cancel();
    };
  }, [from, value, ms, still]);
  return (
    <span className="inline-grid overflow-hidden text-center tabular-nums">
      {/* The room of two digits, drawn by no one. */}
      <span aria-hidden="true" className="invisible col-start-1 row-start-1">
        88
      </span>
      <span ref={incoming} className="col-start-1 row-start-1">
        {capped(value)}
      </span>
      {from !== null && !still && (
        <span ref={outgoing} className="col-start-1 row-start-1">
          {capped(from)}
        </span>
      )}
    </span>
  );
}

/** A count as it is drawn: the number, or 99+ above 99. */
const capped = (count: number) => (count > 99 ? "99+" : String(count));

/** A waiting count in the ✋'s colour, drawn only above 0. */
function Waiting({
  count,
  ms,
  reduced,
  said,
}: {
  count: number;
  ms: number;
  reduced: boolean;
  said: string;
}) {
  if (count <= 0) return null;
  return (
    <Count variant="hot" said={said}>
      <Rolling value={count} ms={ms} reduced={reduced} />
    </Count>
  );
}

const waitingSaid = (count: number) => `${count} waiting on you`;

/** The story's whole state: every count the controls move, and the switches. */
export type Counts = {
  /** Waiting in the focused chat and its subtree. */
  chat: number;
  /** Waiting in this workspace outside the focused chat's subtree. */
  workspaceRest: number;
  /** Waiting in this project's other workspaces. */
  otherWorkspaces: number;
  /** Waiting in each other project on the strip. */
  others: Record<(typeof OTHERS)[number], number>;
};

export type Setup = {
  oneWorkspace: boolean;
  noChatFocused: boolean;
  reduced: boolean;
  ms: number;
};

/** What each chip counts. Chat ≤ Workspace ≤ Project holds by how they are summed. */
export function scopeCounts(counts: Counts, setup: Setup): Record<Scope, number> {
  const chat = setup.noChatFocused ? 0 : counts.chat;
  const workspace = counts.chat + counts.workspaceRest;
  const project = workspace + (setup.oneWorkspace ? 0 : counts.otherWorkspaces);
  return { chat, workspace, project };
}

/** **The ✋** in the title bar: the project in front's waiting count, never more than the Project
 *  chip (reversing ADR 0121 decision 2). Another project's asks show only on its tab. */
function HandBadge({ total, setup }: { total: number; setup: Setup }) {
  return (
    <Button
      variant="quiet"
      size="sm"
      icon={<Hand aria-hidden="true" />}
      aria-label={total > 0 ? `Asks, ${waitingSaid(total)}` : "Asks"}
      tooltip={total > 0 ? waitingSaid(total) : "Nothing waits on you"}
    >
      <Waiting count={total} ms={setup.ms} reduced={setup.reduced} said={waitingSaid(total)} />
    </Button>
  );
}

/** **The project strip**: each project's tab with its own waiting count. */
function ProjectTabs({ waiting, setup }: { waiting: Record<string, number>; setup: Setup }) {
  const names = [HERE, ...OTHERS];
  const idOf = useTabIds();
  const along = useTabStrip({ keys: names, selected: HERE });
  return (
    <div {...tabsPart("layout")}>
      <header className="bar" {...tabsPart("strip")} {...TIPS_BELOW}>
        <TabStripRow
          strip={along}
          name="Projects"
          ids={names.map((_, at) => idOf(at))}
          className="tabs"
          style={{ "--least": "88px" } as CSSProperties}
        >
          {names.map((name, at) => {
            const count = waiting[name] ?? 0;
            return (
              <TabCell
                key={name}
                className="tab"
                variant="view"
                selected={name === HERE}
                first={at === 0}
              >
                <StripTab
                  strip={along}
                  tabKey={name}
                  selected={name === HERE}
                  tip={count > 0 ? `${name}, ${waitingSaid(count)}` : undefined}
                  type="button"
                  id={idOf(at)}
                >
                  <span className="tab-name" {...tabPart("label")}>
                    {name}
                  </span>
                  <Waiting
                    count={count}
                    ms={setup.ms}
                    reduced={setup.reduced}
                    said={waitingSaid(count)}
                  />
                </StripTab>
              </TabCell>
            );
          })}
        </TabStripRow>
      </header>
    </div>
  );
}

/** A made-up ask row, so a scope's list has something in it. */
const ROWS = [
  "Allow npm publish --tag next?",
  "Which token should the runner use?",
  "Run cargo update in purlis-core?",
  "Reply to the review of #6244",
  "Push the release branch?",
];

/** **Asks**: the chips, then the list of the picked scope, or the line that widens it. */
function Asks({ counts, setup }: { counts: Record<Scope, number>; setup: Setup }) {
  const [scope, setScope] = useState<Scope>("chat");
  const offered: Scope[] = setup.oneWorkspace
    ? ["chat", "project"]
    : ["chat", "workspace", "project"];
  // With no chat focused, a Chat pick is drawn on the next scope (Workspace, or Project where
  // there is one workspace), and comes back when a chat has focus. A hidden Workspace leaves its
  // pick on Project, the same asks.
  const reachable = offered.filter((one) => !(one === "chat" && setup.noChatFocused));
  const picked: Scope = reachable.includes(scope)
    ? scope
    : scope === "chat"
      ? (reachable[0] ?? "project")
      : "project";
  const listed = counts[picked];
  const wider = offered.slice(offered.indexOf(picked) + 1).find((one) => counts[one] > 0);
  const chatTip = setup.noChatFocused
    ? "No chat has focus"
    : `${FOCUSED.name} and its ${FOCUSED.tasks} tasks`;
  return (
    <section aria-label="Asks" className="flex flex-col gap-2">
      <SegmentedControl
        size="sm"
        aria-label="Show the asks of"
        value={picked}
        onChange={(to) => setScope(to as Scope)}
      >
        {offered.map((one) => (
          <Segment
            key={one}
            value={one}
            isDisabled={one === "chat" && setup.noChatFocused}
            tooltip={one === "chat" ? chatTip : undefined}
            aria-label={counts[one] > 0 ? `${SAYS[one]}, ${waitingSaid(counts[one])}` : SAYS[one]}
          >
            <span className="inline-flex items-center gap-1">
              {SAYS[one]}
              <Waiting
                count={counts[one]}
                ms={setup.ms}
                reduced={setup.reduced}
                said={waitingSaid(counts[one])}
              />
            </span>
          </Segment>
        ))}
      </SegmentedControl>
      {listed > 0 ? (
        <ul aria-label={`Asks in ${IN[picked]}`} className="flex flex-col gap-1">
          {Array.from({ length: listed }, (_, at) => (
            <li key={at} className="text-sm">
              {ROWS[at % ROWS.length]}
            </li>
          ))}
        </ul>
      ) : (
        <div className="flex flex-col gap-1">
          <p className="text-sm">{`Nothing waits on you in ${IN[picked]}.`}</p>
          {wider !== undefined && (
            <Button variant="ghost" size="sm" onPress={() => setScope(wider)}>
              {`${counts[wider]} waiting in ${IN[wider]} ›`}
            </Button>
          )}
        </div>
      )}
    </section>
  );
}

/** **The prototype**: the ✋, the project strip and Asks, with the controls that move them. */
export function AsksScopesPrototype({ start }: { start: Counts }) {
  const [counts, setCounts] = useState(start);
  const [setup, setSetup] = useState<Setup>({
    oneWorkspace: false,
    noChatFocused: false,
    reduced: false,
    ms: 200,
  });
  const scoped = scopeCounts(counts, setup);
  const waiting: Record<string, number> = { [HERE]: scoped.project, ...counts.others };
  // The ✋ counts the project in front only.
  const total = scoped.project;
  const move = (key: "chat" | "workspaceRest" | "otherWorkspaces", by: number) =>
    setCounts((was) => ({ ...was, [key]: Math.max(0, was[key] + by) }));
  const moveOther = (name: (typeof OTHERS)[number], by: number) =>
    setCounts((was) => ({
      ...was,
      others: { ...was.others, [name]: Math.max(0, was.others[name] + by) },
    }));
  const flip = (key: "oneWorkspace" | "noChatFocused" | "reduced") => (on: boolean) =>
    setSetup((was) => ({ ...was, [key]: on }));
  return (
    <div className="flex flex-col gap-4 p-4">
      <p className="text-sm">Prototype, not for merge: Asks scoped to Chat, Workspace, Project.</p>
      <div className="flex items-center gap-2">
        <HandBadge total={total} setup={setup} />
        <ProjectTabs waiting={waiting} setup={setup} />
      </div>
      <Asks counts={scoped} setup={setup} />
      <section aria-label="Prototype controls" className="flex flex-col gap-2">
        <ButtonRow wraps>
          <Button size="sm" onPress={() => move("chat", 1)}>
            Chat +1
          </Button>
          <Button size="sm" onPress={() => move("chat", -1)}>
            Chat −1
          </Button>
          <Button size="sm" onPress={() => move("chat", 100)}>
            Chat +100
          </Button>
          <Button size="sm" onPress={() => move("chat", -100)}>
            Chat −100
          </Button>
          <Button size="sm" onPress={() => move("workspaceRest", 1)}>
            Workspace +1
          </Button>
          <Button size="sm" onPress={() => move("workspaceRest", -1)}>
            Workspace −1
          </Button>
          <Button size="sm" onPress={() => move("otherWorkspaces", 1)}>
            Other workspace +1
          </Button>
          <Button size="sm" onPress={() => move("otherWorkspaces", -1)}>
            Other workspace −1
          </Button>
          {OTHERS.map((name) => (
            <Button key={`${name}+`} size="sm" onPress={() => moveOther(name, 1)}>
              {`${name} +1`}
            </Button>
          ))}
          {OTHERS.map((name) => (
            <Button key={`${name}-`} size="sm" onPress={() => moveOther(name, -1)}>
              {`${name} −1`}
            </Button>
          ))}
        </ButtonRow>
        <div className="flex flex-wrap gap-4">
          <Switch isSelected={setup.oneWorkspace} onChange={flip("oneWorkspace")}>
            One workspace
          </Switch>
          <Switch isSelected={setup.noChatFocused} onChange={flip("noChatFocused")}>
            No chat focused
          </Switch>
          <Switch isSelected={setup.reduced} onChange={flip("reduced")}>
            Reduced motion
          </Switch>
        </div>
        <SegmentedControl
          size="sm"
          aria-label="Slide duration"
          value={String(setup.ms)}
          onChange={(to) => setSetup((was) => ({ ...was, ms: Number(to) }))}
        >
          {[150, 200, 280].map((ms) => (
            <Segment key={ms} value={String(ms)}>
              {`${ms} ms`}
            </Segment>
          ))}
        </SegmentedControl>
        <output aria-label="State" className="text-xs font-mono">
          {`chips ${JSON.stringify(scoped)} · ✋ ${total} · tabs ${JSON.stringify(waiting)} · ${JSON.stringify(setup)}`}
        </output>
      </section>
    </div>
  );
}
