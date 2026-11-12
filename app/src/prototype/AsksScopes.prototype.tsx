// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **PROTOTYPE v2, NOT FOR MERGE: Asks scoped to Chat, Workspace, Project.** Throwaway code, in
 * place of #6310, that answers one question: do the scope chips, their counts, the counts' slide
 * and the ✋'s bounces look and feel right? It is drawn from the window's own parts as they are
 * drawn: the project rail (`RailProject`), the Asks view (`Inbox` with its rows, its "Quiet now"
 * and its Snooze all) and the right bar's ✋ (`ActivityBar`, `ActivityCount`). Nothing is wired
 * to the core; every count is the story's own, in memory. What no existing class draws is in
 * `AsksScopes.prototype.css`, which the stories import.
 *
 * The design under test:
 * - **Chips**: Chat, Workspace, Project. Chat is the focused chat and its dispatch subtree, named
 *   in its tooltip. With no chat focused it is greyed, and Asks uses Workspace (Project where
 *   the project has one workspace). Workspace is not drawn when there is one workspace.
 * - **Counts**: small circles in the ✋'s colour, only above 0: 1 to 9, then 9+, so nothing moves.
 *   The tabs, the chips and the ✋ count the project in front; the ✋ never exceeds Project.
 * - **A changed count slides once** (150, 200 or 280 ms). Under reduced motion it swaps.
 * - **The ✋ bounces 3 times** when its count goes up, and, while a Blocking ask waits, again on a
 *   backoff of 5, 10, 20, 40 min, then every 60. The backoff starts again at 5 on a new ask, an
 *   answer, or Asks opening, and stops when nothing waits. Remind me: Off (no reminders),
 *   Gently (the backoff), Often (every 5 min).
 * - **No bounce** during Quiet, while typing, under reduced motion, or with the window behind.
 * - **Asks opens on Chat.** An empty scope offers "N waiting in this workspace ›", and nothing
 *   switches by itself.
 */
import { useEffect, useReducer, useRef, useState } from "react";
import type { Shown } from "../bindings";
import { ActivityBar, ActivityCount, VIEW_MARKS, type Tabbed } from "../ActivityBar";
import { SnoozeAll, type Snoozing } from "../AsksSnooze";
import { Inbox } from "../Inbox";
import { QUIET_NOW, QUIET_NOW_TIP } from "../quiet";
import { Count } from "../ui/badge/Badge";
import { Button, ButtonRow, ToggleButton } from "../ui/button/Button";
import { Segment, SegmentedControl } from "../ui/choice/Radio";
import { Switch } from "../ui/choice/Switch";
import { Rail, RailList, RailProject } from "../ui/rail/Rail";
import { useProjectTints } from "../ui/rail/projectColour";
import { AsStripTab, useTabStrip } from "../ui/tabs/TabStrip";
import { tabPanelOf } from "../ui/tabs/Tabs";

// --- The model ----------------------------------------------------------------------------------

/** The scopes, narrowest first. */
export type Scope = "chat" | "workspace" | "project";

const SAYS: Record<Scope, string> = { chat: "Chat", workspace: "Workspace", project: "Project" };
const IN: Record<Scope, string> = {
  chat: "this chat",
  workspace: "this workspace",
  project: "this project",
};

/** The project in front, and the two others on the rail. */
export const HERE = "purlis";
export const OTHERS = ["website", "infra"] as const;
export type Other = (typeof OTHERS)[number];

/** The focused chat and its dispatch subtree, as the Chat chip's tooltip names them. */
const FOCUSED = { name: "steward 8", tasks: 2 };

/** Every count the controls move. */
export type Counts = {
  /** Waiting in the focused chat and its dispatch subtree. */
  chat: number;
  /** Waiting in this workspace, outside the focused chat's subtree. */
  workspaceRest: number;
  /** Waiting in this project's other workspaces. */
  otherWorkspaces: number;
  /** Waiting in each other project on the rail. */
  others: Record<Other, number>;
};

/** How often the ✋ reminds while a Blocking ask waits. */
export type Remind = "off" | "gently" | "often";

/** How fast the story's clock runs, so a backoff of minutes is watched in seconds. */
export type Speed = 1 | 60 | 600;

/** The slide's length, each a motion token's: `quick`, `leave`, `settle`. */
export type SlideMs = 150 | 200 | 280;

/** The switches. */
export type Setup = {
  oneWorkspace: boolean;
  noChatFocused: boolean;
  reduced: boolean;
  quiet: boolean;
  typing: boolean;
  inFront: boolean;
  /** Whether what waits includes a Blocking ask, which the reminders are for. */
  blocking: boolean;
  remind: Remind;
  speed: Speed;
  slideMs: SlideMs;
};

export const SETUP: Setup = {
  oneWorkspace: false,
  noChatFocused: false,
  reduced: false,
  quiet: false,
  typing: false,
  inFront: true,
  blocking: true,
  remind: "gently",
  speed: 1,
  slideMs: 200,
};

/** What each chip counts. Chat ≤ Workspace ≤ Project holds by how they are summed. */
export function scopeCounts(counts: Counts, setup: Setup): Record<Scope, number> {
  const chat = setup.noChatFocused ? 0 : counts.chat;
  const workspace = counts.chat + counts.workspaceRest;
  const project = workspace + (setup.oneWorkspace ? 0 : counts.otherWorkspaces);
  return { chat, workspace, project };
}

/** The chips offered: Workspace not where the project has one workspace. */
export const offeredScopes = (setup: Setup): Scope[] =>
  setup.oneWorkspace ? ["chat", "project"] : ["chat", "workspace", "project"];

/**
 * The scope Asks lists: the one picked, unless it cannot be. With no chat focused, Chat is
 * listed as the next scope offered; a hidden Workspace as Project, the same asks.
 */
export function listedScope(picked: Scope, setup: Setup): Scope {
  const offered = offeredScopes(setup).filter((one) => !(one === "chat" && setup.noChatFocused));
  if (offered.includes(picked)) return picked;
  return picked === "chat" ? (offered[0] ?? "project") : "project";
}

/** A count as it is drawn, one character so its circle never changes shape: the number, or
 *  9+ above 9. What is said is always the real number. */
export const capped = (count: number) => (count > 9 ? "9+" : String(count));

const MINUTE = 60_000;

/** The backoff, Gently: 5, 10, 20, 40 minutes, then every 60. */
export const GENTLY_MINUTES = [5, 10, 20, 40, 60] as const;

/** How long after reminder `step` (0 is the first) the next one comes, or none. */
export function reminderGap(remind: Remind, step: number): number | null {
  if (remind === "off") return null;
  if (remind === "often") return 5 * MINUTE;
  return GENTLY_MINUTES[Math.min(step, GENTLY_MINUTES.length - 1)] * MINUTE;
}

/** Why the ✋ holds still now, or nothing. */
export function heldBy(setup: Setup): string | undefined {
  if (setup.quiet) return "Quiet is on";
  if (setup.typing) return "you are typing";
  if (setup.reduced) return "reduced motion";
  if (!setup.inFront) return "the window is behind";
  return undefined;
}

/** A length of the story's clock as "4:05" or "1:02:03". */
export function clockSaid(ms: number): string {
  const all = Math.max(0, Math.ceil(ms / 1000));
  const hours = Math.floor(all / 3600);
  const minutes = Math.floor((all % 3600) / 60);
  const seconds = String(all % 60).padStart(2, "0");
  return hours > 0
    ? `${hours}:${String(minutes).padStart(2, "0")}:${seconds}`
    : `${minutes}:${seconds}`;
}

// --- The asks drawn -----------------------------------------------------------------------------

/** One made-up permission ask, shaped as the registry tells it. */
const permission = (session: number, chain: string[], says: string): Shown => ({
  session,
  ask: `permission-${session}`,
  says,
  options: [
    { id: "allow", label: "Allow", allows: true },
    { id: "deny", label: "Deny", allows: false },
  ],
  source: "permission",
  chain,
  answer: { via: "hook" },
});

/** What the focused chat's subtree, the rest of the workspace and the other workspaces ask. */
const POOLS: Record<"chat" | "workspaceRest" | "otherWorkspaces", (at: number) => Shown> = {
  chat: (at) =>
    [
      permission(8, ["steward 8"], "Run cargo test -p purlis-core"),
      permission(12, ["steward 8", "dev 12"], "Run npm ci"),
      permission(13, ["steward 8", "dev 13"], "Run git push origin HEAD"),
    ][at % 3] ?? permission(8, ["steward 8"], "Run cargo test"),
  workspaceRest: (at) =>
    permission(
      30 + at,
      [`reviewer ${30 + at}`],
      ["Run gh pr view 6310", "Run npm run lint"][at % 2],
    ),
  otherWorkspaces: (at) =>
    permission(60 + at, [`ops ${60 + at}`], ["Run terraform plan", "Run kubectl get pods"][at % 2]),
};

/** At most this many rows of each part are drawn; the counts stay what the controls say. */
const ROWS_EACH = 2;

const some = (pool: keyof typeof POOLS, count: number) =>
  Array.from({ length: Math.min(count, ROWS_EACH) }, (_, at) => POOLS[pool](at));

/** The rows Asks lists for a scope. */
function asksOf(scope: Scope, counts: Counts, setup: Setup): Shown[] {
  const chat = setup.noChatFocused ? [] : some("chat", counts.chat);
  if (scope === "chat") return chat;
  const workspace = [...some("chat", counts.chat), ...some("workspaceRest", counts.workspaceRest)];
  if (scope === "workspace" || setup.oneWorkspace) return workspace;
  return [...workspace, ...some("otherWorkspaces", counts.otherWorkspaces)];
}

// --- The slide ----------------------------------------------------------------------------------

/**
 * **Which way a number last moved, and how many times**: `dir` is nothing until it first moves,
 * and nothing under reduced motion, so nothing slides then. A count drawn the same (9+ to 9+)
 * has not moved.
 */
function useSlide(value: number, still: boolean): { dir?: "up" | "down"; moves: number } {
  const [seen, setSeen] = useState(value);
  const [moved, setMoved] = useState<{ dir?: "up" | "down"; moves: number }>({ moves: 0 });
  if (seen !== value) {
    setSeen(value);
    if (capped(seen) !== capped(value))
      setMoved((was) => ({
        dir: still ? undefined : value > seen ? "up" : "down",
        moves: was.moves + 1,
      }));
  }
  return moved;
}

/** **A chip's count**: the family's hot `Count`, its number sliding in when it changes; nothing
 *  at 0. Its words are the chip's name, so the number itself is not read twice. */
function ChipCount({ count, still }: { count: number; still: boolean }) {
  const { dir, moves } = useSlide(count, still);
  if (count <= 0) return null;
  return (
    <Count variant="hot" aria-hidden="true">
      <span
        key={moves}
        className="asks-proto-roll"
        data-dir={dir}
        data-capped={count > 9 ? "" : undefined}
      >
        {capped(count)}
      </span>
    </Count>
  );
}

const waitingSaid = (count: number) => `${count} waiting on you`;

// --- The parts ----------------------------------------------------------------------------------

/** **The project rail**: each project's tab, its count the project's own. */
function ProjectRail({ waiting, still }: { waiting: Record<string, number>; still: boolean }) {
  const names = [HERE, ...OTHERS];
  const tintOf = useProjectTints();
  const strip = useTabStrip({ keys: names, selected: HERE });
  return (
    <div className="asks-proto-rail">
      <Rail name="Projects" strip={strip} tabs={names.map((name) => `asks-proto-rail-${name}`)}>
        <RailList>
          {names.map((name) => (
            <RailTab
              key={name}
              name={name}
              waiting={waiting[name] ?? 0}
              still={still}
              strip={strip}
              colour={tintOf(name)}
            />
          ))}
        </RailList>
      </Rail>
    </div>
  );
}

function RailTab({
  name,
  waiting,
  still,
  strip,
  colour,
}: {
  name: string;
  waiting: number;
  still: boolean;
  strip: ReturnType<typeof useTabStrip>;
  colour: ReturnType<ReturnType<typeof useProjectTints>>;
}) {
  const { dir, moves } = useSlide(waiting, still);
  return (
    <AsStripTab strip={strip} tabKey={name} selected={name === HERE}>
      {(tab) => (
        <RailProject
          {...tab}
          id={`asks-proto-rail-${name}`}
          name={name}
          current={name === HERE}
          waiting={waiting}
          colour={colour}
          data-slide={dir === undefined ? undefined : moves % 2 === 0 ? "a" : "b"}
          data-slide-dir={dir}
          data-capped={waiting > 9 ? "" : undefined}
        />
      )}
    </AsStripTab>
  );
}

const BAR: Tabbed[] = [
  { view: "asks", name: "Asks", mark: VIEW_MARKS.asks },
  { view: "updates", name: "Updates", mark: VIEW_MARKS.updates },
  { view: "notices", name: "Notices", mark: VIEW_MARKS.notices },
];

/** **The right bar**, the ✋ first: its count, sliding when it changes, and its bounces. */
function RightBar({
  total,
  still,
  bouncing,
  bounces,
  onBounced,
}: {
  total: number;
  still: boolean;
  bouncing: boolean;
  bounces: number;
  onBounced: () => void;
}) {
  const { dir, moves } = useSlide(total, still);
  return (
    <div
      className={`activity-column asks-proto-hand${bouncing ? " asks-proto-bounce" : ""}`}
      data-side="right"
      data-dir={dir}
      data-bounces={bounces}
      onAnimationEnd={(event) => {
        if (event.animationName === "asks-proto-bounce") onBounced();
      }}
    >
      <ActivityBar
        side="right"
        variant="bar"
        name="Attention"
        views={BAR}
        open="asks"
        current="asks"
        badges={{
          asks: (
            <ActivityCount
              key={moves}
              count={total}
              drawn={capped(total)}
              said={waitingSaid(total)}
              tone="needs-you"
            />
          ),
        }}
        panelOf={(view) => `asks-proto-panel-${view}`}
        tabOf={(view) => `asks-proto-tab-${view}`}
        onPick={() => {}}
      />
    </div>
  );
}

/** **The chips**: the segmented control of the Inbox's scope switch, each chip's count after its
 *  words, at the top of Asks where the scope switch is drawn. */
function Chips({
  counts,
  setup,
  listed,
  onPick,
}: {
  counts: Record<Scope, number>;
  setup: Setup;
  listed: Scope;
  onPick: (to: Scope) => void;
}) {
  const still = setup.reduced;
  return (
    <div className="inbox-scope">
      <SegmentedControl
        size="sm"
        className="scope-switch"
        aria-label="Show the asks of"
        value={listed}
        onChange={(to) => onPick(to as Scope)}
      >
        {offeredScopes(setup).map((one) => {
          const off = one === "chat" && setup.noChatFocused;
          const count = off ? 0 : counts[one];
          const name = count > 0 ? `${SAYS[one]}, ${waitingSaid(count)}` : SAYS[one];
          return (
            <Segment
              key={one}
              value={one}
              isDisabled={off}
              aria-label={name}
              tooltip={
                one === "chat"
                  ? off
                    ? "No chat has focus"
                    : `${FOCUSED.name} and its ${FOCUSED.tasks} tasks`
                  : name
              }
            >
              {SAYS[one]} <ChipCount count={count} still={still} />
            </Segment>
          );
        })}
      </SegmentedControl>
    </div>
  );
}

/** **Quiet at the top of Asks**: the window's own "Quiet now" toggle (`AsksQuiet`), here
 *  pressed by the story's state rather than the core's. */
function QuietTop({ on, onChange }: { on: boolean; onChange: (on: boolean) => void }) {
  return (
    <div className="inbox-quiet">
      <ToggleButton
        size="sm"
        variant="ghost"
        isSelected={on}
        onChange={onChange}
        tooltip={QUIET_NOW_TIP}
      >
        {QUIET_NOW}
      </ToggleButton>
    </div>
  );
}

const SNOOZING: Snoozing = {
  snoozed: [],
  snoozedUntil: null,
  snooze: async () => undefined,
  wake: async () => undefined,
};

/** **The Asks view**: the Inbox's Asks as the window draws it, with the chips in its scope's
 *  place; an empty scope says how many wait in the next wider one. */
function AsksView({
  counts,
  setup,
  picked,
  onPick,
  onQuiet,
}: {
  counts: Counts;
  setup: Setup;
  picked: Scope;
  onPick: (to: Scope) => void;
  onQuiet: (on: boolean) => void;
}) {
  const scoped = scopeCounts(counts, setup);
  const listed = listedScope(picked, setup);
  const asks = asksOf(listed, counts, setup);
  const top = (
    <>
      <QuietTop on={setup.quiet} onChange={onQuiet} />
      <Chips counts={scoped} setup={setup} listed={listed} onPick={onPick} />
    </>
  );
  if (scoped[listed] > 0)
    return (
      <Inbox
        part="asks"
        plane="/prototype"
        asks={asks}
        onGo={() => {}}
        onLeave={() => {}}
        snoozing={SNOOZING}
        quietTop={top}
      />
    );
  const offered = offeredScopes(setup);
  const wider = offered.slice(offered.indexOf(listed) + 1).find((one) => scoped[one] > 0);
  return (
    <section className="inbox" data-view="asks">
      {top}
      <SnoozeAll listed={[]} snoozing={SNOOZING} />
      <div className="inbox-list">
        <div className="inbox-scope-empty">
          <p className="inbox-none">{`Nothing is waiting on you in ${IN[listed]}.`}</p>
          {wider !== undefined && (
            <Button variant="link" onPress={() => onPick(wider)}>
              {`${scoped[wider]} waiting in ${IN[wider]} ›`}
            </Button>
          )}
        </div>
      </div>
    </section>
  );
}

// --- The reminders ------------------------------------------------------------------------------

/** What a bounce and the schedule read of the setup, at the moment they happen. */
type Context = { remind: Remind; waits: boolean; held: string | undefined };

const contextOf = (total: number, setup: Setup): Context => ({
  remind: setup.remind,
  waits: total > 0 && setup.blocking,
  held: heldBy(setup),
});

/** **The story's clock and the ✋'s schedule**: the clock's time, the reminder `step` that
 *  comes `next`, and every bounce drawn. */
export type Clock = {
  now: number;
  step: number;
  next: number | null;
  /** How many bounces were drawn; one held by the setup is said in `last`, not drawn. */
  bounces: number;
  last: string;
};

export type ClockEvent =
  | { type: "tick"; by: number; context: Context }
  | { type: "count"; up: boolean; context: Context }
  | { type: "restart"; context: Context };

function bounceIn(clock: Clock, why: string, context: Context): Clock {
  const at = clockSaid(clock.now);
  return context.held === undefined
    ? { ...clock, bounces: clock.bounces + 1, last: `${why} at ${at}` }
    : { ...clock, last: `${why} at ${at}, held: ${context.held}` };
}

/** The backoff from its first step: 5 minutes on, while a Blocking ask waits. */
export function restarted(clock: Clock, context: Context): Clock {
  const gap = reminderGap(context.remind, 0);
  return { ...clock, step: 0, next: context.waits && gap !== null ? clock.now + gap : null };
}

/**
 * **What happens to the ✋**: a new ask bounces it and starts the backoff again, an answer
 * starts it again, and a reminder that falls due bounces it and waits the next gap.
 */
export function clockAfter(clock: Clock, event: ClockEvent): Clock {
  switch (event.type) {
    case "tick": {
      const now = clock.now + event.by;
      if (clock.next === null || now < clock.next) return { ...clock, now };
      const bounced = bounceIn({ ...clock, now }, `Reminder ${clock.step + 1}`, event.context);
      const step = clock.step + 1;
      const gap = reminderGap(event.context.remind, step);
      return { ...bounced, step, next: gap === null ? null : now + gap };
    }
    case "count":
      return restarted(event.up ? bounceIn(clock, "New ask", event.context) : clock, event.context);
    case "restart":
      return restarted(clock, event.context);
  }
}

/** How the reminders line reads now. */
function nextSaid(clock: Clock, context: Context): string {
  if (context.remind === "off") return "No reminders: Remind me is Off.";
  if (!context.waits) return "No reminders: no Blocking ask waits.";
  if (clock.next === null) return "No reminder scheduled.";
  const gap = (reminderGap(context.remind, clock.step) ?? 0) / MINUTE;
  const held = context.held === undefined ? "" : `; it will not bounce: ${context.held}`;
  return `Next reminder in ${clockSaid(clock.next - clock.now)} (gap ${gap} min)${held}.`;
}

/** The story's clock ticks four times a second, `speed` times as fast as real time. */
const TICK = 250;

// --- The prototype ------------------------------------------------------------------------------

type Moved = "chat" | "workspaceRest" | "otherWorkspaces";

const MOVED: { key: Moved; says: string }[] = [
  { key: "chat", says: "Focused chat and subtree" },
  { key: "workspaceRest", says: "Rest of this workspace" },
  { key: "otherWorkspaces", says: "Other workspaces" },
];

type Flag = "oneWorkspace" | "noChatFocused" | "reduced" | "quiet" | "typing" | "inFront";

const FLAGS: { key: Flag; says: string }[] = [
  { key: "oneWorkspace", says: "One workspace" },
  { key: "noChatFocused", says: "No chat focused" },
  { key: "reduced", says: "Reduced motion" },
  { key: "quiet", says: "Quiet" },
  { key: "typing", says: "Typing" },
  { key: "inFront", says: "Window in front" },
];

/** One count's row in the controls: its name, its number, and the presses that move it. */
function Stepper({
  says,
  value,
  onMove,
}: {
  says: string;
  value: number;
  onMove: (by: number) => void;
}) {
  return (
    <div className="asks-proto-controls-row">
      <span className="asks-proto-controls-name">{says}</span>
      <span className="asks-proto-controls-value">{value}</span>
      <ButtonRow size="sm">
        <Button aria-label={`${says} −1`} onPress={() => onMove(-1)}>
          −1
        </Button>
        <Button aria-label={`${says} +1`} onPress={() => onMove(1)}>
          +1
        </Button>
        <Button aria-label={`${says} +100`} onPress={() => onMove(100)}>
          +100
        </Button>
        <Button aria-label={`${says} to 0`} onPress={() => onMove(-value)}>
          0
        </Button>
      </ButtonRow>
    </div>
  );
}

/** **The prototype**: the rail, the Asks view and the right bar, then the box of controls. */
export function AsksScopesPrototype({
  start,
  setup: given,
}: {
  start: Counts;
  setup?: Partial<Setup>;
}) {
  const [counts, setCounts] = useState(start);
  const [setup, setSetup] = useState<Setup>({ ...SETUP, ...given });
  const [picked, setPicked] = useState<Scope>("chat");
  const scoped = scopeCounts(counts, setup);
  // The ✋ and this project's tab count the project in front, never more than Project.
  const total = scoped.project;
  const waiting: Record<string, number> = { [HERE]: total, ...counts.others };
  const [clock, dispatch] = useReducer(clockAfter, undefined, () =>
    restarted(
      { now: 0, step: 0, next: null, bounces: 0, last: "none yet" },
      contextOf(scopeCounts(start, { ...SETUP, ...given }).project, { ...SETUP, ...given }),
    ),
  );
  /** The bounces whose animation ended: the ✋ bounces while fewer have ended than were drawn. */
  const [ended, setEnded] = useState(clock.bounces);
  // The clock's tick reads the setup as it is when the tick comes, kept here after each render.
  const context = useRef(contextOf(total, setup));
  useEffect(() => {
    context.current = contextOf(total, setup);
  });
  useEffect(() => {
    const every = window.setInterval(
      () => dispatch({ type: "tick", by: TICK * setup.speed, context: context.current }),
      TICK,
    );
    return () => window.clearInterval(every);
  }, [setup.speed]);
  /** New counts: a new ask bounces the ✋ and starts the backoff again, an answer starts it. */
  const recount = (next: Counts) => {
    setCounts(next);
    const to = scopeCounts(next, setup).project;
    if (to !== total) dispatch({ type: "count", up: to > total, context: contextOf(to, setup) });
  };
  const move = (key: Moved, by: number) =>
    recount({ ...counts, [key]: Math.max(0, counts[key] + by) });
  const moveOther = (name: Other, by: number) =>
    recount({
      ...counts,
      others: { ...counts.others, [name]: Math.max(0, counts.others[name] + by) },
    });
  const set = <K extends keyof Setup>(key: K, to: Setup[K]) => {
    const next = { ...setup, [key]: to };
    setSetup(next);
    // What the schedule is for changed: whether a Blocking ask waits, or how often to remind.
    if (key === "remind" || key === "blocking" || key === "oneWorkspace")
      dispatch({
        type: "restart",
        context: contextOf(scopeCounts(counts, next).project, next),
      });
  };
  const openAsks = () => dispatch({ type: "restart", context: contextOf(total, setup) });
  const here = contextOf(total, setup);
  return (
    <div className="asks-proto" data-slide-ms={setup.slideMs}>
      <div className="asks-proto-window">
        <ProjectRail waiting={waiting} still={setup.reduced} />
        <div
          className="asks-proto-side region-view"
          id="asks-proto-panel-asks"
          {...tabPanelOf("asks-proto-tab-asks")}
        >
          <AsksView
            counts={counts}
            setup={setup}
            picked={picked}
            onPick={setPicked}
            onQuiet={(on) => set("quiet", on)}
          />
        </div>
        {BAR.filter(({ view }) => view !== "asks").map(({ view, name }) => (
          <div
            key={view}
            id={`asks-proto-panel-${view}`}
            {...tabPanelOf(`asks-proto-tab-${view}`)}
            hidden
          >
            {name}
          </div>
        ))}
        <RightBar
          total={total}
          still={setup.reduced}
          bouncing={clock.bounces > ended}
          bounces={clock.bounces}
          onBounced={() => setEnded(clock.bounces)}
        />
      </div>

      <section className="asks-proto-controls" aria-label="Prototype controls">
        <h2 className="asks-proto-controls-head">Prototype controls: not part of the design</h2>
        {MOVED.map(({ key, says }) => (
          <Stepper key={key} says={says} value={counts[key]} onMove={(by) => move(key, by)} />
        ))}
        {OTHERS.map((name) => (
          <Stepper
            key={name}
            says={`Project ${name}`}
            value={counts.others[name]}
            onMove={(by) => moveOther(name, by)}
          />
        ))}
        <div className="asks-proto-controls-row">
          {FLAGS.map(({ key, says }) => (
            <Switch key={key} isSelected={setup[key]} onChange={(on) => set(key, on)}>
              {says}
            </Switch>
          ))}
          <Switch isSelected={setup.blocking} onChange={(on) => set("blocking", on)}>
            A Blocking ask waits
          </Switch>
        </div>
        <div className="asks-proto-controls-row">
          <span className="asks-proto-controls-name">Remind me</span>
          <SegmentedControl
            size="sm"
            aria-label="Remind me"
            value={setup.remind}
            onChange={(to) => set("remind", to as Remind)}
          >
            <Segment value="off">Off</Segment>
            <Segment value="gently">Gently</Segment>
            <Segment value="often">Often</Segment>
          </SegmentedControl>
        </div>
        <div className="asks-proto-controls-row">
          <span className="asks-proto-controls-name">Time speed</span>
          <SegmentedControl
            size="sm"
            aria-label="Time speed"
            value={String(setup.speed)}
            onChange={(to) => set("speed", Number(to) as Speed)}
          >
            {([1, 60, 600] as const).map((speed) => (
              <Segment key={speed} value={String(speed)}>{`${speed}×`}</Segment>
            ))}
          </SegmentedControl>
          <Button size="sm" onPress={openAsks}>
            Open Asks (the backoff starts again)
          </Button>
        </div>
        <p className="asks-proto-reminder">{nextSaid(clock, here)}</p>
        <p className="asks-proto-state">{`Clock ${clockSaid(clock.now)} · last bounce: ${clock.last}`}</p>
        <div className="asks-proto-controls-row">
          <span className="asks-proto-controls-name">Slide</span>
          <SegmentedControl
            size="sm"
            aria-label="Slide length"
            value={String(setup.slideMs)}
            onChange={(to) => set("slideMs", Number(to) as SlideMs)}
          >
            {([150, 200, 280] as const).map((ms) => (
              <Segment key={ms} value={String(ms)}>{`${ms} ms`}</Segment>
            ))}
          </SegmentedControl>
        </div>
        <p className="asks-proto-state">
          {`chips ${JSON.stringify(scoped)} · ✋ ${total} · tabs ${JSON.stringify(waiting)} · lists ${listedScope(picked, setup)}`}
        </p>
      </section>
    </div>
  );
}
