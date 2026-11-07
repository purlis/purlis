// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useId, useRef, useState } from "react";
import { Notice } from "./Notice";
import { letRunnerGo } from "./pools";
import { AlertDialog } from "./ui/dialog/Dialog";

/**
 * **An older runner purlis keeps for work it can't bring back** (the person's ruling of
 * 2026-11-07): a draining runner of a pool that holds work no fetch brings here, such as a stash,
 * a commit on no branch or a clone only there. The Notice says so and offers Let it go…, which
 * names in plain words what is lost and asks before anything goes. Only that runner goes: its
 * pool, and the pool's runner now, are untouched. Once it is gone `onDone` is told; where it did
 * not go, as where it holds more now than the ask named, the runner stays, why is said under the
 * line, and `onNotGone` is told so what it holds now is read again.
 */
export function KeptRunnerNotice({
  runner,
  pool,
  left,
  onDone,
  onNotGone,
}: {
  runner: string;
  pool: string;
  /** What it holds that no fetch brings, a sentence each, as the drain's last look found it. */
  left: readonly string[];
  onDone: () => void;
  /** Told where it did not go, so what it holds now can be read again and shown. */
  onNotGone?: () => void;
}) {
  const [asking, setAsking] = useState(false);
  const [going, setGoing] = useState(false);
  const [said, setSaid] = useState<string | undefined>();
  // Where the keyboard goes back to once the ask is put away (WCAG 2.4.3).
  const letGo = useRef<HTMLButtonElement>(null);
  const asked = useRef(false);
  useEffect(() => {
    if (asked.current && !asking && !going) {
      asked.current = false;
      letGo.current?.focus();
    }
  }, [asking, going]);
  const go = () => {
    setAsking(false);
    setGoing(true);
    setSaid(undefined);
    void letRunnerGo(runner, left).then((why) => {
      setGoing(false);
      if (why === undefined) return onDone();
      setSaid(why);
      onNotGone?.();
    });
  };
  return (
    <>
      <Notice
        cause={`devices-kept-runner:${runner}`}
        tone="attention"
        fixes={[
          {
            label: going ? "Letting it go…" : "Let it go…",
            onPress: () => {
              asked.current = true;
              setAsking(true);
            },
            busy: going,
            button: letGo,
          },
        ]}
      >
        {`purlis keeps older runner ${runner} of pool ${pool}, since it holds work purlis can't bring back. To keep that work, copy it out of the runner's container with Docker. Or let the runner go with it.`}
      </Notice>
      {said !== undefined && (
        <Notice
          cause={`devices-kept-runner-not-gone:${runner}`}
          tone="trouble"
          onDismiss={() => setSaid(undefined)}
        >
          {`Runner ${runner} was not let go: `}
          <bdi>{said}</bdi>
        </Notice>
      )}
      {asking && (
        <LetRunnerGo
          runner={runner}
          pool={pool}
          left={left}
          onCancel={() => setAsking(false)}
          onGo={go}
        />
      )}
    </>
  );
}

/**
 * **Let runner {runner} go?**, asked before a kept runner goes with the work it holds: what is
 * lost, a line each, and that the pool and its runner now stay. It opens on Cancel.
 */
export function LetRunnerGo({
  runner,
  pool,
  left,
  onGo,
  onCancel,
}: {
  runner: string;
  pool: string;
  left: readonly string[];
  onGo: () => void;
  onCancel: () => void;
}) {
  const lead = useId();
  return (
    <AlertDialog
      onClose={onCancel}
      title={`Let runner ${runner} go?`}
      description={
        <>
          <p className="came-back pool-named-lead" id={lead}>
            Its container and home are removed from your Docker, and this work is lost with them:
          </p>
          <ul className="came-back pool-named-by" aria-labelledby={lead}>
            {left.map((one) => (
              <li key={one}>
                <bdi>{one}</bdi>
              </li>
            ))}
          </ul>
          <p className="came-back">
            {`Pool ${pool} and its runner now stay as they are. There is no undo.`}
          </p>
        </>
      }
      main={{ children: "Let it go", variant: "danger", onPress: onGo }}
    />
  );
}
