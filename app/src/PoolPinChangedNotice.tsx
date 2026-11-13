// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useState } from "react";
import { commands, POOL_PIN_CHANGED, type PoolPinChanged } from "./bindings";
import { listen } from "./here";
import { Notice } from "./Notice";
import { removePool } from "./pools";
import { RemovePool } from "./settings/pools";

/**
 * **A pool on another machine whose pin no longer holds** (#6270, ADR 0123 §3): the machine
 * shows another host key, its CA file holds another CA, or its engine answers with another id.
 * purlis then starts no runner there, opens no new link and sends no vault value; a link already
 * open stays open. One Notice for each pool (cause `pool-pin-changed:<pool>`), in the core's words,
 * the same as `purlis pool doctor`'s: the pool, what changed, both fingerprints and the usual
 * reason.
 *
 * Its two ways out are ADR 0123's: **Check and trust again**, which opens Pool setup on the pool
 * at Connect, where the machine is met again and only what the person confirms is pinned; and
 * **Remove pool**, asked first, which forgets the pool here and sends nothing to the machine, so
 * its runner stays there, and then says what stays and how to remove it on that machine.
 *
 * The core keeps what it found, contacting no machine for this Notice: each change is found where
 * a run for the pool was refused, and goes once its pin holds again or the pool is removed.
 */
export function PoolPinChangedNotice({
  onCheckAndTrust,
}: {
  /** Opens Pool setup on pool `pool`, at Connect. */
  onCheckAndTrust: (pool: string) => void;
}) {
  const [changes, setChanges] = useState<readonly PoolPinChanged[]>([]);
  const [asking, setAsking] = useState<PoolPinChanged>();
  const [forgotten, setForgotten] = useState<{ pool: string; left: string }>();

  useEffect(() => {
    let gone = false;
    const read = () =>
      void commands
        .poolsPinChanged()
        .then((answer) => {
          // A core that answers nothing has no Notice to draw.
          if (!gone && answer.status === "ok") setChanges(answer.data ?? []);
        })
        .catch(() => undefined);
    read();
    const listening = listen(POOL_PIN_CHANGED, () => {
      if (!gone) read();
    }).catch(() => undefined);
    return () => {
      gone = true;
      void listening.then((stop) => stop?.()).catch(() => undefined);
    };
  }, []);

  const remove = (change: PoolPinChanged) => {
    setAsking(undefined);
    void removePool(change.pool, true).then((left) => {
      setChanges((now) => now.filter((one) => one.pool !== change.pool));
      if (left !== undefined) setForgotten({ pool: change.pool, left });
    });
  };

  return (
    <>
      {changes.map((change) => (
        <Notice
          key={change.pool}
          cause={`pool-pin-changed:${change.pool}`}
          tone="trouble"
          title={change.title}
          fixes={[
            {
              label: "Check and trust again",
              primary: true,
              onPress: () => onCheckAndTrust(change.pool),
            },
            { label: "Remove pool…", onPress: () => setAsking(change) },
          ]}
        >
          {change.said}
        </Notice>
      ))}
      {forgotten && (
        <Notice
          cause={`pool-forgotten:${forgotten.pool}`}
          tone="news"
          onDismiss={() => setForgotten(undefined)}
        >
          {`Pool ${forgotten.pool} is forgotten here. ${forgotten.left}`}
        </Notice>
      )}
      {asking && (
        <RemovePool
          name={asking.pool}
          untrusted={asking.host}
          onRemove={() => remove(asking)}
          onCancel={() => setAsking(undefined)}
        />
      )}
    </>
  );
}
