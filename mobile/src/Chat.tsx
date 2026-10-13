import { useCallback, useEffect, useRef, useState } from "react";

import { AskSheet } from "./AskSheet.tsx";
import { Composer } from "./Composer.tsx";
import { Notice } from "./Notice.tsx";
// `./timeline.ts` and `./Timeline.tsx` differ only in case: the extensions keep a
// case-insensitive disk from handing one back for the other.
import { Timeline } from "./Timeline.tsx";
import {
  commands,
  on,
  said,
  type Chat as ChatRow,
  type DesktopRow,
  type StreamEnded,
  type TimelineEvent,
  type ViewEvent,
} from "./ipc.ts";
import { QUICK_KEYS, bytesOf } from "./quickKeys.ts";
import { feed, openTerminal, type Mirror, type Opener } from "./terminal.ts";
import { dismissed, empty, reduce, type Story } from "./timeline.ts";

/** A chat's level: a host older than 1.1 says none, and such a chat is a terminal (§5.6). */
export function levelOf(chat: ChatRow): number {
  return chat.level ?? 1;
}

/** A stream the desktop ended sooner than this was open for no reason the phone can mend:
 *  it is not attached again on its own, and Reload is offered instead. */
export const REATTACH_AFTER_MS = 5_000;

/** The attaches and detaches of one chat, in the order they were asked: a Back and a reopen,
 *  or StrictMode's double mount, never race on the desktop. */
const inOrder = new Map<string, Promise<unknown>>();

function queued<T>(key: string, work: () => Promise<T>): Promise<T> {
  const previous = inOrder.get(key) ?? Promise.resolve();
  const mine = previous.then(work, work);
  inOrder.set(
    key,
    mine.catch(() => {}),
  );
  return mine;
}

/**
 * The Chat screen (iOS design §6.1): the timeline strip over an xterm mirror at the desktop's
 * size, the quick keys, the composer and Stop; a shell tab gets the mirror and the keys. The
 * view is attached once per link: a new link (§6.2), or a stream the desktop ended, attaches
 * it again and draws it afresh.
 */
export function Chat({
  desktop,
  chat,
  onBack,
  onRetry,
  open = openTerminal,
}: {
  desktop: DesktopRow;
  chat: ChatRow;
  onBack: () => void;
  onRetry: () => void;
  open?: Opener;
}) {
  const device = desktop.device;
  const id = chat.chat;
  const level = levelOf(chat);
  const connected = desktop.standing === "connected";
  const link = desktop.generation;
  const holder = useRef<HTMLDivElement>(null);
  const mirror = useRef<Mirror | null>(null);
  const [story, setStory] = useState<Story>(empty);
  const [trouble, setTrouble] = useState<string | null>(null);
  const [ended, setEnded] = useState(false);
  const [again, setAgain] = useState(0);
  const [ending, setEnding] = useState(false);

  const write = useCallback(
    (bytes: number[]) => {
      commands.write(device, id, bytes).catch((err) => setTrouble(said(err)));
    },
    [device, id],
  );

  // The view, attached once per link: listeners first, so the snapshot the attach emits is
  // never missed; chunks for a view not yet known wait for the attach to say which.
  useEffect(() => {
    if (!connected || !holder.current) return;
    let gone = false;
    let view: number | null = null;
    let openedAt = 0;
    const waiting: ViewEvent[] = [];
    const key = `${device}/${id}`;
    const listening = Promise.all([
      on<ViewEvent>("view", draw),
      on<StreamEnded>("view-ended", (over) => {
        if (gone || over.device !== device || over.chat !== id || over.number !== view) return;
        if (Date.now() - openedAt >= REATTACH_AFTER_MS) setAgain((n) => n + 1);
        else setEnded(true);
      }),
    ]);
    function draw(event: ViewEvent) {
      if (gone || event.device !== device || event.chat !== id) return;
      if (view === null) {
        waiting.push(event);
        return;
      }
      const drawn = view;
      if (event.view !== drawn || !mirror.current) return;
      feed(mirror.current, event.b64, (bytes) => {
        if (!gone) commands.ack(device, id, drawn, bytes).catch(() => {});
      });
    }
    const at = holder.current;
    listening
      .then(() => queued(key, () => commands.attach(device, id)))
      .then((attached) => {
        if (gone) return;
        mirror.current?.dispose();
        at.replaceChildren();
        const opened = open(at, attached.cols, attached.rows);
        opened.onData((data) => write(bytesOf(data)));
        mirror.current = opened;
        view = attached.view;
        openedAt = Date.now();
        for (const event of waiting.splice(0)) draw(event);
        setTrouble(null);
        setEnded(false);
      })
      .catch((err) => {
        if (!gone) setTrouble(said(err));
      });
    return () => {
      gone = true;
      listening.then((offs) => offs.forEach((off) => off()));
      queued(key, () => commands.detach(device, id)).catch(() => {});
    };
  }, [connected, link, again, device, id, open, write]);

  // The timeline, for a chat that has one, read from its start on every attach.
  useEffect(() => {
    if (!connected || level < 2) return;
    let gone = false;
    let timeline: number | null = null;
    let openedAt = 0;
    const waiting: TimelineEvent[] = [];
    const key = `${device}/${id}/timeline`;
    const take = (event: TimelineEvent) => {
      if (gone || event.device !== device || event.chat !== id) return;
      if (timeline === null) {
        waiting.push(event);
        return;
      }
      if (event.timeline !== timeline) return;
      setStory((was) => reduce(was, event.item));
    };
    const listening = Promise.all([
      on<TimelineEvent>("timeline", take),
      on<StreamEnded>("timeline-ended", (over) => {
        if (gone || over.device !== device || over.chat !== id || over.number !== timeline) {
          return;
        }
        if (Date.now() - openedAt >= REATTACH_AFTER_MS) setAgain((n) => n + 1);
        else setEnded(true);
      }),
    ]);
    setStory(empty());
    listening
      .then(() => queued(key, () => commands.attachTimeline(device, id)))
      .then((attached) => {
        if (gone) return;
        timeline = attached.timeline;
        openedAt = Date.now();
        for (const event of waiting.splice(0)) take(event);
      })
      .catch((err) => {
        if (!gone) setTrouble(said(err));
      });
    return () => {
      gone = true;
      listening.then((offs) => offs.forEach((off) => off()));
      queued(key, () => commands.detachTimeline(device, id)).catch(() => {});
    };
  }, [connected, link, again, device, id, level]);

  // Typing into the mirror does nothing while the desktop is away.
  useEffect(() => {
    mirror.current?.stdin(connected);
  }, [connected]);

  useEffect(
    () => () => {
      mirror.current?.dispose();
      mirror.current = null;
    },
    [],
  );

  const openAsk = story.asks.length > 0 ? story.asks[story.asks.length - 1] : null;

  return (
    <main className={`screen chat level-${level}`}>
      <header className="bar">
        <button type="button" className="back" onClick={onBack}>
          Back
        </button>
        <h1>{chat.title ?? chat.chat.slice(-6)}</h1>
        {level >= 2 &&
          (ending ? (
            <span className="ending">
              End this chat?
              <button
                type="button"
                className="danger"
                disabled={!connected}
                onClick={() => {
                  setEnding(false);
                  commands.stop(device, id).catch((err) => setTrouble(said(err)));
                }}
              >
                End
              </button>
              <button type="button" onClick={() => setEnding(false)}>
                Keep
              </button>
            </span>
          ) : (
            <button
              type="button"
              className="danger"
              disabled={!connected}
              onClick={() => setEnding(true)}
            >
              Stop
            </button>
          ))}
      </header>
      <Notice desktop={desktop} onRetry={onRetry} />
      {trouble && (
        <p className="refused" role="alert">
          {trouble}
        </p>
      )}
      {ended && (
        <p className="standing" role="status">
          The desktop closed this view.
          <button type="button" onClick={() => setAgain((n) => n + 1)}>
            Reload
          </button>
        </p>
      )}
      {level >= 2 && <Timeline rows={story.rows} />}
      <div className="mirror" ref={holder} aria-label="Terminal" />
      <div className="quick-keys" role="toolbar" aria-label="Quick keys">
        {QUICK_KEYS.map((key) => (
          <button
            key={key.label}
            type="button"
            disabled={!connected}
            onClick={() => write(key.bytes)}
          >
            {key.label}
          </button>
        ))}
      </div>
      {level >= 2 && (
        <Composer
          connected={connected}
          onSend={(text) =>
            commands.prompt(device, id, text).catch((err) => {
              setTrouble(said(err));
              throw err;
            })
          }
        />
      )}
      {level >= 2 && (
        <AskSheet
          ask={openAsk}
          closed={story.closed}
          connected={connected}
          onAnswer={(ask, choice) => commands.answer(device, id, ask, choice)}
          onDismiss={() => setStory(dismissed)}
        />
      )}
    </main>
  );
}
