// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { History } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { commands, type TimelineMoment, type TimelineMoved } from "./bindings";
import { frameBytes, paint } from "./browserFrames";
import { TIMELINE_MOVED, atSaid, unkeptSaid } from "./browserTimelines";
import { listen } from "./here";
import { EmptyState } from "./ui/empty-state/EmptyState";

/** What a timeline reads as: nothing yet, its moments, or why it could not be read. */
type Read = { kind: "reading" } | { kind: "read"; moments: TimelineMoment[] } | { kind: "refused"; why: string };

/**
 * **A chat's browser timeline** (#2017, spec #2008): what its agent did in its browsers, the
 * newest first, each with the screenshot its tool returned where one is kept, and where the
 * person or purlis drove. Read again each time the core says it moved. A screenshot is drawn on
 * a canvas from its bytes, as a browser's frames are, so the window's content policy is
 * unchanged.
 */
export function BrowserTimeline({ plane, chat }: { plane: string; chat: string }) {
  const [read, setRead] = useState<Read>({ kind: "reading" });
  useEffect(() => {
    let gone = false;
    let stop: (() => void) | undefined;
    let asked = 0;
    const again = () => {
      asked += 1;
      const mine = asked;
      void commands
        .chatBrowserTimeline(plane, chat)
        .then((answer) => {
          if (gone || mine !== asked) return;
          setRead(
            answer.status === "error"
              ? { kind: "refused", why: answer.error }
              : { kind: "read", moments: answer.data },
          );
        })
        .catch(() => undefined);
    };
    again();
    void (async () => {
      try {
        const unlisten = await listen<TimelineMoved>(TIMELINE_MOVED, (event) => {
          if (!gone && event.payload.plane === plane && event.payload.chat === chat) again();
        });
        if (gone) unlisten();
        else stop = unlisten;
      } catch {
        // No window to listen in.
      }
    })();
    return () => {
      gone = true;
      stop?.();
    };
  }, [plane, chat]);

  if (read.kind === "refused")
    return (
      <EmptyState
        mark={History}
        headline="This browser timeline cannot be shown"
        body={read.why}
        testid="browser-timeline-refused"
      />
    );
  if (read.kind === "read" && read.moments.length === 0)
    return (
      <EmptyState
        mark={History}
        headline="This chat has done nothing in a browser yet"
        body="What its agent does in its browsers is listed here, with the screenshots it took. It stays on this device until the chat closes."
        testid="browser-timeline-empty"
      />
    );
  const moments = read.kind === "read" ? [...read.moments].reverse() : [];
  return (
    <ol className="browser-timeline" aria-label="Browser timeline" data-testid="browser-timeline">
      {moments.map((moment) => (
        <li
          key={`${moment.at}:${moment.browser}:${moment.did}:${moment.shot ?? ""}`}
          className="browser-timeline-moment"
          data-by={moment.by}
        >
          <p className="browser-timeline-said">
            <span className="ui-text-meta">{atSaid(moment.at)}</span>{" "}
            <span className="ui-text-secondary">Browser {moment.browser}</span>{" "}
            <span>{moment.did}</span>
          </p>
          {moment.page !== null && (
            <p className="browser-timeline-page ui-text-meta">{moment.page}</p>
          )}
          {moment.shot !== null && <Shot plane={plane} chat={chat} shot={moment.shot} did={moment.did} />}
          {unkeptSaid(moment) !== null && (
            <p className="browser-timeline-unkept ui-text-meta">{unkeptSaid(moment)}</p>
          )}
        </li>
      ))}
    </ol>
  );
}

/** One screenshot of a timeline, read once and drawn on a canvas. */
function Shot({ plane, chat, shot, did }: { plane: string; chat: string; shot: number; did: string }) {
  const canvas = useRef<HTMLCanvasElement>(null);
  const [gone, setGone] = useState<string | null>(null);
  useEffect(() => {
    let left = false;
    void commands
      .chatBrowserTimelineShot(plane, chat, shot)
      .then(async (answer) => {
        if (left) return;
        if (answer.status === "error") {
          setGone(answer.error);
          return;
        }
        if (canvas.current !== null)
          await paint(canvas.current, frameBytes(answer.data), undefined, "image/png");
      })
      .catch(() => undefined);
    return () => {
      left = true;
    };
  }, [plane, chat, shot]);
  if (gone !== null) return <p className="browser-timeline-unkept ui-text-meta">{gone}</p>;
  return (
    <canvas
      ref={canvas}
      className="browser-timeline-shot"
      role="img"
      aria-label={`The screenshot the agent took: ${did}`}
      data-testid="browser-timeline-shot"
    />
  );
}
