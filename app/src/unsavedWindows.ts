// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **A view's unsaved work across windows** (#4028; `unsavedWork.ts`, `windows.ts`).
 *
 * The registry is each window's own, kept in memory. Two things reach across windows, and this
 * module is where they meet:
 *
 * - **A project goes to another window with its work.** A move (`moveProject`) and a split
 *   window's close, which hands its projects back to the main window (`windows.rs`), carry the
 *   work along: the window the project arrives in registers it again (`takeHanded`), and its
 *   view opens on the work as it was. Nothing is asked, because nothing is lost.
 * - **A quit asks in every window that holds work.** Only the main window is asked to quit
 *   (`lifecycle.rs`). It asks about its own work, then has each split window that holds work
 *   ask about its own, in front, one after another. Keep editing in any of them is a quit not
 *   taken.
 *
 * **The main window is the go-between** ({@link HandOver}). It never closes, so a project's
 * work waits there for the window it is going to, which may not have drawn yet:
 *
 * - each split window tells it what work it holds whenever that changes (`mirror`), which is
 *   what a split window's close hands back, since the page goes with the window;
 * - the window a project leaves by a move says what it carried out (`moving`);
 * - the window a project arrives in says it holds it (`claim`), once it is listening for the
 *   work, and the main window hands the work to it (`handed`). Whichever of the move and the
 *   claim is heard first, the work is handed once both are.
 */
import { useCallback, useEffect, useRef, useState } from "react";
import { emit, emitTo } from "@tauri-apps/api/event";
import { commands } from "./bindings";
import { listen } from "./here";
import { MAIN, thisWindow, WINDOWS_CHANGED } from "./windows";
import { handedOver, takeHanded, unsavedNow, useUnsavedWork, type HandedWork } from "./unsavedWork";
import type { UnsavedAsk } from "./UnsavedWorkQuestion";

/** A split window's work, told to the main window whenever it changes. */
const MIRROR = "unsaved-mirror";
/** A project's work, carried out of the window it was moved from. */
const MOVING = "unsaved-moving";
/** A window holds a project now: its work is to be handed to it. */
const CLAIM = "unsaved-claim";
/** Sent to one window: the work of the projects it now holds. */
const HANDED = "unsaved-handed";
/** The main window is listening: say again what you hold, as it may have reloaded. */
const LISTENING = "unsaved-listening";
/** Sent to one window at a quit: ask about your work. */
const ASK_AT_QUIT = "unsaved-ask-at-quit";
/** A window's answer at a quit: go on, or the quit is not taken. */
const ANSWERED_AT_QUIT = "unsaved-answered-at-quit";

type Mirror = { label: string; works: HandedWork[] };
type Moving = { from: string; plane: string; works: HandedWork[] };
type Claim = { label: string; plane: string };
type Answered = { label: string; go: boolean };

/**
 * **The main window's record of work on its way between windows** (#4028): pure, so what is
 * handed to whom is pinned without windows.
 */
export class HandOver {
  /** The work a move carried out of a window, by its project, until the project is claimed. */
  private readonly transit = new Map<string, { from: string; works: HandedWork[] }>();
  /** What each split window last said it holds. */
  private readonly mirrors = new Map<string, HandedWork[]>();
  /** The window that last said it holds each project. */
  private readonly holders = new Map<string, string>();

  /** **`label` holds `works` now**, as it said. */
  mirrored(label: string, works: HandedWork[]): void {
    this.mirrors.set(label, works);
  }

  /**
   * **`from` moved `plane` out, carrying `works`.** Answers where they go now, where the
   * window the project went to has claimed it already.
   */
  moving(from: string, plane: string, works: HandedWork[]): Delivery | undefined {
    this.transit.set(plane, { from, works });
    const holder = this.holders.get(plane);
    if (holder === undefined || holder === from) return undefined;
    return { to: holder, works: this.claimed(holder, plane) };
  }

  /**
   * **`label` holds `plane` now**: the work to hand it. What a move carried out is first, and
   * otherwise what another window last said it held of the project, which is what a split
   * window's close hands back. Either way, no other window's record of it stays: a project is
   * held by one window.
   */
  claimed(label: string, plane: string): HandedWork[] {
    this.holders.set(plane, label);
    const moved = this.transit.get(plane);
    let works: HandedWork[] = [];
    if (moved !== undefined && moved.from !== label) {
      works = moved.works;
      this.transit.delete(plane);
    } else {
      for (const [from, held] of this.mirrors)
        if (from !== label) works.push(...held.filter((one) => one.plane === plane));
    }
    for (const [from, held] of this.mirrors)
      if (from !== label)
        this.mirrors.set(
          from,
          held.filter((one) => one.plane !== plane),
        );
    return works;
  }

  /** The split windows of `alive` that last said they hold work, in their order. */
  holding(alive: readonly string[]): string[] {
    return alive.filter((label) => label !== MAIN && (this.mirrors.get(label)?.length ?? 0) > 0);
  }
}

/** Work to hand to the window `to`. */
export type Delivery = { to: string; works: HandedWork[] };

/** The main window's one record: module-level, as the registry is. */
let handOver = new HandOver();

/** A fresh record, for tests. */
export function forgetHandOver(): void {
  handOver = new HandOver();
}

/** Hands `works` to the window `to`: registered here where it is this window. */
function deliver({ to, works }: Delivery): void {
  if (works.length === 0) return;
  if (to === thisWindow()) return takeHanded(works);
  void emitTo(to, HANDED, works).catch(() => undefined);
}

/**
 * **`plane` was moved out of this window**: its work goes with it (#4028). Said after the move
 * is taken and before the project is let go of here, which lets its work go.
 */
export async function carryOut(plane: string): Promise<void> {
  const works = handedOver(plane);
  const from = thisWindow();
  if (from === MAIN) {
    const now = handOver.moving(from, plane, works);
    if (now !== undefined) deliver(now);
    return;
  }
  await emitTo(MAIN, MOVING, { from, plane, works } satisfies Moving).catch(() => undefined);
}

/**
 * **Every window's half of carrying work across windows** (#4028): this window's work told to
 * the main window, and each project it holds claimed once it is listening for the work. In the
 * main window, the go-between's record is kept. Answers `askAtQuit`, the main window's question
 * at a quit, asked in every window that holds work.
 *
 * `askUnsaved` is the window's one question about unsaved work (`App`'s).
 */
export function useUnsavedAcrossWindows({
  planes,
  askUnsaved,
}: {
  planes: readonly string[];
  askUnsaved: (ask: UnsavedAsk) => void;
}): { askAtQuit: (go: () => void, stay: () => void) => void } {
  const own = thisWindow();
  const main = own === MAIN;
  /** **How many times this window has said what it holds**: 0 until it hears handed work,
   *  since a claim before then would lose it, and one more each time the main window says it
   *  is listening anew. */
  const [hearing, setHearing] = useState(0);
  const askNow = useRef(askUnsaved);
  useEffect(() => {
    askNow.current = askUnsaved;
  });

  useEffect(() => {
    let gone = false;
    const stops: (() => void)[] = [];
    const on = async <T>(event: string, heard: (payload: T) => void) => {
      const stop = await listen<T>(event, (e) => {
        if (!gone) heard(e.payload);
      }).catch(() => undefined);
      if (gone) stop?.();
      else if (stop) stops.push(stop);
    };
    void (async () => {
      if (main) {
        await Promise.all([
          on<Mirror>(MIRROR, ({ label, works }) => handOver.mirrored(label, works)),
          on<Moving>(MOVING, ({ from, plane, works }) => {
            const now = handOver.moving(from, plane, works);
            if (now !== undefined) deliver(now);
          }),
          on<Claim>(CLAIM, ({ label, plane }) =>
            deliver({ to: label, works: handOver.claimed(label, plane) }),
          ),
        ]);
        if (!gone) void emit(LISTENING, own).catch(() => undefined);
      } else {
        await Promise.all([
          on<HandedWork[]>(HANDED, (works) => takeHanded(works)),
          on<string>(LISTENING, () => setHearing((was) => (was === 0 ? was : was + 1))),
          on<null>(ASK_AT_QUIT, () => {
            const answer = (go: boolean) =>
              void emitTo(MAIN, ANSWERED_AT_QUIT, { label: own, go } satisfies Answered).catch(
                () => undefined,
              );
            const work = unsavedNow();
            if (work.length === 0) return answer(true);
            // Asked where the work is, in front.
            void commands.showWindowHolding(work[0].plane).catch(() => undefined);
            askNow.current({ work, then: () => answer(true), kept: () => answer(false) });
          }),
        ]);
      }
      if (!gone) setHearing((was) => (was === 0 ? 1 : was));
    })();
    return () => {
      gone = true;
      stops.forEach((stop) => stop());
    };
  }, [main, own]);

  // **Each project this window holds is claimed**, once: the work a move or a close carried
  // is handed here. Claimed again after the main window says it is listening anew.
  const claimed = useRef({ at: 0, planes: new Set<string>() });
  useEffect(() => {
    if (hearing === 0) return;
    if (claimed.current.at !== hearing) claimed.current = { at: hearing, planes: new Set() };
    const { planes: claimedNow } = claimed.current;
    for (const plane of planes) {
      if (claimedNow.has(plane)) continue;
      claimedNow.add(plane);
      if (main) deliver({ to: own, works: handOver.claimed(own, plane) });
      else void emitTo(MAIN, CLAIM, { label: own, plane } satisfies Claim).catch(() => undefined);
    }
    for (const plane of claimedNow) if (!planes.includes(plane)) claimedNow.delete(plane);
  }, [hearing, main, own, planes]);

  // **A split window's work, told to the main window** whenever it changes, and anew when the
  // main window says it is listening.
  const work = useUnsavedWork();
  useEffect(() => {
    if (main || hearing === 0) return;
    void emitTo(MAIN, MIRROR, { label: own, works: handedOver() } satisfies Mirror).catch(
      () => undefined,
    );
  }, [hearing, main, own, work]);

  const quitting = useRef(false);
  const askAtQuit = useCallback((go: () => void, stay: () => void) => {
    if (quitting.current) return stay();
    quitting.current = true;
    const done = (then: () => void) => {
      quitting.current = false;
      then();
    };
    /** The work asked about here, and the windows asked, in this quit. */
    const askedHere = new Set<string>();
    const askedThere = new Set<string>();
    const next = async (): Promise<void> => {
      const here = unsavedNow().filter((one) => !askedHere.has(one.key));
      if (here.length > 0) {
        for (const one of here) askedHere.add(one.key);
        return askNow.current({
          work: here,
          then: () => void next(),
          kept: () => done(stay),
        });
      }
      const alive = await commands.charterWindows().catch(() => [] as string[]);
      const there = handOver.holding(alive).find((label) => !askedThere.has(label));
      if (there === undefined) return done(go);
      askedThere.add(there);
      if (await askedIn(there)) return next();
      done(stay);
    };
    void next();
  }, []);
  return { askAtQuit };
}

/**
 * **Has the window `label` ask about its work at a quit**, and answers whether the quit goes
 * on. A window that goes meanwhile is a quit not taken: its projects, and their work, come back
 * to the main window, where the next quit asks about them.
 */
async function askedIn(label: string): Promise<boolean> {
  let stops: (() => void)[] = [];
  const answered = new Promise<boolean>((resolve) => {
    const end = (go: boolean) => {
      stops.forEach((stop) => stop());
      resolve(go);
    };
    // Listening before the window is asked: one with nothing to ask about answers at once.
    void Promise.all([
      listen<Answered>(ANSWERED_AT_QUIT, ({ payload }) => {
        if (payload.label === label) end(payload.go);
      }),
      listen<string[]>(WINDOWS_CHANGED, ({ payload }) => {
        if (Array.isArray(payload) && !payload.includes(label)) end(false);
      }),
    ])
      .then((stopping) => {
        stops = stopping;
        return emitTo(label, ASK_AT_QUIT, null);
      })
      .catch(() => end(false));
  });
  return answered;
}
