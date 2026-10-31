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
 * - the window a project is moved out of says what it carries (`moving`) before the move, and
 *   the main window says it has it: without that answer the move asks instead, as a move that
 *   could not carry the work loses it;
 * - each split window tells it what work it holds whenever that changes (`mirror`), which is
 *   what a split window's close hands back, since the page goes with the window;
 * - the window a project arrives in says it holds it (`claim`), once it is listening for the
 *   work, and the main window hands the work to it (`handed`).
 */
import { useCallback, useEffect, useRef, useState } from "react";
import { emit, emitTo } from "@tauri-apps/api/event";
import { commands } from "./bindings";
import { listen } from "./here";
import { MAIN, thisWindow, WINDOWS_CHANGED } from "./windows";
import { handedOver, takeHanded, unsavedNow, watchUnsaved, type HandedWork } from "./unsavedWork";
import type { UnsavedAsk } from "./UnsavedWorkQuestion";

/** A split window's work, told to the main window whenever it changes. */
const MIRROR = "unsaved-mirror";
/** A project's work, carried out of the window it is moved from (`works`), or kept there after
 *  all where the move was refused (`null`). */
const MOVING = "unsaved-moving";
/** The main window's answer to a `MOVING`: it holds the work. */
const CARRIED = "unsaved-carried";
/** A window holds a project now: its work is to be handed to it. */
const CLAIM = "unsaved-claim";
/** Sent to one window: the work of the projects it now holds. */
const HANDED = "unsaved-handed";
/** The main window is listening: say again what you hold, as it may have reloaded. */
const LISTENING = "unsaved-listening";
/** Sent to one window at a quit: ask about your work. */
const ASK_AT_QUIT = "unsaved-ask-at-quit";
/** A window's first answer at a quit: it is asking. */
const ASKING_AT_QUIT = "unsaved-asking-at-quit";
/** A window's answer at a quit: go on, or the quit is not taken. */
const ANSWERED_AT_QUIT = "unsaved-answered-at-quit";

/** **How long a window has to answer that it heard**: a move carrying work, a quit asking. A
 *  window that does not is not taken at its silence: the move asks, the quit is not taken. */
export const HEARD_WITHIN_MS = 2000;

type Mirror = { label: string; works: HandedWork[] };
type Moving = { from: string; plane: string; works: HandedWork[] | null };
type Carried = { to: string; plane: string };
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

  /** **`label` holds `works` now**, as it said. */
  mirrored(label: string, works: HandedWork[]): void {
    this.mirrors.set(label, works);
  }

  /** **`from` is moving `plane` out, carrying `works`**, or kept it after all (`null`). */
  moving(from: string, plane: string, works: HandedWork[] | null): void {
    if (works !== null) this.transit.set(plane, { from, works });
    else if (this.transit.get(plane)?.from === from) this.transit.delete(plane);
  }

  /**
   * **`label` holds `plane` now**: the work to hand it. What a move carried out is first, and
   * otherwise what another window last said it held of the project, which is what a split
   * window's close hands back. Either way, no other window's record of it stays: a project is
   * held by one window.
   */
  claimed(label: string, plane: string): HandedWork[] {
    const moved = this.transit.get(plane);
    let works: HandedWork[] = [];
    if (moved !== undefined && moved.from !== label) {
      works = moved.works;
      this.transit.delete(plane);
    } else if (moved === undefined) {
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

  /** The split windows that last said they hold work: of `alive` and in its order, where it is
   *  given. */
  holding(alive?: readonly string[]): string[] {
    const labels = alive ?? [...this.mirrors.keys()];
    return labels.filter((label) => label !== MAIN && (this.mirrors.get(label)?.length ?? 0) > 0);
  }
}

/** The main window's one record: module-level, as the registry is. */
let handOver = new HandOver();

/** A fresh record, for tests. */
export function forgetHandOver(): void {
  handOver = new HandOver();
}

/** Hands `works` to the window `to`: registered here where it is this window. */
function deliver(to: string, works: HandedWork[]): void {
  if (works.length === 0) return;
  if (to === thisWindow()) return takeHanded(works);
  void emitTo(to, HANDED, works).catch(() => undefined);
}

/**
 * **Sends `event` to the window `to` and waits for `answer`** where `heard` says it is the one,
 * listening before it sends. `undefined` where none comes within {@link HEARD_WITHIN_MS}, or
 * the window cannot be told.
 */
async function askAndHear<T>(
  to: string,
  event: string,
  payload: unknown,
  answer: string,
  heard: (said: T) => boolean,
): Promise<T | undefined> {
  let stop: (() => void) | undefined;
  let timer: ReturnType<typeof setTimeout> | undefined;
  try {
    return await new Promise<T | undefined>((resolve) => {
      timer = setTimeout(() => resolve(undefined), HEARD_WITHIN_MS);
      listen<T>(answer, ({ payload: said }) => {
        if (heard(said)) resolve(said);
      })
        .then((unlisten) => {
          stop = unlisten;
          return emitTo(to, event, payload);
        })
        .catch(() => resolve(undefined));
    });
  } finally {
    clearTimeout(timer);
    stop?.();
  }
}

/**
 * **`plane` is about to be moved out of this window**: its work is carried to the main window
 * first (#4028), which holds it for the window the project goes to. Answers whether the work is
 * carried; where it is not, the move asks about it, as the work would be lost.
 */
export async function carryOut(plane: string): Promise<boolean> {
  const works = handedOver(plane);
  const from = thisWindow();
  if (from === MAIN) {
    handOver.moving(from, plane, works);
    return true;
  }
  const said = await askAndHear<Carried>(
    MAIN,
    MOVING,
    { from, plane, works } satisfies Moving,
    CARRIED,
    (one) => one.to === from && one.plane === plane,
  );
  return said !== undefined;
}

/** **The move of `plane` out of this window was refused**: the work it carried stays here. */
export function carriedBack(plane: string): void {
  const from = thisWindow();
  if (from === MAIN) return handOver.moving(from, plane, null);
  void emitTo(MAIN, MOVING, { from, plane, works: null } satisfies Moving).catch(() => undefined);
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
  const planesNow = useRef(planes);
  useEffect(() => {
    askNow.current = askUnsaved;
    planesNow.current = planes;
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
            handOver.moving(from, plane, works);
            if (works !== null)
              void emitTo(from, CARRIED, { to: from, plane } satisfies Carried).catch(
                () => undefined,
              );
          }),
          on<Claim>(CLAIM, ({ label, plane }) => deliver(label, handOver.claimed(label, plane))),
        ]);
        if (!gone) void emit(LISTENING, own).catch(() => undefined);
      } else {
        await Promise.all([
          // Only the work of a project still held here: one moved on meanwhile is not this
          // window's to keep.
          on<HandedWork[]>(HANDED, (works) =>
            takeHanded(works.filter((one) => planesNow.current.includes(one.plane))),
          ),
          on<string>(LISTENING, () => setHearing((was) => (was === 0 ? was : was + 1))),
          on<null>(ASK_AT_QUIT, () => {
            const tell = (event: string, said: object) =>
              void emitTo(MAIN, event, { label: own, ...said }).catch(() => undefined);
            tell(ASKING_AT_QUIT, {});
            const answer = (go: boolean) => tell(ANSWERED_AT_QUIT, { go });
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
      if (main) deliver(own, handOver.claimed(own, plane));
      else void emitTo(MAIN, CLAIM, { label: own, plane } satisfies Claim).catch(() => undefined);
    }
    for (const plane of claimedNow) if (!planes.includes(plane)) claimedNow.delete(plane);
  }, [hearing, main, own, planes]);

  // **A split window's work, told to the main window** whenever it changes, and anew when the
  // main window says it is listening. Watched rather than drawn: the window is not drawn again
  // for each keystroke in an editor.
  useEffect(() => {
    if (main || hearing === 0) return;
    const tell = () =>
      void emitTo(MAIN, MIRROR, { label: own, works: handedOver() } satisfies Mirror).catch(
        () => undefined,
      );
    tell();
    return watchUnsaved(tell);
  }, [hearing, main, own]);

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
      // No other window has said it holds work: nothing more to ask, and no one to ask.
      if (handOver.holding().length === 0) return done(go);
      // Which of them are still open is the core's. Not knowing is not a quit that loses nothing.
      const alive = await commands.charterWindows().catch(() => undefined);
      if (alive === undefined) return done(stay);
      const open = Array.isArray(alive) ? alive : [];
      const there = handOver.holding(open).find((label) => !askedThere.has(label));
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
 * on. A window that does not say it is asking, or goes meanwhile, is a quit not taken: a window
 * that went hands its projects and their work back to the main window, where the next quit asks
 * about them. A second quit asked within a few seconds quits anyway (`lifecycle.rs`), which is
 * the way out of a window that cannot answer.
 */
async function askedIn(label: string): Promise<boolean> {
  let stops: (() => void)[] = [];
  let answer: (go: boolean) => void = () => undefined;
  const answered = new Promise<boolean>((resolve) => (answer = resolve));
  try {
    stops = await Promise.all([
      listen<Answered>(ANSWERED_AT_QUIT, ({ payload }) => {
        if (payload.label === label) answer(payload.go);
      }),
      listen<string[]>(WINDOWS_CHANGED, ({ payload }) => {
        if (Array.isArray(payload) && !payload.includes(label)) answer(false);
      }),
    ]);
    const asking = await askAndHear<{ label: string }>(
      label,
      ASK_AT_QUIT,
      null,
      ASKING_AT_QUIT,
      (said) => said.label === label,
    );
    if (asking === undefined) answer(false);
    return await answered;
  } catch {
    return false;
  } finally {
    stops.forEach((stop) => stop());
  }
}
