// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { Globe } from "lucide-react";
import type {
  FormEvent,
  KeyboardEvent as ReactKeyboardEvent,
  PointerEvent,
  WheelEvent,
} from "react";
import { useCallback, useEffect, useRef, useState } from "react";
import { CHAT_KEYBOARD } from "./actions";
import {
  type BrowserInput,
  type BrowserWatching,
  commands,
  type GrantLevel,
  type Offered,
} from "./bindings";
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
import { leavesTheSurface } from "./leaveKeys";
import { addressChip, deviceOf, originOfAddress, takeGoWhenShown } from "./browsers";
import { INSPECTOR_OPENS_AT, askToInspect } from "./inspector";
import { useBrowserOrigin, useBrowserWatch, useWindowSeen } from "./browserWatch";
import { poolOf, programsOf, useChatBrowsers } from "./chatBrowsers";
import { EmptyState } from "./ui/empty-state/EmptyState";
import { Notice, type NoticeAction, NoticePaneRow } from "./Notice";
import { Select } from "./ui/select/Select";
import { useProjectShown } from "./projectShown";
import { RECONNECTING_SAID } from "./reconnecting";
import { sandboxCommandReturned } from "./sandboxAsked";
import { onAMac } from "./tabKeys";
import { Button } from "./ui/button/Button";
import { Form, TextField } from "./ui/field/Field";
import { Tooltip } from "./ui/tooltip/Tooltip";

/** The keys that are only held with another. */
const ONLY_HELD = new Set(["Shift", "Control", "Alt", "Meta"]);

/** What the bar says when the person's key or click was dropped because the page moved on
 *  (#2304): the page, or a frame inside it, loaded another document after the frame they acted
 *  on was drawn, so the input never reached something they did not see. */
export const DROPPED =
  "The page changed before your key or click reached it, so purlis dropped it. Type or click again.";

/** Whether `input` is one the person would miss if it were dropped: a key pressed, or a mouse
 *  button pressed or let go. A move or a wheel is followed by the next one. */
function anAct(input: BrowserInput) {
  if (input.kind === "key") return input.action === "down";
  return input.kind === "mouse" && (input.action === "pressed" || input.action === "released");
}

/** What waits to be sent: one event of the person's, or their hand-back, after every event
 *  before it. */
type Queued = { input: BrowserInput } | { handBack: () => Promise<void> };

/**
 * Sends the person's events to the browser one at a time, in the order they came: a key's down
 * before its up, a press before its release, and the hand-back after every key typed before it.
 * A move waiting behind another is replaced by the newer one, so a quick mouse never queues a
 * backlog. `clear` drops whatever waits: a view watched again starts with nothing queued, so
 * nothing aimed at an earlier frame is sent after it. A key or click the page moved on from is
 * told to `dropped`, and the next one that reaches the page clears it (#2304). An answer that
 * comes back after `clear` belongs to the watch before, so it says nothing.
 */
export function useSender(
  program: string,
  said: (why: string | undefined) => void,
  dropped: (now: boolean) => void,
  deliver: typeof commands.browserInput = commands.browserInput,
) {
  const waiting = useRef<Queued[]>([]);
  const sending = useRef(false);
  /** Counts the clears: an answer is heard only if no clear came while it was asked. */
  const watch = useRef(0);
  const next = useCallback(async () => {
    if (sending.current) return;
    sending.current = true;
    try {
      for (let one = waiting.current.shift(); one; one = waiting.current.shift()) {
        if ("handBack" in one) {
          await one.handBack();
          continue;
        }
        const asked = watch.current;
        const sent = await deliver(program, one.input);
        if (asked !== watch.current) continue;
        if (sent.status === "error") said(sent.error);
        else if (sent.data.outcome === "refused") said(sent.data.why);
        // A missed one was aimed at a page that has moved on: it is dropped, as it should be,
        // and the bar says so until the person's next key or click reaches the page.
        else if (anAct(one.input)) dropped(sent.data.outcome === "missed");
      }
    } finally {
      sending.current = false;
    }
  }, [program, said, dropped, deliver]);
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
    watch.current += 1;
    dropped(false);
  }, [dropped]);
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
 *
 * **Inspect opens the page's real DevTools beside it** (#2028): the engine's own, drawn in a
 * view of their own from frames as the page is. Opening them takes the browser over, and they
 * close as the person hands it back. A browser on a pool's runner has none here.
 *
 * **A browser the person opened goes where they send it** (#2176): its view has an address
 * field, drawn by purlis outside the frame. The address goes as an address the agent writes
 * would, so a site new to the project is held back and asked about here, once, under the
 * field; http and https only, any other scheme refused in words. A chat's browser has none: its
 * chat drives it.
 */
export function BrowserView({ plane, program }: { plane: string; program: string }) {
  const canvas = useRef<HTMLCanvasElement>(null);
  const [driving, setDriving] = useState(false);
  const [said, setSaid] = useState<string | undefined>();
  const [dropped, setDropped] = useState(false);
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
  const { send, after, clear } = useSender(program, setSaid, setDropped);
  const { browsers: chatBrowsers, kept, read: chatBrowsersRead } = useChatBrowsers(plane);
  /** Whether it is one the person opened: no chat of the project's uses it, and it is no chat's
   *  pinned past its chat (#2022), which only the window that opened it sends anywhere. */
  const theirs = chatBrowsersRead && !programsOf(chatBrowsers, kept).has(program);
  /** The address the person typed, the origin it went to, and the sites it asks. */
  const [typed, setTyped] = useState("");
  const [wentTo, setWentTo] = useState<string | null>(null);
  /** Where its page is, as its views are told (#2300); before a host says, where it went. */
  const told = useBrowserOrigin(program);
  const origin = told === undefined ? wentTo : told;
  const [asking, setAsking] = useState<SitesAsked | null>(null);
  const [going, setGoing] = useState(false);
  /** The pool whose runner it runs on, as its view said (#2050), until it says: unknown. */
  const [pool, setPool] = useState<string | null | undefined>(undefined);
  /** Its chip's device (#2050): the pool the chats' browsers list it on, else the one its view
   *  said. Until one of them says, none is named: a list read before it names this program
   *  says nothing of where it runs. */
  const listedOn = poolOf(chatBrowsers, kept, program);
  const onPool = listedOn !== undefined ? listedOn : pool;
  const device = onPool === undefined ? null : deviceOf(onPool, mac);

  const drivingNow = useCallback((now: boolean) => {
    drives.current = now;
    setDriving(now);
  }, []);

  // A view watched again starts with nothing queued (`useSender`).
  useEffect(() => {
    if (seen) clear();
  }, [program, seen, clear]);

  const attached = useCallback(
    (watching: BrowserWatching) => {
      page.current = { width: watching.width, height: watching.height };
      const sized = SIZE_PRESETS.find(
        (one) => one.width === watching.width && one.height === watching.height,
      );
      if (sized !== undefined) setSize(sized.id);
      drivingNow(watching.person_drives);
      setPool(watching.pool ?? null);
    },
    [drivingNow],
  );
  const { watched: shown, reconnecting } = useBrowserWatch(
    program,
    "focused",
    canvas,
    seen,
    undefined,
    {
      aimed: (at) => {
        aim.current = at;
      },
      attached,
    },
  );

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
      setDropped(false);
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

  /** Opens the page's DevTools beside it (#2028), taking the browser over first. */
  const inspect = () => {
    void takeOver().then(async (took) => {
      if (!took) return;
      const opened = await commands.inspectBrowser(
        program,
        INSPECTOR_OPENS_AT.width,
        INSPECTOR_OPENS_AT.height,
      );
      if (opened.status === "error") {
        setSaid(opened.error);
        return;
      }
      askToInspect(plane, program);
    });
  };

  /** Sends the browser to `to`, the address as the person typed it, after every key and click
   *  already queued. The browser takes an address on its own lane, so one the person drives is
   *  handed back first; their next click or key takes it over again. */
  const go = useCallback(
    (to: string) => {
      if (going) return;
      setGoing(true);
      after(async () => {
        if (drives.current) {
          const back = await commands.handBackBrowser(program);
          if (back.status === "error") {
            setGoing(false);
            setSaid(back.error);
            return;
          }
          setDropped(false);
          drivingNow(false);
        }
        const went = await commands.goBrowser(plane, program, to);
        setGoing(false);
        if (went.status === "error") {
          setSaid(went.error);
          return;
        }
        setSaid(undefined);
        if (went.data.outcome === "asks") {
          setAsking({ address: to, sites: went.data.sites, answers: went.data.answers });
          return;
        }
        setAsking(null);
        if (went.data.address !== null) setWentTo(originOfAddress(went.data.address));
      });
    },
    [going, after, plane, program, drivingNow],
  );

  // An address the window was asked to open this browser at (the browsers open at the last
  // quit, #2022) goes once the view is live, as if the person typed it.
  useEffect(() => {
    if (!theirs || shown.kind !== "live") return;
    const at = takeGoWhenShown(program);
    if (at === undefined) return;
    // Sent after this render, as the person's own Go would be.
    queueMicrotask(() => go(at));
  }, [theirs, shown.kind, program, go]);

  const onGo = (e: FormEvent<HTMLFormElement>) => {
    e.preventDefault();
    go(typed);
  };

  /** An Allow of the sites ask: on this machine (every browser of the project here) or for
   *  everyone in the project (committed); this one goes there now. */
  const allowSites = (ask: SitesAsked, level: GrantLevel) => {
    void commands.allowPersonBrowserSites(plane, program, ask.sites, level).then((allowed) => {
      if (allowed.status === "error") {
        setSaid(allowed.error);
        return;
      }
      sandboxCommandReturned();
      setAsking(null);
      go(ask.address);
    });
  };

  /** One answer of the sites ask: an Allow at its scope, or Keep blocked putting it away. */
  const answerSites = (ask: SitesAsked, one: Offered) => {
    if (one.allows) allowSites(ask, one.id as GrantLevel);
    else setAsking(null);
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
      {theirs && (
        <Form className="browser-address" aria-label="Go to an address" onSubmit={onGo}>
          {/* One size for the row: the box is as small as Go beside it. */}
          <TextField
            size="sm"
            className="browser-address-text"
            aria-label="Address"
            placeholder="Type an address, such as https://example.com"
            value={typed}
            onChange={setTyped}
          />
          <Button
            size="sm"
            type="submit"
            aria-disabled={going || typed.trim() === "" ? true : undefined}
          >
            Go
          </Button>
        </Form>
      )}
      {theirs && asking !== null && (
        <NoticePaneRow>
          <Notice
            at="pane"
            cause={`browser-sites:${program}`}
            fixes={
              // The core's answers, Keep blocked among them; Keep blocked alone were there none.
              (asking.answers.length > 0
                ? asking.answers.map((one): NoticeAction => ({
                    label: one.label,
                    allows: one.allows,
                    onPress: () => answerSites(asking, one),
                  }))
                : [{ label: KEEP_SAID, onPress: () => setAsking(null) }]) as [
                NoticeAction,
                ...NoticeAction[],
              ]
            }
          >
            {asking.sites.join(", ")} {asking.sites.length === 1 ? "is a site" : "are sites"} new to
            this project, so this browser did not go there.
          </Notice>
        </NoticePaneRow>
      )}
      <div className="browser-bar" data-driving={driving ? "person" : "agent"}>
        {/* **Where the page is, and on which device** (#2024): drawn by purlis, outside the
            page, so a page cannot fake it. */}
        <BrowserChip origin={origin} device={device} />
        <p className="browser-bar-said" role="status">
          {/* The bar is the view's one status: what is said over the page is said here too. */}
          {reconnecting && `${RECONNECTING_SAID} `}
          {driving
            ? "You are driving this browser. The agent is paused until you press Done."
            : // Nothing reaches the page while its runner's link is down: no hint to try.
              !reconnecting && "Click or type in the page to take it over."}
          {dropped && <span className="browser-bar-dropped"> {DROPPED}</span>}
          {said !== undefined && <span className="browser-bar-refused"> {said}</span>}
        </p>
        <Select
          className="browser-bar-size"
          size="sm"
          aria-label="Page size"
          value={size}
          options={SIZE_PRESETS.map((preset) => ({
            value: preset.id,
            label: `${preset.label}, ${preset.width} × ${preset.height}`,
          }))}
          onValueChange={pickSize}
        />
        {onPool === null && (
          <Button
            size="sm"
            tooltip="Open this page's DevTools beside it. You drive the browser while they are open."
            onPress={inspect}
          >
            Inspect
          </Button>
        )}
        {driving && (
          <Button
            size="sm"
            variant="primary"
            tooltip={`Hand the browser back to the agent (${handsBackSaid(mac)})`}
            onPress={handBack}
          >
            Done
          </Button>
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
        onKeyDownCapture={leavesTheSurface}
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

/** What a browser the person opened held back, and the answers its sites ask offers (#2176). */
type SitesAsked = { address: string; sites: string[]; answers: Offered[] };

/** Keep blocked, where the core offered no answer that refuses. */
const KEEP_SAID = "Keep blocked";

/** Where a browser's page is and on which device, as its view says them above its page
 *  (#2024, #2300): its origin alone, never its path or title. A browser on a pool's runner names
 *  its pool, so its `localhost` is never taken for this machine's (#2050). */
function BrowserChip({ origin, device }: { origin: string | null; device: string | null }) {
  const chip = addressChip(origin, device);
  return (
    <Tooltip tip={chip.whole ?? undefined}>
      <span className="browser-chip" data-testid="browser-chip">
        {chip.said}
      </span>
    </Tooltip>
  );
}
