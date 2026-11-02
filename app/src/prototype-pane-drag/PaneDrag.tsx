// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * PROTOTYPE, throwaway (#4583, map #4575). Dragging, splitting and tiling panes, built on the
 * accepted look of #4582 (prototype/one-tab-strip, strip A). Each tab is a binary tree of panes with
 * shares in tenths of a percent (#4587). The persona-and-state icon is the real `ChatPersonaState`;
 * buttons and badges are the real `app/src/ui`. The terminal and the chips are stand-ins.
 *
 * Three variants, switchable from the dashed bar (`?v=A|B|C`):
 *   A · Zones: the pointer's place in a pane picks an edge or the centre, and the result is drawn
 *       before the drop. Keys and single clicks use placement mode: every pane shows five targets.
 *   B · Targets: a pointer drag shows the same five targets, and only they take a drop.
 *   C · Zones and a menu: pointer as A, but keys and single clicks pick from a list of places.
 *
 * URL: `?s=<scene>&v=A|B|C&x=allow|refuse`, `&bare` no bar.
 */
import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState, type CSSProperties, type ReactNode } from "react";
import {
  ArrowDown,
  ArrowLeft,
  ArrowLeftRight,
  ArrowRight,
  ArrowUp,
  Bell,
  ChevronLeft,
  ChevronRight,
  Columns2,
  GripVertical,
  Hand,
  House,
  LayoutGrid,
  Maximize2,
  MessageSquarePlus,
  Minimize2,
  MoreHorizontal,
  Move,
  Pin,
  Plus,
  Settings,
  SquareArrowOutUpRight,
  SquareTerminal,
  X,
} from "lucide-react";
import { ChatPersonaState } from "../ChatRows";
import { ChatsHere, fixedChats, nothingKnown, type ChatStates, type State } from "../chatState";
import { Badge, Count } from "../ui/badge/Badge";
import { Button, IconButton } from "../ui/button/Button";
import { Segment, SegmentedControl } from "../ui/choice/Radio";
import { EmptyState } from "../ui/empty-state/EmptyState";
import { Tooltip } from "../ui/tooltip/Tooltip";
import "../prototype-one-tab/prototype.css";
import "./pane-drag.css";

// ───────────────────────────── the world ─────────────────────────────

type Ws = "smart-ide" | "volaticloud" | "@root";
const WS_NAME: Record<Ws, string> = { "smart-ide": "smart-ide", volaticloud: "volaticloud", "@root": "purlis" };

type Chat = { id: number; name: string; persona: string | null; ws: Ws; parent?: number; state: State; needs?: boolean; report?: "owed" | null };
type Content = { k: "chat"; chat: number } | { k: "empty" } | { k: "view"; view: string } | { k: "job"; job: string; of: number };
type Leaf = { k: "pane"; id: number; c: Content; used: number };
type Split = { k: "split"; id: number; dir: "row" | "column"; share: number; a: Node; b: Node };
type Node = Leaf | Split;
type Tab = { id: number; root: Node; focus: number; pinned?: boolean; zoom?: number | null };
type Place = { tabs: Tab[]; front: number };
type World = { chats: Chat[]; places: Record<Ws, Place>; front: Ws; clock: number; next: number };

const C = (id: number, name: string, persona: string | null, ws: Ws, state: State, more: Partial<Chat> = {}): Chat => ({ id, name, persona, ws, state, ...more });
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
const leaf = (c: Content, used = 0): Leaf => ({ k: "pane", id: ++ids, c, used });
const split = (dir: "row" | "column", a: Node, b: Node, share = 500): Split => ({ k: "split", id: ++ids, dir, share, a, b });
const chat = (id: number): Content => ({ k: "chat", chat: id });
const tabOf = (root: Node, more: Partial<Tab> = {}): Tab => ({ id: ++ids, root, focus: leaves(root)[0]!.id, ...more });
const one = (c: Content): Place => {
  const t = tabOf(leaf(c, 1));
  return { tabs: [t], front: t.id };
};

function base(): World {
  return { chats: chats(), places: { "smart-ide": one(chat(1)), volaticloud: one(chat(21)), "@root": one(chat(31)) }, front: "smart-ide", clock: 10, next: 200 };
}

// ───────────────────────────── the tree ─────────────────────────────

const leaves = (n: Node): Leaf[] => (n.k === "pane" ? [n] : [...leaves(n.a), ...leaves(n.b)]);
const findLeaf = (n: Node, id: number) => leaves(n).find((l) => l.id === id);
/** The tree without the pane: its sibling takes the split's place. */
function without(n: Node, id: number): Node | null {
  if (n.k === "pane") return n.id === id ? null : n;
  const a = without(n.a, id);
  const b = without(n.b, id);
  if (!a) return b;
  if (!b) return a;
  return { ...n, a, b };
}
function replaceLeaf(n: Node, id: number, by: Node): Node {
  if (n.k === "pane") return n.id === id ? by : n;
  return { ...n, a: replaceLeaf(n.a, id, by), b: replaceLeaf(n.b, id, by) };
}
/** An even grid: columns of the square root, rows filled in reading order, every share even. */
function tiled(all: Leaf[]): Node {
  const cols = Math.ceil(Math.sqrt(all.length));
  const rows: Leaf[][] = [];
  for (let i = 0; i < all.length; i += cols) rows.push(all.slice(i, i + cols));
  const even = (items: Node[], dir: "row" | "column"): Node =>
    items.length === 1 ? items[0]! : split(dir, items[0]!, even(items.slice(1), dir), Math.round(1000 / items.length));
  return even(rows.map((r) => even(r, "row")), "column");
}
const gridSaid = (n: number) => {
  const cols = Math.ceil(Math.sqrt(n));
  const rows = Math.ceil(n / cols);
  return rows === 1 ? `${n} side by side` : `${cols} × ${rows}`;
};

// ───────────────────────────── the scenes ─────────────────────────────

type Scene = { key: string; title: string; says: string; make: () => World; all?: boolean; held?: (w: World) => Src };

function inTab(w: World, ws: Ws, root: Node, focusName?: number) {
  const t = tabOf(root);
  if (focusName !== undefined) t.focus = leaves(root).find((l) => l.c.k === "chat" && l.c.chat === focusName)!.id;
  w.places[ws] = { tabs: [t], front: t.id };
  return t;
}

const SCENES: Scene[] = [
  {
    key: "split",
    title: "Three panes: drag to split and swap",
    says: "Drag a pane by its name (the grip). Near an edge the pane splits on that side; in the middle the two panes swap. Drag a sidebar row in the same way. Press the grip without dragging, or press Enter on it, for the keyboard route.",
    make: () => {
      const w = base();
      inTab(w, "smart-ide", split("row", leaf(chat(1), 3), split("column", leaf(chat(2), 2), leaf(chat(4), 1))), 1);
      return w;
    },
  },
  {
    key: "tile",
    title: "Tile all: six panes nested unevenly",
    says: "Six panes, as today's splits leave them: each new pane halves the one before. The grid button at the strip's right end (or ⌘⇧G) tiles the tab's panes into an even grid, in reading order. Undo puts the old tree back.",
    make: () => {
      const w = base();
      const deep = split("row", leaf(chat(1), 6), split("row", leaf(chat(2), 5), split("column", leaf(chat(4), 4), split("row", leaf(chat(5), 3), split("column", leaf(chat(6), 2), leaf(chat(7), 1))))), 520);
      inTab(w, "smart-ide", deep, 1);
      return w;
    },
  },
  {
    key: "tabs",
    title: "Panes and tabs: to the strip and back",
    says: "Drag a pane onto the strip: a line shows where its new tab goes. Hold it over a tab for half a second and that tab comes forward, so the pane can land in its layout. Drag a tab down off the strip and it splits in beside a pane, with all its panes.",
    make: () => {
      const w = base();
      const main = tabOf(split("row", leaf(chat(1), 4), leaf(chat(2), 3)));
      const other = tabOf(split("column", leaf(chat(9), 2), leaf(chat(10), 1)), { pinned: true });
      const lone = tabOf(leaf(chat(7), 1));
      const view = tabOf(leaf({ k: "view", view: "Settings" }, 0));
      w.places["smart-ide"] = { tabs: [main, other, lone, view], front: main.id };
      return w;
    },
  },
  {
    key: "cross",
    title: "Another workspace's chat",
    all: true,
    says: "The sidebar shows All. Drag volaticloud's steward 2 into smart-ide's pane. A press would bring volaticloud forward (#4588); a drag is deliberate, so it may land here. Switch the bar's 'Other workspace' to see the refusal instead.",
    make: () => {
      const w = base();
      inTab(w, "smart-ide", split("row", leaf(chat(1), 2), leaf(chat(2), 1)), 1);
      return w;
    },
  },
  {
    key: "keys",
    title: "The keyboard route: placement mode",
    says: "developer 7 is picked up with the keyboard (Enter on its grip, or ⌘⇧M anywhere). Every pane shows five targets and the strip a New tab target. Arrows move inside a pane's targets, Tab goes to the next pane, Enter drops, Escape puts it back.",
    make: () => {
      const w = base();
      inTab(w, "smart-ide", split("row", leaf(chat(1), 3), split("column", leaf(chat(2), 2), leaf(chat(4), 1))), 2);
      return w;
    },
    held: (w) => {
      const t = w.places["smart-ide"].tabs[0]!;
      return { k: "pane", ws: "smart-ide", tab: t.id, pane: leaves(t.root).find((l) => l.c.k === "chat" && l.c.chat === 2)!.id };
    },
  },
  {
    key: "limits",
    title: "What can't be dropped",
    says: "Four narrow panes and a one-job program (vim, for a commit message). A split that would leave a pane under about 40 columns by 8 rows is refused, and says so. The vim split goes when vim exits (#4588), so it has no grip and takes no swap.",
    make: () => {
      const w = base();
      const job = leaf({ k: "job", job: "vim · COMMIT_EDITMSG", of: 1 }, 1);
      inTab(w, "smart-ide", split("row", split("row", leaf(chat(1), 5), job, 600), split("row", leaf(chat(2), 3), split("row", leaf(chat(4), 2), leaf(chat(5), 1)))), 1);
      return w;
    },
  },
];

// ───────────────────────────── sources, targets, the drop ─────────────────────────────

type Src = { k: "pane"; ws: Ws; tab: number; pane: number } | { k: "chat"; chat: number } | { k: "tab"; ws: Ws; tab: number };
type Side = "left" | "right" | "top" | "bottom" | "centre";
type Target = { k: "pane"; ws: Ws; tab: number; pane: number; side: Side } | { k: "strip"; ws: Ws; index: number };

const byId = (w: World, id: number) => w.chats.find((c) => c.id === id)!;
const frontTab = (p: Place) => p.tabs.find((t) => t.id === p.front) ?? p.tabs[0]!;
const focused = (t: Tab) => findLeaf(t.root, t.focus) ?? leaves(t.root)[0]!;
const tabById = (w: World, ws: Ws, id: number) => w.places[ws].tabs.find((t) => t.id === id);
function shownIn(w: World, id: number) {
  for (const ws of Object.keys(w.places) as Ws[])
    for (const t of w.places[ws].tabs) {
      const l = leaves(t.root).find((x) => x.c.k === "chat" && x.c.chat === id);
      if (l) return { ws, t, l };
    }
  return undefined;
}
function contentName(w: World, c: Content) {
  return c.k === "chat" ? byId(w, c.chat).name : c.k === "view" ? c.view : c.k === "job" ? c.job.split(" · ")[0]! : "empty pane";
}
function srcName(w: World, s: Src): string {
  if (s.k === "chat") return byId(w, s.chat).name;
  const t = tabById(w, s.ws, s.tab);
  if (!t) return "?";
  if (s.k === "tab") {
    const n = leaves(t.root).length;
    return n > 1 ? `the tab ${contentName(w, focused(t).c)} (${n} panes)` : contentName(w, focused(t).c);
  }
  const l = findLeaf(t.root, s.pane);
  return l ? contentName(w, l.c) : "?";
}
/** A chat source already on screen acts as its pane (D1: never shown twice). */
function normal(w: World, s: Src): Src {
  if (s.k !== "chat") return s;
  const on = shownIn(w, s.chat);
  return on ? { k: "pane", ws: on.ws, tab: on.t.id, pane: on.l.id } : s;
}

export const MIN_W = 300;
export const MIN_H = 140;
const SIDE_WORD: Record<Exclude<Side, "centre">, string> = { left: "left of", right: "right of", top: "above", bottom: "below" };

type Verdict = { ok: true; says: string } | { ok: false; why: string };
type Rules = { otherWs: "allow" | "refuse" };

/** What a drop would do, in words, or why it can't. `size` is the target pane's size on screen. */
function judge(w: World, s0: Src, tg: Target, rules: Rules, size?: { w: number; h: number }): Verdict {
  const s = normal(w, s0);
  const name = srcName(w, s);
  const home = s0.k === "chat" ? byId(w, s0.chat).ws : undefined;
  if (home && home !== tg.ws && rules.otherWs === "refuse")
    return { ok: false, why: `${name} works in ${WS_NAME[home]}. Press its row to go there.` };
  if (tg.k === "strip") {
    if (s.k === "tab") return { ok: true, says: `Move the tab here` };
    return { ok: true, says: `A new tab with ${name}` };
  }
  const t = tabById(w, tg.ws, tg.tab)!;
  const target = findLeaf(t.root, tg.pane)!;
  const there = contentName(w, target.c);
  if (s.k === "tab" && s.tab === tg.tab) return { ok: false, why: "That's the tab being moved" };
  if (s.k === "pane" && s.pane === tg.pane) return { ok: false, why: "That's the pane being moved" };
  if (tg.side === "centre") {
    if (target.c.k === "job") return { ok: false, why: `${there} goes when it exits. Drop beside it.` };
    if (s.k === "tab") return leaves(tabById(w, s.ws, s.tab)!.root).length > 1 ? { ok: false, why: "A tab of several panes goes beside a pane, not into one" } : { ok: true, says: `Put ${name} here` };
    if (target.c.k === "empty") return { ok: true, says: `Put ${name} in this empty pane` };
    if (s.k === "pane") return { ok: true, says: `Swap ${name} and ${there}` };
    return { ok: true, says: `Show ${name} here. ${there} keeps running in the sidebar.${home && home !== tg.ws ? ` ${name} stays filed in ${WS_NAME[home]}.` : ""}` };
  }
  if (size) {
    const across = tg.side === "left" || tg.side === "right";
    if (across ? size.w / 2 < MIN_W : size.h / 2 < MIN_H)
      return { ok: false, why: `Too ${across ? "narrow" : "short"}: each half would be under ${across ? "40 columns" : "8 rows"}` };
  }
  const cross = home && home !== tg.ws ? ` It stays filed in ${WS_NAME[home]}.` : "";
  return { ok: true, says: `${name} ${SIDE_WORD[tg.side]} ${there}.${cross}` };
}

/** Takes the source out of where it is: a pane leaves its tree, a tab its strip. */
function lift(w: World, s: Src): Node | null {
  if (s.k === "chat") return leaf(chat(s.chat), ++w.clock);
  const place = w.places[s.ws];
  const t = tabById(w, s.ws, s.tab)!;
  if (s.k === "tab") {
    place.tabs = place.tabs.filter((x) => x !== t);
    if (place.tabs.length === 0) place.tabs.push(tabOf(leaf({ k: "empty" })));
    if (place.front === t.id) place.front = place.tabs[0]!.id;
    return t.root;
  }
  const l = findLeaf(t.root, s.pane)!;
  const rest = without(t.root, s.pane);
  if (rest) {
    t.root = rest;
    if (t.focus === l.id) t.focus = leaves(rest)[0]!.id;
    if (t.zoom === l.id) t.zoom = null;
  } else if (place.tabs.length > 1) {
    place.tabs = place.tabs.filter((x) => x !== t);
    if (place.front === t.id) place.front = place.tabs[0]!.id;
  } else {
    const e = leaf({ k: "empty" });
    t.root = e;
    t.focus = e.id;
  }
  return l;
}

function drop(w: World, s0: Src, tg: Target) {
  const s = normal(w, s0);
  w.clock++;
  if (tg.k === "strip") {
    const place = w.places[tg.ws];
    const before = place.tabs[tg.index];
    const moved = lift(w, s)!;
    const t = s.k === "tab" ? { ...tabOf(moved), id: s.tab } : tabOf(moved);
    const at = before ? place.tabs.indexOf(before) : -1;
    place.tabs.splice(at < 0 ? place.tabs.length : at, 0, t);
    place.front = t.id;
    w.front = tg.ws;
    return;
  }
  const t = tabById(w, tg.ws, tg.tab)!;
  const target = findLeaf(t.root, tg.pane)!;
  if (tg.side === "centre") {
    if (s.k === "pane" && target.c.k !== "empty") {
      const from = findLeaf(tabById(w, s.ws, s.tab)!.root, s.pane)!;
      [from.c, target.c] = [target.c, from.c];
    } else {
      const moved = lift(w, s)!;
      target.c = leaves(moved)[0]!.c;
    }
    target.used = w.clock;
    t.focus = target.id;
  } else {
    const moved = lift(w, s)!;
    const live = findLeaf(t.root, tg.pane)!;
    const dir = tg.side === "left" || tg.side === "right" ? "row" : "column";
    const first = tg.side === "left" || tg.side === "top";
    t.root = replaceLeaf(t.root, live.id, first ? split(dir, moved, live) : split(dir, live, moved));
    t.focus = leaves(moved)[0]!.id;
  }
  w.places[tg.ws].front = t.id;
  w.front = tg.ws;
}

function statesOf(w: World): ChatStates {
  const bySession: Record<number, State> = {};
  for (const c of w.chats) bySession[c.id] = c.state;
  return { ...nothingKnown, bySession, needsYou: w.chats.filter((c) => c.needs).map((c) => c.id) };
}

// ───────────────────────────── drawing ─────────────────────────────

export type Look = { appearance: "light" | "dark"; theme: string };
type Variant = "A" | "B" | "C";
const VARIANTS: Record<Variant, string> = { A: "Zones + placement mode", B: "Targets only", C: "Zones + a list of places" };

type Act = (run: (w: World) => void, undoable?: string) => void;
type Drag = { src: Src; x: number; y: number; over: Target | null; verdict: Verdict | null; pointer: boolean };
type Ui = {
  v: Variant;
  rules: Rules;
  drag: Drag | null;
  held: Src | null;
  menu: Src | null;
  startDrag: (src: Src, e: React.PointerEvent) => void;
  pickUp: (src: Src) => void;
  place: (tg: Target) => void;
  cancel: () => void;
};

function Icon({ w, id }: { w: World; id: number }) {
  const c = byId(w, id);
  return <ChatPersonaState persona={c.persona} session={c.id} shell={c.persona === null} report={c.report ?? null} harness="Claude Code" />;
}
function ContentIcon({ w, c }: { w: World; c: Content }) {
  if (c.k === "chat") return <Icon w={w} id={c.chat} />;
  if (c.k === "view") return <Settings className="ot-glyph" aria-hidden="true" />;
  if (c.k === "job") return <SquareTerminal className="ot-glyph" aria-hidden="true" />;
  return <Plus className="ot-glyph" aria-hidden="true" />;
}

function Strip({ w, act, ui }: { w: World; act: Act; ui: Ui }) {
  const place = w.places[w.front];
  const t = frontTab(place);
  const n = leaves(t.root).length;
  const over = ui.drag?.over?.k === "strip" ? ui.drag.over.index : null;
  return (
    <div className="ot-strip pd-strip" role="tablist" aria-label="Tabs" data-drop-strip={w.front}>
      {place.tabs.map((tb, i) => {
        const f = focused(tb);
        const all = leaves(tb.root);
        const sel = place.front === tb.id;
        return (
          <div key={tb.id} className="ot-tab pd-tab" role="tab" aria-selected={sel} data-pinned={tb.pinned || undefined} data-drop-tab={tb.id} data-index={i} data-gap-before={over === i || undefined}>
            <Tooltip tip={all.map((l) => contentName(w, l.c)).join(" · ")}>
              <button
                className="ot-tab-press"
                data-src-tab={tb.id}
                onClick={() => act((d) => void (d.places[d.front].front = tb.id))}
                onPointerDown={(e) => ui.startDrag({ k: "tab", ws: w.front, tab: tb.id }, e)}
                onKeyDown={(e) => {
                  if (e.key === "m" && (e.metaKey || e.ctrlKey) && e.shiftKey) {
                    e.preventDefault();
                    ui.pickUp({ k: "tab", ws: w.front, tab: tb.id });
                  }
                }}
              >
                {tb.pinned && <Pin className="ot-pin" aria-hidden="true" />}
                <ContentIcon w={w} c={f.c} />
                <span className="ot-tab-name">{contentName(w, f.c)}</span>
                {all.length > 1 && (
                  <span className="ot-split-more">
                    <Columns2 aria-hidden="true" />+{all.length - 1}
                  </span>
                )}
                {tb.zoom ? <Maximize2 className="pd-zoomed" aria-label="one pane zoomed" /> : null}
              </button>
            </Tooltip>
            <button className="ot-tab-x" aria-label="Close tab" onClick={() => act((d) => {
              const p = d.places[d.front];
              if (p.tabs.length === 1) { const e = leaf({ k: "empty" }); p.tabs[0]!.root = e; p.tabs[0]!.focus = e.id; return; }
              p.tabs = p.tabs.filter((x) => x.id !== tb.id);
              if (p.front === tb.id) p.front = p.tabs[0]!.id;
            }, "Closed a tab")}>
              <X aria-hidden="true" />
            </button>
          </div>
        );
      })}
      <div className="pd-strip-end" data-gap-before={over === place.tabs.length || undefined} data-index={place.tabs.length} />
      {(ui.held || (ui.drag && !ui.drag.pointer)) && (
        <PlaceButton ui={ui} w={w} tg={{ k: "strip", ws: w.front, index: place.tabs.length }} className="pd-strip-target">
          <SquareArrowOutUpRight aria-hidden="true" /> New tab
        </PlaceButton>
      )}
      <Tooltip tip="New tab">
        <button className="ot-strip-plus" aria-label="New tab" onClick={() => act((d) => { const nt = tabOf(leaf({ k: "empty" })); d.places[d.front].tabs.push(nt); d.places[d.front].front = nt.id; })}>
          <Plus aria-hidden="true" />
        </button>
      </Tooltip>
      <span className="ot-grow" />
      <Tooltip tip={n < 2 ? "Tile all: one pane, nothing to tile" : `Tile all: ${n} panes, ${gridSaid(n)} (⌘⇧G)`}>
        <button className="ot-strip-plus pd-tile" aria-label={`Tile all ${n} panes`} disabled={n < 2} onClick={() => act((d) => tileFront(d), `Tiled ${n} panes, ${gridSaid(n)}`)}>
          <LayoutGrid aria-hidden="true" />
        </button>
      </Tooltip>
    </div>
  );
}

function tileFront(d: World) {
  const t = frontTab(d.places[d.front]);
  t.root = tiled(leaves(t.root));
  t.zoom = null;
}

/** One place to drop, as a real button: placement mode and the list both use it. */
function PlaceButton({ ui, w, tg, children, className, size }: { ui: Ui; w: World; tg: Target; children: ReactNode; className?: string; size?: { w: number; h: number } }) {
  const src = ui.held ?? ui.drag?.src ?? ui.menu;
  if (!src) return null;
  const v = judge(w, src, tg, ui.rules, size);
  const label = v.ok ? v.says : `Can't: ${v.why}`;
  return (
    <Tooltip tip={label}>
      <button
        className={`pd-target ${className ?? ""}`}
        aria-label={label}
        aria-disabled={!v.ok || undefined}
        data-target-side={tg.k === "pane" ? tg.side : "strip"}
        data-hot={(ui.drag?.over && sameTarget(ui.drag.over, tg)) || undefined}
        onClick={() => v.ok && ui.place(tg)}
      >
        {children}
      </button>
    </Tooltip>
  );
}
const sameTarget = (a: Target, b: Target) =>
  a.k === "pane" && b.k === "pane" ? a.pane === b.pane && a.side === b.side : a.k === "strip" && b.k === "strip" && a.index === b.index;

const ARROW: Record<Side, ReactNode> = {
  top: <ArrowUp aria-hidden="true" />,
  left: <ArrowLeft aria-hidden="true" />,
  centre: <ArrowLeftRight aria-hidden="true" />,
  right: <ArrowRight aria-hidden="true" />,
  bottom: <ArrowDown aria-hidden="true" />,
};

/** Five targets in a cross, in the middle of a pane. */
function Compass({ w, ui, ws, t, l, size }: { w: World; ui: Ui; ws: Ws; t: Tab; l: Leaf; size: { w: number; h: number } }) {
  const sides: Side[] = ["top", "left", "centre", "right", "bottom"];
  return (
    <div
      className="pd-compass"
      role="group"
      aria-label={`Places in ${contentName(w, l.c)}`}
      onKeyDown={(e) => {
        const map: Record<string, Side> = { ArrowUp: "top", ArrowLeft: "left", ArrowRight: "right", ArrowDown: "bottom" };
        const side = map[e.key];
        if (!side) return;
        e.preventDefault();
        const here = (e.target as HTMLElement).dataset.targetSide;
        const go = here === side ? "centre" : side;
        (e.currentTarget.querySelector(`[data-target-side="${go}"]`) as HTMLElement | null)?.focus();
      }}
    >
      {sides.map((side) => (
        <PlaceButton key={side} ui={ui} w={w} tg={{ k: "pane", ws, tab: t.id, pane: l.id, side }} className={`pd-at-${side}`} size={size}>
          {ARROW[side]}
        </PlaceButton>
      ))}
    </div>
  );
}

function useSize(ref: React.RefObject<HTMLElement | null>) {
  const [size, setSize] = useState({ w: 800, h: 600 });
  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return;
    const ro = new ResizeObserver(() => setSize({ w: el.clientWidth, h: el.clientHeight }));
    ro.observe(el);
    setSize({ w: el.clientWidth, h: el.clientHeight });
    return () => ro.disconnect();
  }, [ref]);
  return size;
}

function PaneView({ w, ws, t, l, act, ui }: { w: World; ws: Ws; t: Tab; l: Leaf; act: Act; ui: Ui }) {
  const ref = useRef<HTMLElement>(null);
  const size = useSize(ref);
  const all = leaves(t.root);
  const alone = all.length === 1 && w.places[ws].tabs.length === 1;
  const isFocus = t.focus === l.id && all.length > 1;
  const c = l.c;
  const me: Src = { k: "pane", ws, tab: t.id, pane: l.id };
  const lifted = (ui.drag && ui.drag.src.k === "pane" && ui.drag.src.pane === l.id) || (ui.held?.k === "pane" && ui.held.pane === l.id);
  const over = ui.drag?.over?.k === "pane" && ui.drag.over.pane === l.id ? ui.drag.over : null;
  const placing = ui.held !== null || (ui.v === "B" && ui.drag?.pointer && ui.drag.over?.k === "pane" && ui.drag.over.pane === l.id) || (ui.v === "B" && ui.drag?.pointer && hoverPane.current === l.id);
  const crossWs = c.k === "chat" && byId(w, c.chat).ws !== ws;
  const find = (d: World) => {
    const tt = tabById(d, ws, t.id)!;
    return { tt, ll: findLeaf(tt.root, l.id)! };
  };
  const grip = c.k !== "job" && c.k !== "empty";
  return (
    <section
      ref={ref}
      className="ot-pane pd-pane"
      data-focus={isFocus || undefined}
      data-lifted={lifted || undefined}
      data-drop-pane={l.id}
      data-ws={ws}
      data-tab={t.id}
      onMouseDown={all.length > 1 ? () => act((d) => { const { tt, ll } = find(d); tt.focus = ll.id; ll.used = ++d.clock; }) : undefined}
    >
      {c.k !== "empty" && (
        <header className="ot-pane-head">
          {grip ? (
            <button
              className="pd-grip"
              data-src-pane={l.id}
              aria-label={`Move ${contentName(w, c)}: press, or drag`}
              aria-describedby="pd-how"
              onPointerDown={(e) => ui.startDrag(me, e)}
              onKeyDown={(e) => {
                if (e.key === "Enter" || e.key === " ") {
                  e.preventDefault();
                  ui.pickUp(me);
                }
              }}
            >
              <GripVertical className="pd-grip-dots" aria-hidden="true" />
              <ContentIcon w={w} c={c} />
              <span className="ot-pane-name">{contentName(w, c)}</span>
            </button>
          ) : (
            <span className="pd-nogrip">
              <ContentIcon w={w} c={c} />
              <span className="ot-pane-name">{contentName(w, c)}</span>
            </span>
          )}
          {c.k === "chat" && <span className="ot-pane-where">Claude Code</span>}
          {crossWs && <Badge variant="accent">{WS_NAME[byId(w, (c as { chat: number }).chat).ws]}</Badge>}
          {c.k === "job" && <span className="ot-pane-where">goes when it exits</span>}
          <span className="ot-grow" />
          {c.k === "chat" && <IconButton aria-label="Notices" tooltip="Notices (#4584's slot)" size="sm" variant="ghost" icon={<Bell aria-hidden="true" />} />}
          {c.k === "chat" && <IconButton aria-label="Chat actions" tooltip="Chat actions" size="sm" variant="ghost" icon={<MoreHorizontal aria-hidden="true" />} />}
          {all.length > 1 && (
            <IconButton
              aria-label={t.zoom === l.id ? "Unzoom" : "Zoom"}
              tooltip={t.zoom === l.id ? "Back to the split (⌘⇧Enter)" : "Zoom: this pane fills the tab for now (⌘⇧Enter)"}
              size="sm"
              variant="ghost"
              icon={t.zoom === l.id ? <Minimize2 aria-hidden="true" /> : <Maximize2 aria-hidden="true" />}
              onPress={() => act((d) => { const { tt } = find(d); tt.zoom = tt.zoom === l.id ? null : l.id; })}
            />
          )}
          {c.k !== "job" && (
            <IconButton aria-label="Split" tooltip="Split: an empty pane beside" size="sm" variant="ghost" icon={<Columns2 aria-hidden="true" />}
              onPress={() => act((d) => { const { tt, ll } = find(d); const n = leaf({ k: "empty" }, ++d.clock); tt.root = replaceLeaf(tt.root, ll.id, split(size.w >= size.h * 1.2 ? "row" : "column", ll, n)); tt.focus = n.id; }, "Split a pane")} />
          )}
          {c.k !== "job" && (
            <IconButton aria-label="Close pane" tooltip={c.k === "chat" ? "Close pane: the chat keeps running in the sidebar" : "Close"} size="sm" variant="ghost" icon={<X aria-hidden="true" />}
              onPress={() => act((d) => void lift(d, me), "Closed a pane")} />
          )}
        </header>
      )}
      <div className="ot-pane-body">
        {c.k === "chat" ? (
          <Terminal w={w} id={c.chat} />
        ) : c.k === "view" ? (
          <div className="ot-view"><h2>Settings</h2><p className="ui-text-secondary">A view is a pane like any other: it can be dragged and split.</p></div>
        ) : c.k === "job" ? (
          <pre className="ot-term">{"# Please enter the commit message for your changes.\n# On branch prototype/pane-drag\n~\n~\n~\n-- INSERT --"}</pre>
        ) : (
          <div className="ot-empty">
            {!alone && <div className="ot-empty-x"><IconButton aria-label="Close pane" tooltip="Close pane" size="sm" variant="ghost" icon={<X aria-hidden="true" />} onPress={() => act((d) => void lift(d, me))} /></div>}
            <EmptyState mark={MessageSquarePlus} headline={all.length > 1 ? "An empty pane" : "Nothing in this pane"} body="Pick a chat in the sidebar, or drag one here." action={<Button variant="primary" onPress={() => undefined}>New chat</Button>} />
          </div>
        )}
        {over && ui.drag?.verdict && <Preview side={over.side} verdict={ui.drag.verdict} />}
        {placing && !lifted && ui.v !== "C" && (ui.held || ui.drag) && <Compass w={w} ui={ui} ws={ws} t={t} l={l} size={size} />}
      </div>
    </section>
  );
}
/** Which pane the pointer is over while dragging, for variant B's targets. */
const hoverPane: { current: number | null } = { current: null };

function Preview({ side, verdict }: { side: Side; verdict: Verdict }) {
  return (
    <div className="pd-preview" data-side={side} data-refused={!verdict.ok || undefined}>
      <span className="pd-preview-says">{verdict.ok ? verdict.says : verdict.why}</span>
    </div>
  );
}

const LINES: Record<string, string[]> = {
  running: ["● Reading app/src/LayoutPanes.tsx", "● Searching for placeBeside callers", "  4 results", "✻ Working…"],
  waiting: ["● Done. The tests pass.", "", "❯ "],
  done: ["● Reported to steward 4.", "", "❯ "],
  failed: ["● cargo test failed: 2 tests", "", "❯ "],
};
function Terminal({ w, id }: { w: World; id: number }) {
  const c = byId(w, id);
  return <pre className="ot-term" aria-label={`${c.name}'s terminal (stand-in)`}>{`${c.name} · ${c.persona ?? "shell"}\n\n` + (c.needs ? ["● I need your call before I go on.", "", "  ✋ Waiting for you"] : LINES[c.state] ?? LINES.waiting!).join("\n")}</pre>;
}

function Tree({ w, ws, t, n, act, ui }: { w: World; ws: Ws; t: Tab; n: Node; act: Act; ui: Ui }) {
  if (n.k === "pane") return <PaneView key={n.id} w={w} ws={ws} t={t} l={n} act={act} ui={ui} />;
  const style: CSSProperties = { flexBasis: `calc(${n.share / 10}% - 2px)` };
  return (
    <div className="pd-split" data-dir={n.dir}>
      <div className="pd-cell" style={style}><Tree w={w} ws={ws} t={t} n={n.a} act={act} ui={ui} /></div>
      <SplitBar w={w} ws={ws} t={t} n={n} act={act} />
      <div className="pd-cell pd-rest"><Tree w={w} ws={ws} t={t} n={n.b} act={act} ui={ui} /></div>
    </div>
  );
}

/** The separator: arrows resize by 5%, a double press evens the split (a missing manipulation). */
function SplitBar({ w, ws, t, n, act }: { w: World; ws: Ws; t: Tab; n: Split; act: Act }) {
  const set = (share: number) => act((d) => {
    const walk = (x: Node): void => { if (x.k === "split") { if (x.id === n.id) x.share = Math.max(100, Math.min(900, share)); walk(x.a); walk(x.b); } };
    walk(tabById(d, ws, t.id)!.root);
  });
  const names = (x: Node) => leaves(x).map((l) => contentName(w, l.c)).join(", ");
  return (
    <div
      className="pd-bar"
      role="separator"
      tabIndex={0}
      aria-orientation={n.dir === "row" ? "vertical" : "horizontal"}
      aria-valuenow={n.share / 10}
      aria-valuemin={10}
      aria-valuemax={90}
      aria-label={`Between ${names(n.a)} and ${names(n.b)}`}
      title="Drag to resize · double-click to even · arrow keys 5%"
      onDoubleClick={() => set(500)}
      onKeyDown={(e) => {
        const grow = n.dir === "row" ? { ArrowLeft: -50, ArrowRight: 50 } : { ArrowUp: -50, ArrowDown: 50 };
        const by = (grow as Record<string, number>)[e.key];
        if (by) { e.preventDefault(); set(n.share + by); }
        if (e.key === "Enter") set(500);
      }}
      onPointerDown={(e) => {
        const box = (e.currentTarget.parentElement as HTMLElement).getBoundingClientRect();
        const move = (m: PointerEvent) => set(Math.round(((n.dir === "row" ? m.clientX - box.left : m.clientY - box.top) / (n.dir === "row" ? box.width : box.height)) * 1000));
        const up = () => { window.removeEventListener("pointermove", move); window.removeEventListener("pointerup", up); };
        window.addEventListener("pointermove", move);
        window.addEventListener("pointerup", up);
      }}
    />
  );
}

function Panes({ w, act, ui }: { w: World; act: Act; ui: Ui }) {
  const t = frontTab(w.places[w.front]);
  const zoomed = t.zoom ? findLeaf(t.root, t.zoom) : undefined;
  return (
    <div className="pd-panes">
      {zoomed ? <PaneView w={w} ws={w.front} t={t} l={zoomed} act={act} ui={ui} /> : <Tree w={w} ws={w.front} t={t} n={t.root} act={act} ui={ui} />}
    </div>
  );
}

function Sidebar({ w, act, scope, onScope, ui }: { w: World; act: Act; scope: string; onScope: (s: string) => void; ui: Ui }) {
  const order: Ws[] = ["smart-ide", "volaticloud", "@root"];
  const list = (scope === "all" ? w.chats : w.chats.filter((c) => c.ws === w.front)).slice().sort((a, b) => order.indexOf(a.ws) - order.indexOf(b.ws));
  const front = focused(frontTab(w.places[w.front])).c;
  const frontId = front.k === "chat" ? front.chat : undefined;
  const onScreen = new Set(leaves(frontTab(w.places[w.front]).root).flatMap((l) => (l.c.k === "chat" ? [l.c.chat] : [])));
  const tree: { c: Chat; level: number }[] = [];
  const walk = (parent: number | undefined, level: number) => {
    for (const c of list.filter((x) => x.parent === parent)) { tree.push({ c, level }); walk(c.id, level + 1); }
  };
  walk(undefined, 0);
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
      <ul role="tree">
        {tree.map(({ c, level }) => {
          const head = scope === "all" && level === 0 && c.ws !== lastWs;
          if (level === 0) lastWs = c.ws;
          const lifted = (ui.drag?.src.k === "chat" && ui.drag.src.chat === c.id) || (ui.held?.k === "chat" && ui.held.chat === c.id);
          return (
            <li key={c.id} role="none" className="ot-li">
              {head && <div className="ot-side-ws">{WS_NAME[c.ws]}</div>}
              <div
                role="treeitem"
                tabIndex={-1}
                aria-current={c.id === frontId ? "true" : undefined}
                className="ot-row chat pd-row"
                data-current={c.id === frontId || undefined}
                data-shown={(onScreen.has(c.id) && c.id !== frontId) || undefined}
                data-lifted={lifted || undefined}
                data-src-chat={c.id}
                style={{ paddingInlineStart: 8 + level * 14 }}
                onPointerDown={(e) => ui.startDrag({ k: "chat", chat: c.id }, e)}
                onClick={() => !ui.drag && act((d) => pickChat(d, c.id))}
              >
                <Icon w={w} id={c.id} />
                <span className="ot-row-name">{c.name}</span>
                <span className="ot-grow" />
                <Tooltip tip="Place… (⌘⇧M): choose a pane, a side or a new tab">
                  <button className="ot-row-tab pd-row-place" aria-label={`Place ${c.name}…`} onPointerDown={(e) => e.stopPropagation()} onClick={(e) => { e.stopPropagation(); ui.pickUp({ k: "chat", chat: c.id }); }}>
                    <Move aria-hidden="true" />
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

/** A sidebar press, as #4582 drew it: the focused pane switches, another workspace comes forward. */
function pickChat(w: World, id: number) {
  const on = shownIn(w, id);
  w.clock++;
  if (on) { w.front = on.ws; w.places[on.ws].front = on.t.id; on.t.focus = on.l.id; return; }
  const ws = byId(w, id).ws;
  w.front = ws;
  const t = frontTab(w.places[ws]);
  const l = focused(t);
  l.c = chat(id);
  l.used = w.clock;
}

function TitleBar({ w, act }: { w: World; act: Act }) {
  const needs = w.chats.filter((c) => c.needs).length;
  return (
    <header className="ot-title">
      <span className="ot-lights" aria-hidden="true"><i /><i /><i /></span>
      <button className="ot-chip ot-house" aria-current={w.front === "@root" ? "page" : undefined} aria-label="purlis plane root" onClick={() => act((d) => void (d.front = "@root"))}><House aria-hidden="true" /></button>
      {(["smart-ide", "volaticloud"] as Ws[]).map((ws) => {
        const count = w.chats.filter((c) => c.ws === ws && c.needs).length;
        return (
          <span key={ws} className="ot-chip-wrap" data-chip={ws}>
            <button className="ot-chip" aria-current={w.front === ws ? "page" : undefined} onClick={() => act((d) => void (d.front = ws))}>
              {WS_NAME[ws]}
              {count > 0 && <Count variant="waiting" said={`${count} need you`}>{count}</Count>}
            </button>
          </span>
        );
      })}
      <span className="ot-grow" />
      <button className="ot-hand" aria-label={`${needs} need you`}><Hand aria-hidden="true" />{needs > 0 && <Count variant="waiting">{needs}</Count>}</button>
    </header>
  );
}

/** Variant C: the places, as a list. The same targets as the compass, in words. */
function PlaceList({ w, ui }: { w: World; ui: Ui }) {
  const src = ui.menu;
  const first = useRef<HTMLDivElement>(null);
  useEffect(() => { (first.current?.querySelector("button:not([aria-disabled])") as HTMLElement | null)?.focus(); }, [src]);
  if (!src) return null;
  const place = w.places[w.front];
  const t = frontTab(place);
  const sides: Side[] = ["left", "right", "top", "bottom", "centre"];
  const word: Record<Side, string> = { left: "Left", right: "Right", top: "Above", bottom: "Below", centre: "Swap / here" };
  return (
    <div className="pd-list-at" onMouseDown={(e) => e.stopPropagation()}>
      <div className="pd-list" role="dialog" aria-label={`Place ${srcName(w, src)}`} ref={first} onKeyDown={(e) => e.key === "Escape" && ui.cancel()}>
        <div className="pd-list-head">Place <strong>{srcName(w, src)}</strong></div>
        {leaves(t.root).map((l) => {
          const el = document.querySelector(`[data-drop-pane="${l.id}"]`);
          const size = el ? { w: el.clientWidth, h: el.clientHeight } : undefined;
          return (
            <div key={l.id} className="pd-list-row">
              <span className="pd-list-name"><ContentIcon w={w} c={l.c} />{contentName(w, l.c)}</span>
              {sides.map((side) => (
                <PlaceButton key={side} ui={ui} w={w} tg={{ k: "pane", ws: w.front, tab: t.id, pane: l.id, side }} className="pd-list-btn" size={size}>
                  {ARROW[side]} {word[side]}
                </PlaceButton>
              ))}
            </div>
          );
        })}
        <div className="pd-list-row">
          <PlaceButton ui={ui} w={w} tg={{ k: "strip", ws: w.front, index: place.tabs.length }} className="pd-list-btn"><SquareArrowOutUpRight aria-hidden="true" /> A new tab</PlaceButton>
          <button className="pd-list-btn" onClick={ui.cancel}>Cancel (Esc)</button>
        </div>
      </div>
    </div>
  );
}

// ───────────────────────────── the page ─────────────────────────────

function readUrl() {
  const q = new URLSearchParams(location.search);
  const s = SCENES.find((x) => x.key === q.get("s")) ?? SCENES[0]!;
  const v = (["A", "B", "C"] as const).find((x) => x === q.get("v")) ?? "A";
  const x = q.get("x") === "refuse" ? "refuse" : "allow";
  return { s, v, x: x as Rules["otherWs"], bare: q.has("bare") };
}

/** Which target the pointer is over. Variant B takes only the five targets. */
function hit(x: number, y: number, v: Variant): { tg: Target | null; size?: { w: number; h: number } } {
  hoverPane.current = null;
  for (const el of document.elementsFromPoint(x, y) as HTMLElement[]) {
    if (el.closest(".pd-ghost")) continue;
    const strip = el.closest("[data-drop-strip]") as HTMLElement | null;
    if (strip) {
      const tabs = [...strip.querySelectorAll("[data-drop-tab]")] as HTMLElement[];
      let index = tabs.length;
      for (const [i, tb] of tabs.entries()) {
        const r = tb.getBoundingClientRect();
        if (x < r.left + r.width / 2) { index = i; break; }
      }
      return { tg: { k: "strip", ws: strip.dataset.dropStrip as Ws, index } };
    }
    const p = el.closest("[data-drop-pane]") as HTMLElement | null;
    if (!p) continue;
    const pane = Number(p.dataset.dropPane);
    const ws = p.dataset.ws as Ws;
    const tab = Number(p.dataset.tab);
    const size = { w: p.clientWidth, h: p.clientHeight };
    hoverPane.current = pane;
    if (v === "B") {
      const side = (el.closest("[data-target-side]") as HTMLElement | null)?.dataset.targetSide as Side | undefined;
      return { tg: side ? { k: "pane", ws, tab, pane, side } : null, size };
    }
    const r = p.getBoundingClientRect();
    const fx = (x - r.left) / r.width;
    const fy = (y - r.top) / r.height;
    const edges: [Side, number][] = [["left", fx], ["right", 1 - fx], ["top", fy], ["bottom", 1 - fy]];
    const [side, d] = edges.sort((a, b) => a[1] - b[1])[0]!;
    return { tg: { k: "pane", ws, tab, pane, side: d < 0.25 ? side : "centre" }, size };
  }
  return { tg: null };
}

export function PaneDrag({ look, onLook }: { look: Look; onLook: (look: Look) => void }) {
  const first = useMemo(readUrl, []);
  const [scene, setScene] = useState<Scene>(first.s);
  const [v, setV] = useState<Variant>(first.v);
  const [rules, setRules] = useState<Rules>({ otherWs: first.x });
  const [w, setW] = useState<World>(() => first.s.make());
  const [scope, setScope] = useState(first.s.all ? "all" : "ws");
  const [drag, setDrag] = useState<Drag | null>(null);
  const [held, setHeld] = useState<Src | null>(null);
  const [menu, setMenu] = useState<Src | null>(null);
  const [said, setSaid] = useState("");
  const [undo, setUndo] = useState<{ label: string; was: World } | null>(null);
  const wRef = useRef(w);
  wRef.current = w;
  const returnTo = useRef<HTMLElement | null>(null);
  const spring = useRef<{ tab: number; at: number } | null>(null);

  const act: Act = useCallback((run, undoable) => {
    setW((prev) => {
      const next = structuredClone(prev);
      run(next);
      if (undoable) setUndo({ label: undoable, was: prev });
      return next;
    });
  }, []);

  const go = useCallback((s: Scene, nv: Variant = v, nx: Rules["otherWs"] = rules.otherWs) => {
    setScene(s); setV(nv); setRules({ otherWs: nx });
    const fresh = s.make();
    setW(fresh); setScope(s.all ? "all" : "ws"); setDrag(null); setMenu(null); setUndo(null);
    setHeld(s.held && nv !== "C" ? s.held(fresh) : null);
    setMenu(s.held && nv === "C" ? s.held(fresh) : null);
    setSaid(s.held ? `Picked up ${srcName(fresh, s.held(fresh))}. Arrow keys choose a side, Tab the next pane, Enter drops, Escape cancels.` : "");
    const q = new URLSearchParams(location.search);
    q.set("s", s.key); q.set("v", nv); q.set("x", nx);
    try { history.replaceState(null, "", `?${q}`); } catch { /* a sandboxed frame refuses it */ }
  }, [v, rules.otherWs]);

  useEffect(() => {
    if (first.s.held) { const h = first.s.held(w); if (first.v === "C") setMenu(h); else setHeld(h); setSaid(`Picked up ${srcName(w, h)}. Arrow keys choose a side, Tab the next pane, Enter drops, Escape cancels.`); }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const pickUp = useCallback((src: Src) => {
    returnTo.current = document.activeElement as HTMLElement;
    const name = srcName(wRef.current, src);
    if (v === "C") { setMenu(src); setSaid(`Place ${name}: a list of places is open. Escape cancels.`); return; }
    setHeld(src);
    setSaid(`Picked up ${name}. Arrow keys choose a side, Tab the next pane, Enter drops, Escape cancels.`);
  }, [v]);

  const cancel = useCallback(() => {
    setHeld(null); setMenu(null); setDrag(null);
    setSaid("Put back. Nothing moved.");
    returnTo.current?.focus?.();
  }, []);

  const place = useCallback((tg: Target) => {
    const src = held ?? menu ?? drag?.src;
    if (!src) return;
    const cur = wRef.current;
    const v1 = judge(cur, src, tg, rules);
    if (!v1.ok) return;
    act((d) => drop(d, src, tg), v1.says);
    setSaid(`Dropped. ${v1.says}`);
    setHeld(null); setMenu(null); setDrag(null);
  }, [held, menu, drag, rules, act]);

  // Placement mode: focus the first target, Escape cancels.
  useEffect(() => {
    if (!held) return;
    requestAnimationFrame(() => {
      // The centre of the first other pane: the held pane draws no targets of its own.
      const f = document.querySelector('.pd-compass [data-target-side="centre"]:not([aria-disabled])') as HTMLElement | null;
      (f ?? (document.querySelector(".pd-target:not([aria-disabled])") as HTMLElement | null))?.focus();
    });
  }, [held]);

  const startDrag = useCallback((src: Src, e: React.PointerEvent) => {
    if (e.button !== 0) return;
    const x0 = e.clientX;
    const y0 = e.clientY;
    let live = false;
    const move = (m: PointerEvent) => {
      if (!live && Math.hypot(m.clientX - x0, m.clientY - y0) < 5) return;
      if (!live) { live = true; setSaid(`Dragging ${srcName(wRef.current, src)}.`); }
      const { tg, size } = hit(m.clientX, m.clientY, v);
      const verdict = tg ? judge(wRef.current, src, tg, rules, size) : null;
      // Hold over a tab for half a second and it comes forward.
      const tabEl = (document.elementFromPoint(m.clientX, m.clientY) as HTMLElement | null)?.closest("[data-drop-tab]") as HTMLElement | null;
      if (tabEl) {
        const id = Number(tabEl.dataset.dropTab);
        if (spring.current?.tab !== id) spring.current = { tab: id, at: performance.now() };
        else if (performance.now() - spring.current.at > 500) setW((p) => { const pl = p.places[p.front]; if (pl.front === id) return p; const n = structuredClone(p); n.places[n.front].front = id; return n; });
      } else spring.current = null;
      setDrag({ src, x: m.clientX, y: m.clientY, over: tg, verdict, pointer: true });
    };
    const up = (u: PointerEvent) => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
      window.removeEventListener("keydown", esc, true);
      spring.current = null;
      if (!live) {
        // A press without a drag: the single-pointer route (WCAG 2.5.7). Rows keep their press.
        if (src.k === "pane") pickUp(src);
        return;
      }
      const { tg, size } = hit(u.clientX, u.clientY, v);
      const verdict = tg ? judge(wRef.current, src, tg, rules, size) : null;
      if (tg && verdict?.ok) {
        setW((p) => { const n = structuredClone(p); drop(n, src, tg); setUndo({ label: verdict.says, was: p }); return n; });
        setSaid(`Dropped. ${verdict.says}`);
      } else setSaid(verdict && !verdict.ok ? `Not dropped: ${verdict.why}` : "Not dropped.");
      setTimeout(() => setDrag(null), 0);
    };
    const esc = (k: KeyboardEvent) => {
      if (k.key !== "Escape") return;
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
      window.removeEventListener("keydown", esc, true);
      setDrag(null);
      setSaid("Drag cancelled. Nothing moved.");
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
    window.addEventListener("keydown", esc, true);
  }, [v, rules, pickUp]);

  useEffect(() => {
    const key = (e: KeyboardEvent) => {
      if ((e.target as HTMLElement)?.closest?.("input,textarea,select,[contenteditable]")) return;
      if (e.key === "Escape" && (held || menu)) { e.preventDefault(); cancel(); return; }
      const mod = e.metaKey || e.ctrlKey;
      if (mod && e.shiftKey && (e.key === "m" || e.key === "M")) {
        e.preventDefault();
        const t = frontTab(w.places[w.front]);
        const f = focused(t);
        if (f.c.k !== "empty" && f.c.k !== "job") pickUp({ k: "pane", ws: w.front, tab: t.id, pane: f.id });
        return;
      }
      if (mod && e.shiftKey && (e.key === "g" || e.key === "G")) { e.preventDefault(); const n = leaves(frontTab(w.places[w.front]).root).length; act(tileFront, `Tiled ${n} panes, ${gridSaid(n)}`); return; }
      if (mod && e.shiftKey && e.key === "Enter") { e.preventDefault(); act((d) => { const t = frontTab(d.places[d.front]); t.zoom = t.zoom ? null : t.focus; }); return; }
      if (mod && !e.shiftKey && (e.key === "z" || e.key === "Z") && undo) { e.preventDefault(); setW(undo.was); setUndo(null); setSaid(`Undone: ${undo.label}`); return; }
      if (held || menu) return;
      const i = SCENES.indexOf(scene);
      if (e.key === "]") go(SCENES[(i + 1) % SCENES.length]!);
      if (e.key === "[") go(SCENES[(i - 1 + SCENES.length) % SCENES.length]!);
    };
    window.addEventListener("keydown", key);
    return () => window.removeEventListener("keydown", key);
  }, [scene, go, held, menu, cancel, pickUp, w, act, undo]);

  const ui: Ui = { v, rules, drag, held, menu, startDrag, pickUp, place, cancel };
  const states = useMemo(() => fixedChats(statesOf(w)), [w]);
  const t = frontTab(w.places[w.front]);
  const shape = (n: Node): string => (n.k === "pane" ? contentName(w, n.c) : `${n.dir === "row" ? "row" : "col"} ${n.share}(${shape(n.a)} | ${shape(n.b)})`);

  return (
    <ChatsHere.Provider value={states}>
      <div className="ot-root chats-section pd-root" data-bare={first.bare || undefined} data-dragging={drag?.pointer || undefined} data-variant={v}>
        <TitleBar w={w} act={act} />
        <div className="ot-body">
          <Sidebar w={w} act={act} scope={scope} onScope={setScope} ui={ui} />
          <main className="ot-main">
            <Strip w={w} act={act} ui={ui} />
            <Panes w={w} act={act} ui={ui} />
          </main>
        </div>
        <footer className="ot-status ui-text-secondary">
          {held ? <span className="pd-status-held"><Move aria-hidden="true" /> Placing {srcName(w, held)}: choose a target · Esc puts it back</span> : `${leaves(t.root).length} panes in this tab · ${w.places[w.front].tabs.length} tabs · ${WS_NAME[w.front]}`}
        </footer>
        <div id="pd-how" hidden>Press Enter or Space to pick up, or drag. Then arrow keys choose a side, Tab the next pane, Enter drops, Escape cancels.</div>
        <div className="pd-live" aria-live="polite" role="status">{said}</div>
        {undo && !drag && !held && (
          <div className="pd-toast" role="status">
            <span>{undo.label}</span>
            <Button size="sm" variant="secondary" onPress={() => { setW(undo.was); setUndo(null); setSaid(`Undone: ${undo.label}`); }}>Undo (⌘Z)</Button>
            <IconButton aria-label="Dismiss" tooltip="Dismiss" size="sm" variant="ghost" icon={<X aria-hidden="true" />} onPress={() => setUndo(null)} />
          </div>
        )}
        <PlaceList w={w} ui={ui} />
        {drag?.pointer && (
          <div className="pd-ghost" style={{ left: drag.x + 12, top: drag.y + 10 }} data-refused={(drag.verdict && !drag.verdict.ok) || undefined}>
            {drag.src.k === "chat" ? <Icon w={w} id={drag.src.chat} /> : <Move aria-hidden="true" className="ot-glyph" />}
            <span>{srcName(w, drag.src)}</span>
          </div>
        )}
        {!first.bare && (
          <div className="ot-bar" onMouseDown={(e) => e.stopPropagation()}>
            <div className="ot-bar-row">
              <button aria-label="Previous scene" onClick={() => go(SCENES[(SCENES.indexOf(scene) - 1 + SCENES.length) % SCENES.length]!)}><ChevronLeft /></button>
              <select value={scene.key} onChange={(e) => go(SCENES.find((x) => x.key === e.target.value)!)} aria-label="Scene">
                {SCENES.map((s, i) => <option key={s.key} value={s.key}>{i + 1}. {s.title}</option>)}
              </select>
              <button aria-label="Next scene" onClick={() => go(SCENES[(SCENES.indexOf(scene) + 1) % SCENES.length]!)}><ChevronRight /></button>
              <span className="ot-bar-sep" />
              {(["A", "B", "C"] as const).map((x) => <button key={x} aria-pressed={v === x} onClick={() => go(scene, x)}>{x} · {VARIANTS[x]}</button>)}
              <span className="ot-bar-sep" />
              <span>Other workspace:</span>
              {(["allow", "refuse"] as const).map((x) => <button key={x} aria-pressed={rules.otherWs === x} onClick={() => go(scene, v, x)}>{x}</button>)}
              <span className="ot-bar-sep" />
              <button onClick={() => go(scene)}>Reset</button>
              <button onClick={() => onLook({ ...look, appearance: look.appearance === "dark" ? "light" : "dark" })}>{look.appearance === "dark" ? "Light" : "Dark"}</button>
            </div>
            <div className="ot-bar-says">{scene.says}</div>
            <div className="ot-bar-says ot-bar-time">Screen reader hears: {said || "—"} · Tree: {shape(t.root)}</div>
          </div>
        )}
      </div>
    </ChatsHere.Provider>
  );
}
