// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * PROTOTYPE, throwaway (#4582, map #4575). One tab, the strip and its icons, drawn with 24 chats in
 * two workspaces and the plane root. The persona-and-state icon on every row, tab and pane head is
 * the real `ChatPersonaState`; buttons, counts, cards and the empty state are the real `app/src/ui`.
 * The terminal, the title bar's chips and the sidebar's row frame are stand-ins. The dashed bar at
 * the bottom is the prototype's, not the design.
 *
 * URL: `?s=<scene>` the scene, `&v=A|B|C` the strip, `&bare` no bar.
 */
import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import {
  ChevronLeft,
  ChevronRight,
  Columns2,
  FolderPlus,
  Hand,
  House,
  MessageSquarePlus,
  MoreHorizontal,
  Pin,
  Plus,
  Settings,
  SquareArrowOutUpRight,
  X,
} from "lucide-react";
import { ChatPersonaState } from "../ChatRows";
import { ChatsHere, fixedChats, nothingKnown, type ChatStates, type State } from "../chatState";
import { Badge, Count } from "../ui/badge/Badge";
import { Button, IconButton } from "../ui/button/Button";
import { Card } from "../ui/card/Card";
import { Segment, SegmentedControl } from "../ui/choice/Radio";
import { EmptyState } from "../ui/empty-state/EmptyState";
import { Tooltip } from "../ui/tooltip/Tooltip";
import "./prototype.css";

// ───────────────────────────── the world ─────────────────────────────

type Ws = "smart-ide" | "volaticloud" | "@root";
const WS_NAME: Record<Ws, string> = { "smart-ide": "smart-ide", volaticloud: "volaticloud", "@root": "purlis" };
const CHIPS: Ws[] = ["smart-ide", "volaticloud"];

type Chat = {
  id: number;
  name: string;
  persona: string | null;
  ws: Ws;
  parent?: number;
  state: State;
  needs?: boolean;
  report?: "owed" | null;
};
type Content = { k: "chat"; chat: number } | { k: "empty" } | { k: "view"; view: string };
type Pane = { id: number; c: Content; used: number };
type Tab = { id: number; panes: Pane[]; focus: number; pinned?: boolean };
type Place = { tabs: Tab[]; front: number };
type Card_ = { ws: Ws; chat: number; from: number; open: boolean };
type World = {
  chats: Chat[];
  places: Record<Ws, Place>;
  front: Ws;
  clock: number;
  fresh: number[];
  card: Card_ | null;
  noWorkspaces?: boolean;
  next: number;
};

const C = (id: number, name: string, persona: string | null, ws: Ws, state: State, more: Partial<Chat> = {}): Chat => ({
  id,
  name,
  persona,
  ws,
  state,
  ...more,
});

/** 24 chats: 14 in smart-ide, 6 in volaticloud, 4 in the plane root. */
function chats(): Chat[] {
  return [
    C(1, "steward 4", "steward", "smart-ide", "running"),
    C(2, "developer 7", "developer", "smart-ide", "running", { parent: 1, report: "owed" }),
    C(3, "designer 3", "designer", "smart-ide", "waiting", { parent: 1, report: "owed", needs: true }),
    C(4, "architect 2", "architect", "smart-ide", "running", { parent: 1, report: "owed" }),
    C(5, "developer 9", "developer", "smart-ide", "running", { parent: 4, report: "owed" }),
    C(6, "qa 5", "qa", "smart-ide", "done", { parent: 5, report: "owed" }),
    C(7, "reviewer 1", "reviewer", "smart-ide", "waiting"),
    C(8, "talk 2", null, "smart-ide", "running"),
    C(9, "steward 5", "steward", "smart-ide", "waiting"),
    C(10, "developer 11", "developer", "smart-ide", "failed", { parent: 9, report: "owed" }),
    C(11, "designer 4", "designer", "smart-ide", "running"),
    C(12, "dev server", null, "smart-ide", "running"),
    C(13, "steward 6", "steward", "smart-ide", "waiting"),
    C(14, "researcher 2", "researcher", "smart-ide", "running", { parent: 13, report: "owed" }),
    C(21, "devops 3", "devops", "volaticloud", "running"),
    C(22, "steward 2", "steward", "volaticloud", "waiting"),
    C(23, "developer 3", "developer", "volaticloud", "running", { parent: 22, report: "owed" }),
    C(24, "security 1", "security", "volaticloud", "waiting", { needs: true }),
    C(25, "devops 4", "devops", "volaticloud", "done"),
    C(26, "shell 3", null, "volaticloud", "waiting"),
    C(31, "steward 1", "steward", "@root", "waiting"),
    C(32, "steward 3", "steward", "@root", "running"),
    C(33, "curator 1", "curator", "@root", "running", { parent: 32, report: "owed" }),
    C(34, "librarian 2", "librarian", "@root", "waiting"),
  ];
}

let ids = 100;
const pane = (c: Content, used = 0): Pane => ({ id: ++ids, c, used });
const tab = (panes: Pane[], more: Partial<Tab> = {}): Tab => ({ id: ++ids, panes, focus: panes[0]!.id, ...more });
const one = (c: Content): Place => {
  const t = tab([pane(c, 1)]);
  return { tabs: [t], front: t.id };
};
const chat = (id: number): Content => ({ k: "chat", chat: id });

function base(): World {
  return {
    chats: chats(),
    places: { "smart-ide": one(chat(1)), volaticloud: one(chat(21)), "@root": one(chat(31)) },
    front: "smart-ide",
    clock: 10,
    fresh: [],
    card: null,
    next: 200,
  };
}

// ───────────────────────────── the scenes ─────────────────────────────

type Scene = { key: string; title: string; says: string; make: () => World; all?: boolean; event?: { label: string; run: (w: World) => void } };

const SCENES: Scene[] = [
  {
    key: "one",
    title: "24 chats, one tab",
    says: "After the upgrade: each workspace keeps one tab, showing the chat that was in front. Every other chat lives in the sidebar. Press a row: the pane switches, the tab stays.",
    make: base,
  },
  {
    key: "split",
    title: "A split tab",
    says: "steward 4 with its task developer 7 beside it, in one tab. The tab says the focused pane's chat and how many more it holds; hover it for all of them. Click the other pane to move focus and watch the tab change.",
    make: () => {
      const w = base();
      const a = pane(chat(1), 2);
      const b = pane(chat(2), 1);
      w.places["smart-ide"] = { tabs: [tab([a, b], { focus: a.id })], front: 0 };
      w.places["smart-ide"].front = w.places["smart-ide"].tabs[0]!.id;
      return w;
    },
  },
  {
    key: "tabs",
    title: "Several tabs",
    says: "Tabs appear only for an explicit Open in new tab, a view, or a drag onto the strip. Here: the main tab, a pinned split, a chat opened in its own tab, and Settings. A press on a row while Settings is in front goes to the most recently used chat pane, and its tab comes forward.",
    make: () => {
      const w = base();
      const main = tab([pane(chat(1), 3)]);
      const s1 = pane(chat(9), 2);
      const s2 = pane(chat(10), 1);
      const split = tab([s1, s2], { focus: s1.id, pinned: true });
      const own = tab([pane(chat(7), 1)]);
      const view = tab([pane({ k: "view", view: "Settings" }, 4)]);
      w.places["smart-ide"] = { tabs: [split, main, own, view], front: view.id };
      return w;
    },
  },
  {
    key: "empty",
    title: "The empty pane",
    says: "The last pane of the last tab was closed: the tab stays, holding an empty pane. All three of today's empty windows become this one, and the chats are still in the sidebar.",
    make: () => {
      const w = base();
      w.places["smart-ide"] = one({ k: "empty" });
      return w;
    },
  },
  {
    key: "fresh",
    title: "A new workspace",
    says: "A workspace with no chat yet: its one tab holds the empty pane. The sidebar's Workspace list says it is empty. (Today: empty-workspace, 'No chats in this workspace'.)",
    make: () => {
      const w = base();
      w.chats = w.chats.filter((c) => c.ws !== "volaticloud");
      w.places.volaticloud = one({ k: "empty" });
      w.front = "volaticloud";
      return w;
    },
  },
  {
    key: "plane",
    title: "A plane with no workspace",
    says: "The plane root keeps its one tab too. With no workspace yet its empty pane offers Create a workspace beside New chat. (Today: empty-plane.)",
    make: () => {
      const w = base();
      w.chats = [];
      w.noWorkspaces = true;
      w.places["@root"] = one({ k: "empty" });
      w.front = "@root";
      return w;
    },
  },
  {
    key: "handoff",
    title: "A handoff arrives",
    all: true,
    says: "steward 4 handed work to volaticloud. No pane is taken: a card hangs from volaticloud's chip, its row says new, and the Inbox gets an Update (not counted on ✋). Press anywhere else and the card goes, leaving a dot on the chip; press the dot to bring it back.",
    make: () => {
      const w = base();
      arrive(w);
      return w;
    },
    event: { label: "Another handoff", run: (w) => arrive(w) },
  },
  {
    key: "ask",
    title: "An ask from a chat in no pane",
    says: "designer 3 and security 1 need you and are in no pane. The ask never takes a pane or focus: it's on the row, the rows above it, ✋ and the chip's count, and you answer it in the Inbox.",
    make: base,
    event: {
      label: "reviewer 1 asks",
      run: (w) => {
        const c = w.chats.find((x) => x.id === 7);
        if (c) {
          c.needs = true;
          c.state = "waiting";
        }
      },
    },
  },
];

function arrive(w: World) {
  const id = w.next++;
  const n = w.chats.filter((c) => c.persona === "devops").length + 3;
  w.chats.push(C(id, `devops ${n}`, "devops", "volaticloud", "running"));
  w.fresh.push(id);
  w.card = { ws: "volaticloud", chat: id, from: 1, open: true };
}

// ───────────────────────────── the rules ─────────────────────────────

const byId = (w: World, id: number) => w.chats.find((c) => c.id === id)!;
const panesOf = (p: Place) => p.tabs.flatMap((t) => t.panes.map((pn) => ({ t, pn })));
const frontTab = (p: Place) => p.tabs.find((t) => t.id === p.front) ?? p.tabs[0]!;
const focused = (t: Tab) => t.panes.find((pn) => pn.id === t.focus) ?? t.panes[0]!;
const shownIn = (w: World, id: number) => {
  for (const ws of Object.keys(w.places) as Ws[]) {
    const hit = panesOf(w.places[ws]).find(({ pn }) => pn.c.k === "chat" && pn.c.chat === id);
    if (hit) return { ws, ...hit };
  }
  return undefined;
};
const frontChat = (w: World): number | undefined => {
  const pn = focused(frontTab(w.places[w.front]));
  return pn.c.k === "chat" ? pn.c.chat : undefined;
};

/** A sidebar press: rules 1 to 4 of #4588 and D1. */
function pick(w: World, id: number) {
  w.clock++;
  w.fresh = w.fresh.filter((x) => x !== id);
  if (w.card?.chat === id) w.card = null;
  const on = shownIn(w, id);
  if (on) {
    w.front = on.ws;
    w.places[on.ws].front = on.t.id;
    on.t.focus = on.pn.id;
    on.pn.used = w.clock;
    return;
  }
  const ws = byId(w, id).ws;
  w.front = ws;
  const place = w.places[ws];
  let t = frontTab(place);
  let pn = focused(t);
  if (pn.c.k === "view") {
    const best = panesOf(place)
      .filter(({ pn: p }) => p.c.k !== "view")
      .sort((a, b) => b.pn.used - a.pn.used)[0];
    if (best) {
      t = best.t;
      pn = best.pn;
    } else {
      pn = pane({ k: "empty" });
      t = tab([pn]);
      place.tabs.push(t);
    }
  }
  pn.c = chat(id);
  pn.used = w.clock;
  place.front = t.id;
  t.focus = pn.id;
}

function closePane(w: World, ws: Ws, t: Tab, pn: Pane) {
  const place = w.places[ws];
  if (t.panes.length > 1) {
    t.panes = t.panes.filter((x) => x !== pn);
    t.focus = t.panes[0]!.id;
    return;
  }
  if (place.tabs.length > 1) {
    place.tabs = place.tabs.filter((x) => x !== t);
    place.front = place.tabs.slice().sort((a, b) => focused(b).used - focused(a).used)[0]!.id;
    return;
  }
  pn.c = { k: "empty" };
}

function closeTab(w: World, ws: Ws, t: Tab) {
  const place = w.places[ws];
  if (place.tabs.length === 1) {
    t.panes = [pane({ k: "empty" }, w.clock)];
    t.focus = t.panes[0]!.id;
    return;
  }
  place.tabs = place.tabs.filter((x) => x !== t);
  if (place.front === t.id) place.front = place.tabs[0]!.id;
}

function newChat(w: World, ws: Ws, t: Tab, pn: Pane) {
  const id = w.next++;
  const n = w.chats.filter((c) => c.persona === "steward").length + 4;
  w.chats.push(C(id, `steward ${n}`, "steward", ws, "running"));
  w.clock++;
  pn.c = chat(id);
  pn.used = w.clock;
  t.focus = pn.id;
}

function openInNewTab(w: World, id: number) {
  const on = shownIn(w, id);
  if (on) {
    pick(w, id);
    return;
  }
  const ws = byId(w, id).ws;
  w.clock++;
  const t = tab([pane(chat(id), w.clock)]);
  w.places[ws].tabs.push(t);
  w.places[ws].front = t.id;
  w.front = ws;
  w.fresh = w.fresh.filter((x) => x !== id);
}

function statesOf(w: World): ChatStates {
  const bySession: Record<number, State> = {};
  for (const c of w.chats) bySession[c.id] = c.state;
  return { ...nothingKnown, bySession, needsYou: w.chats.filter((c) => c.needs).map((c) => c.id) };
}

// ───────────────────────────── drawing ─────────────────────────────

export type Look = { appearance: "light" | "dark"; theme: string };
type Variant = "A" | "B" | "C";
const VARIANTS: Record<Variant, string> = {
  A: "Name tabs",
  B: "No strip for one tab",
  C: "Layout tabs",
};

function Icon({ w, id }: { w: World; id: number }) {
  const c = byId(w, id);
  return (
    <ChatPersonaState
      persona={c.persona}
      session={c.id}
      shell={c.persona === null}
      report={c.report ?? null}
      harness="Claude Code"
    />
  );
}

function contentName(w: World, c: Content) {
  return c.k === "chat" ? byId(w, c.chat).name : c.k === "view" ? c.view : "New chat";
}

function ContentIcon({ w, c }: { w: World; c: Content }) {
  if (c.k === "chat") return <Icon w={w} id={c.chat} />;
  if (c.k === "view") return <Settings className="ot-glyph" aria-hidden="true" />;
  return <Plus className="ot-glyph" aria-hidden="true" />;
}

type Act = (run: (w: World) => void) => void;

/** A tab on the strip, in the variant's words. */
function StripTab({ w, ws, t, v, act }: { w: World; ws: Ws; t: Tab; v: Variant; act: Act }) {
  const place = w.places[ws];
  const sel = place.front === t.id;
  const f = focused(t);
  const all = t.panes.map((p) => contentName(w, p.c));
  const tip = t.panes.length > 1 ? all.join(" · ") : all[0]!;
  const body =
    v === "C" ? (
      <span className="ot-layout" data-n={t.panes.length}>
        {t.panes.map((p) => (
          <span key={p.id} className="ot-cell" data-focus={p.id === t.focus || undefined}>
            <ContentIcon w={w} c={p.c} />
          </span>
        ))}
        {t.panes.length === 1 && <span className="ot-tab-name">{all[0]}</span>}
      </span>
    ) : (
      <>
        <ContentIcon w={w} c={f.c} />
        <span className="ot-tab-name">{contentName(w, f.c)}</span>
        {t.panes.length > 1 && (
          <span className="ot-split-more">
            <Columns2 aria-hidden="true" />+{t.panes.length - 1}
          </span>
        )}
      </>
    );
  return (
    <div className="ot-tab" role="tab" aria-selected={sel} data-pinned={t.pinned || undefined}>
      <Tooltip tip={tip}>
        <button className="ot-tab-press" onClick={() => act((d) => void (d.places[ws].front = t.id))}>
          {t.pinned && <Pin className="ot-pin" aria-hidden="true" />}
          {body}
        </button>
      </Tooltip>
      <button
        className="ot-tab-x"
        aria-label="Close tab"
        title="Close tab: its chats keep running in the sidebar"
        onClick={() => act((d) => closeTab(d, ws, d.places[ws].tabs.find((x) => x.id === t.id)!))}
      >
        <X aria-hidden="true" />
      </button>
    </div>
  );
}

function Strip({ w, v, act }: { w: World; v: Variant; act: Act }) {
  const place = w.places[w.front];
  if (v === "B" && place.tabs.length === 1) return null;
  return (
    <div className="ot-strip" role="tablist" aria-label="Tabs">
      {place.tabs.map((t) => (
        <StripTab key={t.id} w={w} ws={w.front} t={t} v={v} act={act} />
      ))}
      <Tooltip tip="New tab">
        <button className="ot-strip-plus" aria-label="New tab" onClick={() => act((d) => openEmptyTab(d))}>
          <Plus aria-hidden="true" />
        </button>
      </Tooltip>
    </div>
  );
}

function openEmptyTab(w: World) {
  const t = tab([pane({ k: "empty" }, ++w.clock)]);
  w.places[w.front].tabs.push(t);
  w.places[w.front].front = t.id;
}

function PaneView({ w, ws, t, pn, act, v, alone }: { w: World; ws: Ws; t: Tab; pn: Pane; act: Act; v: Variant; alone: boolean }) {
  const isFocus = t.focus === pn.id && t.panes.length > 1;
  const find = (d: World) => {
    const tt = d.places[ws].tabs.find((x) => x.id === t.id)!;
    return { tt, pp: tt.panes.find((x) => x.id === pn.id)! };
  };
  const focus = () =>
    act((d) => {
      const { tt, pp } = find(d);
      tt.focus = pp.id;
      pp.used = ++d.clock;
    });
  const c = pn.c;
  return (
    <section className="ot-pane" data-focus={isFocus || undefined} onMouseDown={t.panes.length > 1 ? focus : undefined}>
      {c.k !== "empty" && (
        <header className="ot-pane-head">
          <ContentIcon w={w} c={c} />
          <span className="ot-pane-name">{contentName(w, c)}</span>
          {c.k === "chat" && <span className="ot-pane-where">Claude Code · {WS_NAME[ws]}</span>}
          <span className="ot-grow" />
          {c.k === "chat" && (
            <IconButton aria-label="Chat actions" tooltip="Chat actions" size="sm" variant="ghost" icon={<MoreHorizontal aria-hidden="true" />} />
          )}
          <IconButton
            aria-label="Split"
            tooltip="Split: an empty pane beside"
            size="sm"
            variant="ghost"
            icon={<Columns2 aria-hidden="true" />}
            onPress={() =>
              act((d) => {
                const { tt, pp } = find(d);
                const n = pane({ k: "empty" }, ++d.clock);
                tt.panes.splice(tt.panes.indexOf(pp) + 1, 0, n);
                tt.focus = n.id;
              })
            }
          />
          <IconButton
            aria-label="Close pane"
            tooltip={c.k === "chat" ? "Close pane: the chat keeps running in the sidebar" : "Close"}
            size="sm"
            variant="ghost"
            icon={<X aria-hidden="true" />}
            onPress={() =>
              act((d) => {
                const { tt, pp } = find(d);
                closePane(d, ws, tt, pp);
              })
            }
          />
        </header>
      )}
      <div className="ot-pane-body">
        {c.k === "chat" ? (
          <Terminal w={w} id={c.chat} />
        ) : c.k === "view" ? (
          <div className="ot-view">
            <h2>Settings</h2>
            <p className="ui-text-secondary">A view keeps its tab. A press on a sidebar row goes to the most recently used chat pane instead.</p>
          </div>
        ) : (
          <EmptyPane w={w} ws={ws} alone={alone} t={t} act={act} onNew={() => act((d) => newChat(d, ws, find(d).tt, find(d).pp))} onClose={() => act((d) => closePane(d, ws, find(d).tt, find(d).pp))} v={v} />
        )}
      </div>
    </section>
  );
}

function EmptyPane({
  w,
  ws,
  alone,
  t,
  onNew,
  onClose,
}: {
  w: World;
  ws: Ws;
  alone: boolean;
  t: Tab;
  act: Act;
  v: Variant;
  onNew: () => void;
  onClose: () => void;
}) {
  const running = w.chats.filter((c) => c.ws === ws).length;
  const body =
    w.noWorkspaces === true
      ? "A workspace holds the repos you work on and the chats about them. A chat here starts in the plane itself."
      : running > 0
        ? `${running} chats are running here. Pick one in the sidebar to show it in this pane.`
        : "Nothing runs in this workspace yet.";
  return (
    <div className="ot-empty">
      {!alone && (
        <div className="ot-empty-x">
          <IconButton aria-label="Close pane" tooltip="Close pane" size="sm" variant="ghost" icon={<X aria-hidden="true" />} onPress={onClose} />
        </div>
      )}
      <EmptyState
        mark={w.noWorkspaces ? FolderPlus : MessageSquarePlus}
        headline={w.noWorkspaces ? "No workspaces yet" : t.panes.length > 1 ? "An empty pane" : "Nothing in this pane"}
        body={body}
        action={
          <>
            {w.noWorkspaces && (
              <Button variant="primary" onPress={() => undefined}>
                Create a workspace
              </Button>
            )}
            <Button variant="primary" onPress={onNew}>
              New chat
            </Button>
          </>
        }
      />
    </div>
  );
}

const LINES: Record<string, string[]> = {
  running: ["● Reading app/src/PlaneView.tsx (10,412 lines)", "● Searching for ownTab callers", "  3 results", "✻ Working…"],
  waiting: ["● Done. The tests pass.", "", "❯ "],
  done: ["● Reported to steward 4.", "", "❯ "],
  failed: ["● cargo test failed: 2 tests", "", "❯ "],
};
function Terminal({ w, id }: { w: World; id: number }) {
  const c = byId(w, id);
  return (
    <pre className="ot-term" aria-label={`${c.name}'s terminal (stand-in)`}>
      {`${c.name} · ${c.persona ?? "shell"}\n\n` + (c.needs ? ["● I need your call on the layout before I go on.", "", "  ✋ Waiting for you"] : LINES[c.state] ?? LINES.waiting!).join("\n")}
    </pre>
  );
}

/** The panes of the front tab, side by side (the tree is the pane prototype's, #4583). */
function Panes({ w, v, act }: { w: World; v: Variant; act: Act }) {
  const place = w.places[w.front];
  const t = frontTab(place);
  return (
    <div className="ot-panes">
      {t.panes.map((pn) => (
        <PaneView key={pn.id} w={w} ws={w.front} t={t} pn={pn} act={act} v={v} alone={place.tabs.length === 1 && t.panes.length === 1} />
      ))}
    </div>
  );
}

/** The sidebar: Workspace and All (This tab goes, D5). The row's frame is a stand-in. */
function Sidebar({ w, act, scope, onScope, onPick }: { w: World; act: Act; scope: string; onScope: (s: string) => void; onPick: (id: number) => void }) {
  const order: Ws[] = ["smart-ide", "volaticloud", "@root"];
  const list = (scope === "all" ? w.chats : w.chats.filter((c) => c.ws === w.front))
    .slice()
    .sort((a, b) => order.indexOf(a.ws) - order.indexOf(b.ws));
  const front = frontChat(w);
  const onScreen = new Set(
    frontTab(w.places[w.front]).panes.flatMap((p) => (p.c.k === "chat" ? [p.c.chat] : [])),
  );
  const tree: { c: Chat; level: number }[] = [];
  const walk = (parent: number | undefined, level: number) => {
    for (const c of list.filter((x) => x.parent === parent)) {
      tree.push({ c, level });
      walk(c.id, level + 1);
    }
  };
  walk(undefined, 0);
  const needsBelow = (id: number): boolean =>
    w.chats.some((x) => x.parent === id && (x.needs || needsBelow(x.id)));
  let lastWs: Ws | undefined;
  return (
    <nav className="ot-side" aria-label="Chats">
      <div className="ot-side-head">
        <span>Chats</span>
        <SegmentedControl aria-label="Which chats" size="sm" value={scope} onChange={onScope}>
          <Segment value="ws">Workspace</Segment>
          <Segment value="all">All</Segment>
        </SegmentedControl>
      </div>
      {tree.length === 0 && <p className="ot-side-none ui-text-secondary">No chats in this workspace yet.</p>}
      <ul role="tree">
        {tree.map(({ c, level }) => {
          const head = scope === "all" && level === 0 && c.ws !== lastWs;
          if (level === 0) lastWs = c.ws;
          const fresh = w.fresh.includes(c.id);
          return (
            <li key={c.id} role="none" className="ot-li">
              {head && <div className="ot-side-ws">{WS_NAME[c.ws]}</div>}
              <div
                role="treeitem"
                aria-current={c.id === front ? "true" : undefined}
                className="ot-row chat"
                data-current={c.id === front || undefined}
                data-shown={(onScreen.has(c.id) && c.id !== front) || undefined}
                style={{ paddingInlineStart: 8 + level * 14 }}
                onClick={() => onPick(c.id)}
              >
                <Icon w={w} id={c.id} />
                <span className="ot-row-name">{c.name}</span>
                {fresh && <Badge variant="accent">new</Badge>}
                {!c.needs && needsBelow(c.id) && (
                  <Tooltip tip="A chat below needs you">
                    <Hand className="ot-row-hand" aria-label="A chat below needs you" />
                  </Tooltip>
                )}
                <span className="ot-grow" />
                <Tooltip tip="Open in new tab">
                  <button
                    className="ot-row-tab"
                    aria-label={`Open ${c.name} in new tab`}
                    onClick={(e) => {
                      e.stopPropagation();
                      act((d) => openInNewTab(d, c.id));
                    }}
                  >
                    <SquareArrowOutUpRight aria-hidden="true" />
                  </button>
                </Tooltip>
              </div>
            </li>
          );
        })}
      </ul>
    </nav>
  );
}

/** The title bar: the root's house, the workspaces' chips, ✋. The chips are stand-ins of #3647's. */
function TitleBar({ w, act }: { w: World; act: Act }) {
  const needs = w.chats.filter((c) => c.needs).length;
  const chips: Ws[] = w.noWorkspaces ? [] : CHIPS;
  return (
    <header className="ot-title">
      <span className="ot-lights" aria-hidden="true">
        <i />
        <i />
        <i />
      </span>
      <button
        className="ot-chip ot-house"
        aria-current={w.front === "@root" ? "page" : undefined}
        aria-label="purlis plane root"
        onClick={() => act((d) => void (d.front = "@root"))}
      >
        <House aria-hidden="true" />
      </button>
      {chips.map((ws) => {
        const count = w.chats.filter((c) => c.ws === ws && c.needs).length;
        const dot = w.card !== null && w.card.ws === ws && !w.card.open;
        return (
          <span key={ws} className="ot-chip-wrap" data-chip={ws}>
            <button
              className="ot-chip"
              aria-current={w.front === ws ? "page" : undefined}
              onClick={() => act((d) => void (d.front = ws))}
            >
              {WS_NAME[ws]}
              {count > 0 && (
                <Count variant="waiting" said={`${count} need you`}>
                  {count}
                </Count>
              )}
            </button>
            {dot && (
              <button
                className="ot-dot"
                aria-label="A handoff arrived: show it"
                onClick={() =>
                  act((d) => {
                    if (d.card) d.card.open = true;
                  })
                }
              />
            )}
          </span>
        );
      })}
      <span className="ot-grow" />
      <Tooltip tip={`${needs} need you (Inbox)`}>
        <button className="ot-hand" aria-label={`${needs} need you`}>
          <Hand aria-hidden="true" />
          {needs > 0 && <Count variant="waiting">{needs}</Count>}
        </button>
      </Tooltip>
    </header>
  );
}

function HandoffCard({ w, act }: { w: World; act: Act }) {
  const card = w.card;
  const [pos, setPos] = useState<{ left: number; top: number } | null>(null);
  useLayoutEffect(() => {
    const at = card ? document.querySelector(`[data-chip="${card.ws}"]`) : null;
    if (!at) return;
    const r = at.getBoundingClientRect();
    setPos({ left: r.left, top: r.bottom + 6 });
  }, [card?.ws, card?.open, card?.chat]);
  if (!card || !card.open || !pos) return null;
  const c = byId(w, card.chat);
  return (
    <div className="ot-card-at" style={pos} onMouseDown={(e) => e.stopPropagation()}>
      <Card variant="raised" className="ot-card">
        <div className="ot-card-head">
          <Icon w={w} id={c.id} />
          <span>
            <strong>{c.name}</strong> arrived from {byId(w, card.from).name}
          </span>
          <span className="ot-grow" />
          <IconButton
            aria-label="Dismiss"
            tooltip="Dismiss"
            size="sm"
            variant="ghost"
            icon={<X aria-hidden="true" />}
            onPress={() => act((d) => void (d.card = null))}
          />
        </div>
        <p className="ui-text-secondary">A handoff to {WS_NAME[card.ws]}. It runs in the sidebar and took no pane.</p>
        <div className="ot-card-acts">
          <Button size="sm" variant="primary" onPress={() => act((d) => pick(d, card.chat))}>
            Go to it
          </Button>
        </div>
      </Card>
    </div>
  );
}

// ───────────────────────────── the page ─────────────────────────────

function readUrl() {
  const q = new URLSearchParams(location.search);
  const s = SCENES.find((x) => x.key === q.get("s")) ?? SCENES[0]!;
  const v = (["A", "B", "C"] as const).find((x) => x === q.get("v")) ?? "A";
  return { s, v, bare: q.has("bare") };
}

export function OneTab({ look, onLook }: { look: Look; onLook: (look: Look) => void }) {
  const first = useMemo(readUrl, []);
  const [scene, setScene] = useState<Scene>(first.s);
  const [v, setV] = useState<Variant>(first.v);
  const [w, setW] = useState<World>(() => first.s.make());
  const [scope, setScope] = useState(first.s.all ? "all" : "ws");
  const [timing, setTiming] = useState<{ ms: number; frames: number; what: string }[]>([]);
  const pressAt = useRef<{ t: number; what: string } | null>(null);

  const act: Act = useCallback((run) => {
    setW((prev) => {
      const next = structuredClone(prev);
      run(next);
      return next;
    });
  }, []);

  const go = useCallback((s: Scene, nv: Variant = v) => {
    setScene(s);
    setV(nv);
    setW(s.make());
    setScope(s.all ? "all" : "ws");
    const q = new URLSearchParams(location.search);
    q.set("s", s.key);
    q.set("v", nv);
    try {
      history.replaceState(null, "", `?${q}`);
    } catch {
      // A sandboxed frame (the artifact page) refuses a query string: the bar still works.
    }
  }, [v]);

  const onPick = useCallback(
    (id: number) => {
      pressAt.current = { t: performance.now(), what: `pick ${id}` };
      act((d) => pick(d, id));
    },
    [act],
  );

  // Press to drawn frame, for the sidebar pick (#4579): the prototype's own React work only.
  useLayoutEffect(() => {
    const at = pressAt.current;
    if (!at) return;
    pressAt.current = null;
    const committed = performance.now() - at.t;
    requestAnimationFrame(() =>
      requestAnimationFrame(() => {
        const drawn = performance.now() - at.t;
        setTiming((old) => [{ ms: drawn, frames: committed, what: at.what }, ...old].slice(0, 20));
      }),
    );
  });

  useEffect(() => {
    const key = (e: KeyboardEvent) => {
      if ((e.target as HTMLElement)?.closest?.("input,textarea,[contenteditable]")) return;
      const i = SCENES.indexOf(scene);
      if (e.key === "]") go(SCENES[(i + 1) % SCENES.length]!);
      if (e.key === "[") go(SCENES[(i - 1 + SCENES.length) % SCENES.length]!);
      const vs: Variant[] = ["A", "B", "C"];
      if (e.key === "ArrowRight" && e.altKey) go(scene, vs[(vs.indexOf(v) + 1) % 3]!);
      if (e.key === "ArrowLeft" && e.altKey) go(scene, vs[(vs.indexOf(v) + 2) % 3]!);
    };
    window.addEventListener("keydown", key);
    return () => window.removeEventListener("keydown", key);
  }, [scene, v, go]);

  const states = useMemo(() => fixedChats(statesOf(w)), [w]);
  const median = timing.length ? timing.map((x) => x.ms).sort((a, b) => a - b)[Math.floor(timing.length / 2)]! : undefined;

  return (
    <ChatsHere.Provider value={states}>
      <div
        className="ot-root chats-section"
        data-bare={first.bare || undefined}
        onMouseDown={() => {
          if (w.card?.open) act((d) => void (d.card!.open = false));
        }}
      >
        <TitleBar w={w} act={act} />
        <div className="ot-body">
          <Sidebar w={w} act={act} scope={scope} onScope={setScope} onPick={onPick} />
          <main className="ot-main">
            <Strip w={w} v={v} act={act} />
            <Panes w={w} v={v} act={act} />
          </main>
        </div>
        <footer className="ot-status ui-text-secondary">
          {w.chats.length} chats running · {WS_NAME[w.front]} ·{" "}
          {Object.values(w.places).reduce((n, p) => n + p.tabs.length, 0)} tabs in all
        </footer>
        <HandoffCard w={w} act={act} />
        {!first.bare && (
          <div className="ot-bar" onMouseDown={(e) => e.stopPropagation()}>
            <div className="ot-bar-row">
              <button aria-label="Previous scene" onClick={() => go(SCENES[(SCENES.indexOf(scene) - 1 + SCENES.length) % SCENES.length]!)}>
                <ChevronLeft />
              </button>
              <select value={scene.key} onChange={(e) => go(SCENES.find((x) => x.key === e.target.value)!)} aria-label="Scene">
                {SCENES.map((s, i) => (
                  <option key={s.key} value={s.key}>
                    {i + 1}. {s.title}
                  </option>
                ))}
              </select>
              <button aria-label="Next scene" onClick={() => go(SCENES[(SCENES.indexOf(scene) + 1) % SCENES.length]!)}>
                <ChevronRight />
              </button>
              <span className="ot-bar-sep" />
              <span>Strip:</span>
              {(["A", "B", "C"] as const).map((x) => (
                <button key={x} aria-pressed={v === x} onClick={() => go(scene, x)}>
                  {x} · {VARIANTS[x]}
                </button>
              ))}
              <span className="ot-bar-sep" />
              {scene.event && <button onClick={() => act(scene.event!.run)}>▶ {scene.event.label}</button>}
              <button onClick={() => go(scene)}>Reset</button>
              <button onClick={() => onLook({ ...look, appearance: look.appearance === "dark" ? "light" : "dark" })}>
                {look.appearance === "dark" ? "Light" : "Dark"}
              </button>
            </div>
            <div className="ot-bar-says">{scene.says}</div>
            <div className="ot-bar-says ot-bar-time">
              Sidebar pick, press → drawn frame (prototype React only, no terminal):{" "}
              {median === undefined ? "press a row" : `median ${median.toFixed(1)} ms over ${timing.length}, last ${timing[0]!.ms.toFixed(1)} ms`}
            </div>
          </div>
        )}
      </div>
    </ChatsHere.Provider>
  );
}
