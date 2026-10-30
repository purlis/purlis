// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useState } from "react";
import type { ChatsReconnecting, PlaneId, PoolRemoved } from "./bindings";
import { listen } from "./here";
import { Notice } from "./Notice";
import { makeRunnerAgain, readRemovedPools, removedSaid, usePools } from "./pools";
import { RECONNECTING } from "./reconnecting";

/**
 * **The runner a project's chats run on was removed outside purlis** (#4149): the Notice a
 * refused start shows, at the window's own level and not on each chat's pane.
 *
 * The person removed the pool's container outside purlis, so Docker has no container of its id.
 * This machine's session host does not link to it again on its own and refuses each start there
 * in plain words; the core says so as `runner_gone` (`chats-reconnecting`). This asks which pools'
 * runners are gone and offers, for each, **Make the runner again**, which makes it from the
 * pool's own image and limits, with the sign-ins of its home where that is still there, and keeps the pool's
 * profiles there again; **Open Pools settings**; and **Dismiss**. It goes once the core says the
 * chats are reconnected, or once each pool's runner is made again.
 */
export function RunnerGoneNotice({
  plane,
  onOpenPools,
}: {
  plane: PlaneId;
  /** Opens Settings at this machine's Pools. */
  onOpenPools: () => void;
}) {
  const [removed, setRemoved] = useState<{ plane: PlaneId; pools: readonly PoolRemoved[] }>();
  // Why Make the runner again was refused, by the pool's name.
  const [said, setSaid] = useState<Readonly<Record<string, string>>>({});
  const { remaking } = usePools();
  useEffect(() => {
    let gone = false;
    let stop: (() => void) | undefined;
    void (async () => {
      try {
        const unlisten = await listen<ChatsReconnecting>(RECONNECTING, (event) => {
          const told = event.payload;
          if (told.plane !== plane) return;
          if (told.reconnection === "runner_gone")
            void readRemovedPools().then((pools) => {
              if (!gone) setRemoved({ plane, pools });
            });
          else if (told.reconnection === "reconnected") setRemoved(undefined);
        });
        if (gone) unlisten();
        else stop = unlisten;
      } catch {
        // No window to listen in: a unit test, or a webview being torn down.
      }
    })();
    return () => {
      gone = true;
      stop?.();
    };
  }, [plane]);
  const pools = removed?.plane === plane ? removed.pools : [];
  // A pool's Notice goes: put away, or its runner made again.
  const putAway = (name: string) =>
    setRemoved((was) =>
      was === undefined ? was : { ...was, pools: was.pools.filter((one) => one.name !== name) },
    );
  const makeAgain = (name: string) =>
    void makeRunnerAgain(name, plane).then((why) => {
      setSaid((was) => {
        const now = { ...was };
        if (why === undefined) Reflect.deleteProperty(now, name);
        else now[name] = why;
        return now;
      });
      if (why === undefined) putAway(name);
    });
  return pools.map((pool) => (
    <Notice
      key={pool.name}
      cause={`runner-gone:${plane}:${pool.name}`}
      tone="trouble"
      label="Runner removed"
      fixes={[
        {
          label: "Make the runner again",
          onPress: () => makeAgain(pool.name),
          busy: remaking === pool.name,
        },
      ]}
      link={{ label: "Open Pools settings", onPress: onOpenPools }}
      onDismiss={() => putAway(pool.name)}
    >
      {removedSaid(pool.name, pool.home_kept)}
      {said[pool.name] !== undefined && <> purlis could not make it again: {said[pool.name]}</>}
    </Notice>
  ));
}
