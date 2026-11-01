// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useState } from "react";
import { commands, POOL_MINTS_WITHOUT_ASKING } from "./bindings";
import { listen } from "./here";
import { Notice } from "./Notice";

/**
 * **A cluster pool began minting without asking** (#4442, the person's ruling of 2026-11-01): one
 * Notice for that change, saying what changed and who controls it. It holds and refuses nothing,
 * as the admin's opt-in stands. The core keeps it due until it is dismissed, so a window opened
 * later still says it, and a change back and forth says it again.
 */
export function PoolMintingNotice({ onOpenPools }: { onOpenPools: () => void }) {
  const [pools, setPools] = useState<string[]>([]);

  useEffect(() => {
    let gone = false;
    const read = () =>
      void commands
        .poolsMintingWithoutAsking()
        .then((answer) => {
          // A core that answers nothing has no Notice to draw.
          if (!gone && answer.status === "ok") setPools(answer.data ?? []);
        })
        .catch(() => undefined);
    read();
    const listening = listen(POOL_MINTS_WITHOUT_ASKING, () => {
      if (!gone) read();
    }).catch(() => undefined);
    return () => {
      gone = true;
      void listening.then((stop) => stop?.()).catch(() => undefined);
    };
  }, []);

  const dismiss = (pool: string) => {
    setPools((now) => now.filter((one) => one !== pool));
    void commands.putAwayPoolMintingNotice(pool).catch(() => undefined);
  };

  return (
    <>
      {pools.map((pool) => (
        <Notice
          key={pool}
          cause={`pool-mints-without-asking:${pool}`}
          tone="news"
          label={`Pool ${pool} stopped asking`}
          link={{ label: "Open Pools settings", onPress: onOpenPools }}
          onDismiss={() => dismiss(pool)}
        >
          Chats on pool {pool} now get GitHub App tokens from the cluster without asking you first,
          and its operator logs each one. You are asked only where the cluster refuses. The
          cluster&apos;s admin turned this on, and the admin controls this setting.
        </Notice>
      ))}
    </>
  );
}
