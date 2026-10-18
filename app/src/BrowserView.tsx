// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { Channel } from "@tauri-apps/api/core";
import { Globe } from "lucide-react";
import type { KeyboardEvent as ReactKeyboardEvent, PointerEvent, WheelEvent } from "react";
import { useCallback, useEffect, useRef, useState } from "react";
import { CHAT_KEYBOARD } from "./actions";
import { type BrowserFrame, type BrowserInput, commands } from "./bindings";
import {
  SIZE_PRESETS,
  drivenBy,
  handsBack,
  handsBackSaid,
  keyInput,
  mouseInput,
  pagePoint,
  wheelInput,
} from "./browserDriving";
import { frameBytes, latestOnly, paint } from "./browserFrames";
import { addressChip, thisDevice } from "./browsers";
import { useWindowSeen } from "./browserWatch";
import { EmptyState } from "./EmptyState";
import { Select } from "./primitives/Select";
import { useProjectShown } from "./projectShown";
import { RECONNECTING_SAID } from "./reconnecting";
import { onAMac } from "./tabKeys";

/** The keys that are only held with another. */
const ONLY_HELD = new Set(["Shift", "Control", "Alt", "Meta"]);

/** Where a browser view stands: opening, drawing, ended, or refused with the host's words. */
type Shown =
  | { kind: "opening" }
  | { kind: "live"; width: number; height: number }
  | { kind: "ended" }
  | { kind: "refused"; why: string };

/** What waits to be sent: one event of the person's, or their hand-back, after every event
 *  before it. */
type Queued = { input: BrowserInput } | { handBack: () => Promise<void> };

/**
 * Sends the person's events to the browser one at a time, in the order they came: a key's down
 * before its up, a press before its release, and the hand-back after every key typed before it.
 * A move waiting behind another is replaced by the newer one, so a quick mouse never queues a
 * backlog. `clear` drops whatever waits: a view watched again starts with nothing queued, so
 * nothing aimed at an earlier frame is sent after it.
 */
function useSender(program: string, said: (why: string | undefined) => void) {
  const waiting = useRef<Queued[]>([]);
  const sending = useRef(false);
  const next = useCallback(async () => {
    if (sending.current) return;
    sending.current = true;
    try {
      for (let one = waiting.current.shift(); one; one = waiting.current.shift()) {
        if ("handBack" in one) {
          await one.handBack();
          continue;
        }
        const sent = await commands.browserInput(program, one.input);
        if (sent.status === "error") said(sent.error);
        else if (sent.data.outcome === "refused") said(sent.data.why);
        // A missed one was aimed at a page that has moved on: it is dropped, as it should be.
      }
    } finally {
      sending.current = false;
    }
  }, [program, said]);
  const send = useCallback(
    (input: BrowserInput) => {
      const queue = waiting.current;
      const last = queue[queue.length - 1];
      const aMove = (one: Queued | undefined) =>
        one !== undefined &&
        "input" in one &&
        one.input.kind === "mouse" &&
        one.input.action === "moved";
      if (aMove({ input }) && aMove(last)) queue[queue.length - 1] = { input };
      else queue.push({ input });
      void next();
    },
    [next],
  );
  const after = useCallback(
    (handBack: () => Promise<void>) => {
      waiting.current.push({ handBack });
      void next();
    },
    [next],
  );
  const clear = useCallback(() => {
    waiting.current = [];
  }, []);
  return { send, after, clear };
}

/**
 * **A browser, drawn live** (#2012, spec #2008): the browser view of `program`, a browser the
 * session host runs. Its frames come whole, as images, and are drawn on a canvas, scaled to the
 * pane; nothing of the page runs in the window. While it is on screen it is the focused view,
 * sent every frame. When nobody can see it (its project is not the one in front, or the window
 * is hidden) its view closes and the last frame stays drawn, and a browser nobody watches makes
 * no frames; it is attached again, from its latest frame, when it can be seen.
 *
 * **Clicking or typing into it takes it over** (#2014): the bar above the page, drawn by purlis
 * outside the frame, says the agent is paused, and the person's clicks, moves, wheel and keys
 * go to the page, each aimed at what the frame they saw showed. Only Done, or the one chord
 * ({@link handsBack}), hands it back; the session host keeps the browser theirs until then,
 * across a crash of the window too. A size preset is the person's too, so picking one takes the
 * browser over.
 *
 * **A browser on a runner whose link is down says "Reconnecting…" over its last frame**
 * (#2542), quietly, as a runner's chat does (#2496), until the core says it is reconnected: the
 * link is back, or the view drew again from a new snapshot.
 */
export function BrowserView({ program }: { program: string }) {
  const canvas = useRef<HTMLCanvasElement>(null);
  const [shown, setShown] = useState<Shown>({ kind: "opening" });
  const [driving, setDriving] = useState(false);
  const [reconnecting, setReconnecting] = useState(false);
  const [said, setSaid] = useState<string | undefined>();
  const [size, setSize] = useState<string>(SIZE_PRESETS[0].id);
  const drives = useRef(false);
  const taking = useRef<Promise<boolean> | null>(null);
  /** What the latest frame showed: what the person's input is aimed at. */
  const aim = useRef("");
  /** The page's size in CSS pixels: what a point of the view is mapped onto. */
  const page = useRef({ width: 0, height: 0 });
  const inFront = useProjectShown();
  const windowSeen = useWindowSeen();
  const seen = inFront && windowSeen;
  const mac = onAMac();
  const { send, after, clear } = useSender(program, setSaid);

  const drivingNow = useCallback((now: boolean) => {
    drives.current = now;
    setDriving(now);
  }, []);

  useEffect(() => {
    if (!seen) return;
    clear();
    let gone = false;
    let view: number | undefined;
    const draw = latestOnly((jpeg) =>
      canvas.current === null ? Promise.resolve() : paint(canvas.current, jpeg),
    );
    const frames = new Channel<BrowserFrame>();
    frames.onmessage = (frame) => {
      if (gone) return;
      // Where the view stands while its runner's link is down and back: no image of its own.
      if (frame.reconnection !== null) {
        setReconnecting(frame.reconnection === "reconnecting");
        return;
      }
      // An empty frame is the host's word that the view has ended for good.
      if (frame.jpeg === "") {
        setShown({ kind: "ended" });
        return;
      }
      // A frame comes only over a link that is up: a view watched again after its runner's
      // link came back is drawn as it comes.
      setReconnecting(false);
      aim.current = frame.aim;
      draw(frameBytes(frame.jpeg));
    };
    void commands.watchBrowser(program, "focused", frames).then((watched) => {
      if (watched.status === "error") {
        if (!gone) setShown({ kind: "refused", why: watched.error });
        return;
      }
      view = watched.data.view;
      if (gone) {
        void commands.unwatchBrowser(program, view);
        return;
      }
      page.current = { width: watched.data.width, height: watched.data.height };
      const sized = SIZE_PRESETS.find(
        (one) => one.width === watched.data.width && one.height === watched.data.height,
      );
      if (sized !== undefined) setSize(sized.id);
      drivingNow(watched.data.person_drives);
      setShown({ kind: "live", width: watched.data.width, height: watched.data.height });
    });
    return () => {
      gone = true;
      if (view !== undefined) void commands.unwatchBrowser(program, view);
    };
  }, [program, seen, drivingNow, clear]);

  /** Takes the browser over, once: answers whether the person drives it now. */
  const takeOver = useCallback((): Promise<boolean> => {
    if (drives.current) return Promise.resolve(true);
    taking.current ??= commands.takeOverBrowser(program).then((took) => {
      taking.current = null;
      if (took.status === "error") {
        setSaid(took.error);
        return false;
      }
      setSaid(undefined);
      drivingNow(true);
      return true;
    });
    return taking.current;
  }, [program, drivingNow]);

  // After every key already typed: none of them is lost to the hand-back.
  const handBack = useCallback(() => {
    after(async () => {
      const back = await commands.handBackBrowser(program);
      if (back.status === "error") {
        setSaid(back.error);
        return;
      }
      setSaid(undefined);
      drivingNow(false);
    });
  }, [after, program, drivingNow]);

  // While the person drives, the palette's pass-through key, pressed again, reaches the page.
  useEffect(() => {
    if (!driving) return;
    return drivenBy(program, (key) => {
      const named = {
        key,
        code: key,
        keyCode: 0,
        altKey: false,
        ctrlKey: false,
        metaKey: false,
        shiftKey: false,
      };
      send(keyInput(aim.current, true, named));
      send(keyInput(aim.current, false, named));
    });
  }, [driving, program, send]);

  const pointAt = (e: { clientX: number; clientY: number }) => {
    const box = canvas.current?.getBoundingClientRect();
    if (box === undefined) return undefined;
    return pagePoint({ x: e.clientX, y: e.clientY }, box, page.current);
  };

  const onPointerDown = (e: PointerEvent<HTMLDivElement>) => {
    const point = pointAt(e);
    if (point === undefined) return;
    e.preventDefault();
    e.currentTarget.focus();
    e.currentTarget.setPointerCapture?.(e.pointerId);
    const pressed = mouseInput(aim.current, "pressed", point, e);
    void takeOver().then((took) => took && send(pressed));
  };

  /** Sends `input` while the person drives, or once a take-over under way answers. */
  const whileDriving = (input: BrowserInput) => {
    if (drives.current) send(input);
    else void taking.current?.then((took) => took && send(input));
  };

  const onPointerUp = (e: PointerEvent<HTMLDivElement>) => {
    const point = pointAt(e);
    if (point === undefined) return;
    whileDriving(mouseInput(aim.current, "released", point, e));
  };

  const onPointerMove = (e: PointerEvent<HTMLDivElement>) => {
    const point = pointAt(e);
    if (!drives.current || point === undefined) return;
    send(mouseInput(aim.current, "moved", point, e));
  };

  const onWheel = (e: WheelEvent<HTMLDivElement>) => {
    const point = pointAt(e);
    if (!drives.current || point === undefined) return;
    send(wheelInput(aim.current, point, e));
  };

  const onKeyDown = (e: ReactKeyboardEvent<HTMLDivElement>) => {
    if (handsBack(e.nativeEvent, mac)) {
      e.preventDefault();
      if (drives.current) handBack();
      return;
    }
    // A modifier alone is not typing: it takes nothing over, and reaches a page only with the
    // key it is held for.
    if (!drives.current && ONLY_HELD.has(e.key)) return;
    e.preventDefault();
    const down = keyInput(aim.current, true, e);
    void takeOver().then((took) => took && send(down));
  };

  const onKeyUp = (e: ReactKeyboardEvent<HTMLDivElement>) => {
    if (handsBack(e.nativeEvent, mac)) return;
    e.preventDefault();
    whileDriving(keyInput(aim.current, false, e));
  };

  const pickSize = (id: string) => {
    const preset = SIZE_PRESETS.find((one) => one.id === id);
    if (preset === undefined) return;
    void takeOver().then(async (took) => {
      if (!took) return;
      const sized = await commands.sizeBrowser(program, preset.width, preset.height);
      if (sized.status === "error") {
        setSaid(sized.error);
        return;
      }
      page.current = { width: preset.width, height: preset.height };
      setSize(id);
    });
  };

  if (shown.kind === "ended")
    return (
      <EmptyState
        mark={Globe}
        headline="This browser has ended"
        body="Its pages are gone. Open a new browser from the palette to start again."
        testid="browser-ended"
      />
    );
  if (shown.kind === "refused")
    return (
      <EmptyState
        mark={Globe}
        headline="This browser cannot be shown"
        body={shown.why}
        testid="browser-refused"
      />
    );
  const keyboard = driving ? { [CHAT_KEYBOARD]: "" } : {};
  return (
    <div className="browser-view" data-testid="browser-view">
      <div className="browser-bar" data-driving={driving ? "person" : "agent"}>
        {/* **Where the page is, and on which device** (#2024): drawn by purlis, outside the
            page, so a page cannot fake it. */}
        <BrowserChip mac={mac} />
        <p className="browser-bar-said" role="status">
          {/* The bar is the view's one status: what is said over the page is said here too. */}
          {reconnecting && `${RECONNECTING_SAID} `}
          {driving
            ? "You are driving this browser. The agent is paused until you press Done."
            : "Click or type in the page to take it over."}
          {said !== undefined && <span className="browser-bar-refused"> {said}</span>}
        </p>
        <Select
          className="browser-bar-size"
          aria-label="Page size"
          value={size}
          options={SIZE_PRESETS.map((preset) => ({
            value: preset.id,
            label: `${preset.label}, ${preset.width} × ${preset.height}`,
          }))}
          onValueChange={pickSize}
        />
        {driving && (
          <button
            type="button"
            className="ui-button ui-button-primary ui-button-sm"
            title={`Hand the browser back to the agent (${handsBackSaid(mac)})`}
            onClick={handBack}
          >
            Done
          </button>
        )}
      </div>
      <div
        className={reconnecting ? "browser-surface reconnecting" : "browser-surface"}
        data-testid="browser-surface"
        tabIndex={0}
        aria-label="The browser's page: click or type to take it over"
        {...keyboard}
        onPointerDown={onPointerDown}
        onPointerUp={onPointerUp}
        onPointerMove={onPointerMove}
        onWheel={onWheel}
        onKeyDown={onKeyDown}
        onKeyUp={onKeyUp}
        onContextMenu={(e) => e.preventDefault()}
      >
        <canvas
          ref={canvas}
          className="browser-frame"
          aria-label="The browser's page"
          width={shown.kind === "live" ? shown.width : undefined}
          height={shown.kind === "live" ? shown.height : undefined}
        />
        {/* Seen over the page; a screen reader hears it in the bar's status above. */}
        {reconnecting && (
          <div className="pane-reconnecting" aria-hidden="true">
            <span className="pane-reconnecting-said ui-text-secondary">{RECONNECTING_SAID}</span>
          </div>
        )}
      </div>
    </div>
  );
}

/** A browser's address and device, as its view says them above its page (#2024). */
function BrowserChip({ mac }: { mac: boolean }) {
  const chip = addressChip(null, thisDevice(mac));
  return (
    <span className="browser-chip" data-testid="browser-chip" title={chip.whole ?? undefined}>
      {chip.said}
    </span>
  );
}
