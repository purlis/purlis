// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useState } from "react";
import { LoaderCircle } from "lucide-react";
import { commands, type LandQuestion, type PlaneId, type PushQuestion } from "./bindings";
import { Choice, SettingRow } from "./settings/components";
import { AlertDialog, type DialogProps } from "./ui/dialog/Dialog";
import { Tooltip } from "./ui/tooltip/Tooltip";

/**
 * **A cross-repo change's Push and Land, from its changes view** (#474, ADR 0060).
 *
 * Both ask first, and both ask the core twice: once for what would happen, which this names in
 * full before anything outward-facing runs, and once to do it, handing back exactly what was
 * named. The core refuses the second when the first is no longer true, so what the operator
 * said yes to is all that can happen. Every gate and every refusal's wording is the core's,
 * the same as `charter change push` and `charter change land`.
 *
 * Words: a "branch", a "repo", and the request as its forge calls it, a pull request or a merge
 * request.
 */

/** What a command answered: its lines, or its refusal in the core's words. */
type Said<T> = { ok: T } | { refused: string };

async function asked<T>(
  call: () => Promise<{ status: "ok"; data: T } | { status: "error"; error: string }>,
): Promise<Said<T>> {
  try {
    const said = await call();
    return said.status === "ok" ? { ok: said.data } : { refused: said.error };
  } catch (err: unknown) {
    return { refused: String(err) };
  }
}

/** Ask what Land would do to `repo` of `change`: the core's gates, and nothing merged. */
export function askLand(
  plane: PlaneId,
  workspace: string,
  change: string,
  repo: string,
): Promise<Said<LandQuestion>> {
  return asked(() => commands.changeLandQuestion(plane, workspace, change, repo));
}

/** The answers a question ends with: Cancel, which has the focus, then the act at the edge.
 *  The act stays in its place once it has run, done, and Cancel says Close. */
function doing({
  act,
  running,
  done,
  disabled,
  onAct,
}: {
  act: string;
  running: boolean;
  done: boolean;
  disabled?: boolean;
  onAct: () => void;
}): Pick<DialogProps, "cancel" | "main"> {
  return {
    cancel: {
      children: done ? "Close" : "Cancel",
      reserve: ["Cancel", "Close"],
      isDisabled: running,
    },
    main: {
      children: act,
      icon: running ? (
        <LoaderCircle className="node-icon spinning" aria-hidden="true" />
      ) : undefined,
      variant: "danger",
      isDisabled: running || disabled || done,
      "aria-busy": running,
      onPress: onAct,
    },
  };
}

/** What an act came to: the core's lines, or its refusal. */
function Outcome({ lines, refused }: { lines?: string[]; refused?: string }) {
  return (
    <>
      {refused !== undefined && (
        <p className="trouble" role="alert">
          {refused}
        </p>
      )}
      {lines !== undefined && (
        <ul className="came-back" aria-label="What happened">
          {lines.map((line, at) => (
            <li key={at}>{line}</li>
          ))}
        </ul>
      )}
    </>
  );
}

/**
 * The question before a Push: each member's repo, branch and destination, and where its
 * request goes, read from its folder with nothing pushed. Push hands that list back. Drawn as
 * Save all's confirmation is (ADR 0060 D4): an `AlertDialog`, focus on Cancel.
 */
export function PushAsk({
  plane,
  workspace,
  change,
  onClose,
}: {
  plane: PlaneId;
  workspace: string;
  change: string;
  /** Closed; `acted` when anything was pushed, so the view is read again. */
  onClose: (acted: boolean) => void;
}) {
  const [question, setQuestion] = useState<Said<PushQuestion>>();
  const [running, setRunning] = useState(false);
  const [ran, setRan] = useState<Said<string[]>>();

  // Bumped when a push was refused: what it would push now is asked again, so the list on
  // screen is always the one the next press hands back.
  const [round, setRound] = useState(0);

  useEffect(() => {
    let gone = false;
    void asked(() => commands.changePushQuestion(plane, workspace, change)).then((said) => {
      if (!gone) setQuestion(said);
    });
    return () => {
      gone = true;
    };
  }, [plane, workspace, change, round]);

  const push = (destinations: PushQuestion["destinations"]) => {
    if (running) return;
    setRunning(true);
    void asked(() => commands.changePush(plane, workspace, change, destinations)).then((said) => {
      setRan(said);
      setRunning(false);
      if ("refused" in said) {
        setQuestion(undefined);
        setRound((n) => n + 1);
      }
    });
  };

  const done = ran !== undefined && "ok" in ran;
  return (
    <AlertDialog
      onClose={() => {
        if (!running) onClose(ran !== undefined);
      }}
      title={`Push ${change}?`}
      description={
        question === undefined ? (
          <p className="pending" aria-busy="true">
            <LoaderCircle className="node-icon spinning" aria-hidden="true" />
            Reading where each branch goes…
          </p>
        ) : "refused" in question ? (
          <p className="trouble" role="alert">
            {question.refused}
          </p>
        ) : (
          <p className="came-back">
            {question.ok.destinations.length === 0
              ? "No member of this change can be pushed."
              : "Each branch is pushed to its own name, and only fast-forwarded: nothing is committed and nothing is forced. Then each request is opened or found, and its description gets the change's links."}
          </p>
        )
      }
      {...doing({
        act: "Push",
        running,
        done,
        disabled:
          question === undefined || "refused" in question || question.ok.destinations.length === 0,
        onAct: () => {
          if (question !== undefined && "ok" in question) push(question.ok.destinations);
        },
      })}
    >
      {question !== undefined && "ok" in question && (
        <>
          {question.ok.destinations.length > 0 && (
            <ul className="saving-files" aria-label="What is pushed">
              {question.ok.destinations.map((d) => (
                <li key={d.repo}>
                  <strong>{d.repo}</strong>: branch {d.branch} at {d.head_short} → {d.to}
                  {d.base !== null && `, its ${d.request} into ${d.base}`}
                </li>
              ))}
            </ul>
          )}
          {question.ok.not_pushed.map((line, at) => (
            <p key={at} className="trouble" role="alert">
              {line}
            </p>
          ))}
        </>
      )}
      {ran !== undefined && (
        <Outcome
          lines={"ok" in ran ? ran.ok : undefined}
          refused={"refused" in ran ? ran.refused : undefined}
        />
      )}
    </AlertDialog>
  );
}

/**
 * The question before a Land, once its gates have passed: the request, the head its checks
 * passed at, and whether charter merges it now or puts it in the forge's queue. Land hands
 * that back, and the core refuses it when any of it has changed.
 */
export function LandAsk({
  plane,
  workspace,
  change,
  question,
  onClose,
}: {
  plane: PlaneId;
  workspace: string;
  change: string;
  question: LandQuestion;
  /** Closed; `acted` when the landing ran, so the view is read again. */
  onClose: (acted: boolean) => void;
}) {
  const [running, setRunning] = useState(false);
  const [ran, setRan] = useState<Said<string[]>>();
  const [squash, setSquash] = useState(false);
  // What is on screen, and what the next press hands back: the question this opened with,
  // or, after a landing was refused, the core's answer asked again — `undefined` while that
  // is asked, and a refusal when its gates now refuse.
  const [now, setNow] = useState<Said<LandQuestion> | undefined>({ ok: question });
  const q = now !== undefined && "ok" in now ? now.ok : question;
  const askable = now !== undefined && "ok" in now;
  const done = ran !== undefined && "ok" in ran;

  const land = () => {
    if (running || !askable) return;
    setRunning(true);
    void asked(() => commands.changeLand(plane, workspace, change, q, squash && q.squash)).then(
      (said) => {
        setRan(said);
        setRunning(false);
        if ("refused" in said) {
          setNow(undefined);
          void askLand(plane, workspace, change, q.repo).then(setNow);
        }
      },
    );
  };

  return (
    <AlertDialog
      onClose={() => {
        if (!running) onClose(ran !== undefined);
      }}
      title={
        <>
          {q.through === "record" ? "Record" : "Land"} {q.repo}&apos;s {q.request} {q.sigil}
          {q.number}?
        </>
      }
      description={<p className="came-back">{q.how}</p>}
      {...doing({
        act: q.through === "record" ? "Record" : "Land",
        running,
        done,
        disabled: !askable,
        onAct: land,
      })}
    >
      {now === undefined && (
        <p className="pending" aria-busy="true">
          <LoaderCircle className="node-icon spinning" aria-hidden="true" />
          Checking it again…
        </p>
      )}
      {now !== undefined && "refused" in now && (
        <p className="trouble" role="alert">
          {now.refused}
        </p>
      )}
      <dl className="facts">
        <dt>change</dt>
        <dd>{change}</dd>
        <dt>{q.request}</dt>
        <dd>{q.url}</dd>
        <dt>head</dt>
        <dd>
          <Tooltip tip={q.head}>
            <code>{q.head}</code>
          </Tooltip>
        </dd>
        <dt>how</dt>
        <dd>
          {q.through === "queue"
            ? `into the ${q.queue}`
            : q.through === "record"
              ? "recorded only"
              : `merged now, at ${q.head_short}`}
        </dd>
      </dl>
      {q.said.map((line, at) => (
        <p key={at} className="note">
          {line}
        </p>
      ))}
      {q.squash && !done && (
        <SettingRow
          label="Squash it into one commit"
          control={(ids) => (
            <Choice
              ids={ids}
              kind="toggle"
              checked={squash}
              disabled={running}
              onCheckedChange={setSquash}
            />
          )}
        />
      )}
      {ran !== undefined && (
        <Outcome
          lines={"ok" in ran ? ran.ok : undefined}
          refused={"refused" in ran ? ran.refused : undefined}
        />
      )}
    </AlertDialog>
  );
}
