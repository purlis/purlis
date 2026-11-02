// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useRef, useState } from "react";
import { Notice } from "./Notice";
import { bringBackHeldRunner, removeHeldRunner } from "./pools";
import { DiscardHeldRunner } from "./settings/pools";

/**
 * **A runner purlis kept with its chats' conversations** (#4633, ruled 2026-11-02): an older
 * runner of a pool, stopped or removed outside purlis, whose home holds what its chats left. The
 * Notice offers to bring their conversations back to the pool's runner, where those chats resume,
 * or to discard them, which asks twice. Once either is done the runner is gone and `onDone` is
 * told. Where it did not finish, as where some conversations could not be brought back, the
 * runner stays and what stopped it is said under the line.
 */
export function HeldRunnerNotice({
  runner,
  pool,
  onDone,
}: {
  runner: string;
  pool: string;
  onDone: () => void;
}) {
  const [doing, setDoing] = useState<"bringing" | "discarding" | undefined>();
  const [ask, setAsk] = useState<"first" | "again" | undefined>();
  const [said, setSaid] = useState<{ what: string; why: string } | undefined>();
  // Where the keyboard goes back to once an ask is put away: the second ask opened from the first,
  // which is gone, so nothing else would take it (WCAG 2.4.3).
  const discard = useRef<HTMLButtonElement>(null);
  const asked = useRef(false);
  useEffect(() => {
    if (asked.current && ask === undefined && doing === undefined) {
      asked.current = false;
      discard.current?.focus();
    }
  }, [ask, doing]);
  const run = (what: "bringing" | "discarding") => {
    setAsk(undefined);
    setDoing(what);
    setSaid(undefined);
    const done = what === "bringing" ? bringBackHeldRunner(runner) : removeHeldRunner(runner);
    void done.then((why) => {
      setDoing(undefined);
      if (why === undefined) return onDone();
      setSaid({
        what:
          what === "bringing"
            ? `Bringing back runner ${runner}'s conversations did not finish: `
            : `Discarding runner ${runner}'s conversations did not finish: `,
        why,
      });
    });
  };
  return (
    <>
      <Notice
        cause={`devices-held-runner:${runner}`}
        tone="attention"
        fixes={[
          {
            label: doing === "bringing" ? "Bringing back…" : "Bring back conversations",
            onPress: () => run("bringing"),
            busy: doing !== undefined,
          },
          {
            label: doing === "discarding" ? "Discarding…" : "Discard…",
            onPress: () => {
              asked.current = true;
              setAsk("first");
            },
            busy: doing !== undefined,
            button: discard,
          },
        ]}
      >
        {`purlis kept runner ${runner} of pool ${pool}, with what its chats left in its home. Bring their conversations back to the pool's runner, which then removes this runner and the rest of its home, or discard them.`}
      </Notice>
      {said !== undefined && (
        <Notice
          cause={`devices-held-runner-not-done:${runner}`}
          tone="trouble"
          onDismiss={() => setSaid(undefined)}
        >
          {said.what}
          <bdi>{said.why}</bdi>
        </Notice>
      )}
      {ask !== undefined && (
        <DiscardHeldRunner
          name={runner}
          ask={ask}
          onCancel={() => setAsk(undefined)}
          onGoOn={() => (ask === "first" ? setAsk("again") : run("discarding"))}
        />
      )}
    </>
  );
}
