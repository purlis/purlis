// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useState } from "react";
import { Notice } from "./Notice";
import { bringBackHeldRunner, removeHeldRunner } from "./pools";
import { DiscardHeldRunner } from "./settings/pools";

/**
 * **A runner purlis kept with its chats' conversations** (#4633, ruled 2026-11-02): an older
 * runner of a pool, stopped or removed outside purlis, whose home holds what its chats left. The
 * Notice offers to bring their conversations back to the pool's runner, where those chats resume,
 * or to discard them, which asks twice. Either way, once it is done the runner is gone and
 * `onDone` is told; what stopped it is said under it, and the runner stays as it was.
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
            onPress: () => setAsk("first"),
            busy: doing !== undefined,
          },
        ]}
      >
        {`purlis kept runner ${runner} of pool ${pool}, with what its chats left in its home. Bring their conversations back to the pool's runner to pick those chats up there, or discard them.`}
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
