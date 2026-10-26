// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { History } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { commands, type TimelineMoment, type TimelineMoved } from "./bindings";
import { frameBytes, paint } from "./browserFrames";
import { TIMELINE_MOVED, atSaid, unkeptSaid } from "./browserTimelines";
import { listen } from "./here";
import { BrowserProof } from "./BrowserProof";
import { Button, ButtonRow } from "./ui/button/Button";
import { Checkbox } from "./ui/choice/Checkbox";
import { EmptyState } from "./ui/empty-state/EmptyState";

/** What a timeline reads as: nothing yet, its moments, or why it could not be read. */
type Read =
  | { kind: "reading" }
  | { kind: "read"; moments: TimelineMoment[] }
  | { kind: "refused"; why: string };

/**
 * **A chat's browser timeline** (#2017, spec #2008): what its agent did in its browsers, the
 * newest first, each with the screenshot its tool returned where one is kept, and where the
 * person or purlis drove. Read again each time the core says it moved. A screenshot is drawn on
 * a canvas from its bytes, as a browser's frames are, so the window's content policy is
 * unchanged.
 */
export function BrowserTimeline({ plane, chat }: { plane: string; chat: string }) {
  const [read, setRead] = useState<Read>({ kind: "reading" });
  // The screenshots the person picked for browser proof (#2026): none until they pick, and
  // never more than the timeline still keeps.
  const [picked, setPicked] = useState<ReadonlySet<number>>(new Set());
  const [attaching, setAttaching] = useState(false);
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
  const kept = new Set(moments.flatMap((moment) => (moment.shot === null ? [] : [moment.shot])));
  const chosen = moments.flatMap((moment) =>
    moment.shot !== null && picked.has(moment.shot) ? [moment.shot] : [],
  );
  const pick = (shot: number, on: boolean) =>
    setPicked((was) => {
      const now = new Set([...was].filter((one) => kept.has(one)));
      if (on) now.add(shot);
      else now.delete(shot);
      return now;
    });
  const didOf = (shot: number) => moments.find((moment) => moment.shot === shot)?.did ?? "";
  return (
    <div className="browser-timeline-pane">
      {kept.size > 0 &&
        (attaching ? (
          <BrowserProof
            plane={plane}
            chat={chat}
            picked={chosen}
            drawn={(shot) => <Shot plane={plane} chat={chat} shot={shot} did={didOf(shot)} />}
            onDone={() => {
              setAttaching(false);
              setPicked(new Set());
            }}
          />
        ) : (
          <div className="browser-proof-bar" data-testid="browser-proof-bar">
            <p className="ui-text-meta">
              {chosen.length === 0
                ? "Pick screenshots to attach to a pull request as proof."
                : chosen.length === 1
                  ? "1 screenshot picked."
                  : `${chosen.length} screenshots picked.`}
            </p>
            <ButtonRow size="sm">
              <Button
                aria-disabled={chosen.length === 0 ? true : undefined}
                tooltip={
                  chosen.length === 0
                    ? "Pick the screenshots first: nothing is attached that you did not pick."
                    : undefined
                }
                onPress={() => {
                  if (chosen.length > 0) setAttaching(true);
                }}
              >
                Attach browser proof
              </Button>
            </ButtonRow>
          </div>
        ))}
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
            {moment.shot !== null && (
              <>
                <Shot plane={plane} chat={chat} shot={moment.shot} did={moment.did} />
                {!attaching && (
                  <Checkbox
                    isSelected={picked.has(moment.shot)}
                    onChange={(on) => {
                      if (moment.shot !== null) pick(moment.shot, on);
                    }}
                  >
                    Pick for browser proof
                  </Checkbox>
                )}
              </>
            )}
            {unkeptSaid(moment) !== null && (
              <p className="browser-timeline-unkept ui-text-meta">{unkeptSaid(moment)}</p>
            )}
          </li>
        ))}
      </ol>
    </div>
  );
}

/** What a screenshot that could not be drawn says in its place. */
const NOT_DRAWN = "This screenshot could not be drawn.";

/** One screenshot of a timeline, read once and drawn on a canvas. */
function Shot({
  plane,
  chat,
  shot,
  did,
}: {
  plane: string;
  chat: string;
  shot: number;
  did: string;
}) {
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
      // A picture that does not decode says so, where it would have been.
      .catch(() => {
        if (!left) setGone(NOT_DRAWN);
      });
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
