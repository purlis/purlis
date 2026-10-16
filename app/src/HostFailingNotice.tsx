// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useState } from "react";
import { listen } from "./here";
import { commands, type Failing } from "./bindings";
import { Notice } from "./Notice";

/** The event the core sends when the session host's breaker trips, and when it is tried again. */
export const FAILING = "session-host-failing";

/**
 * **The session host is failing** (#2007, ADR 0068 §3): the line at the top of the window once
 * the window's supervisor has stopped starting its session host again.
 *
 * purlis starts its session host again after each unexpected exit, until three come within five
 * minutes; then it stops, so a host that cannot run does not run in a loop. Until it runs, no
 * chat starts, and each start says why. The line says so, with the last lines the host wrote to
 * its log, and offers **Try again**, which starts it once more with the count started over.
 * It stays until the host is tried again: there is nothing else to do about it.
 */
export function HostFailingNotice() {
  const [failing, setFailing] = useState<Failing | null>(null);
  const [trying, setTrying] = useState(false);

  useEffect(() => {
    let gone = false;
    void commands
      .sessionHostFailing()
      .then((now) => {
        if (!gone && isFailing(now)) setFailing((was) => was ?? now);
      })
      .catch(() => undefined);
    const listening = listen<Failing | null>(FAILING, (event) => {
      if (gone) return;
      setFailing(isFailing(event.payload) ? event.payload : null);
      setTrying(false);
    }).catch(() => undefined);
    return () => {
      gone = true;
      void listening.then((stop) => stop?.()).catch(() => undefined);
    };
  }, []);

  if (!failing) return null;
  const tryAgain = () => {
    setTrying(true);
    void commands
      .startTheSessionHostAgain()
      .then((answer) => {
        if (answer.status === "error") setTrying(false);
      })
      .catch(() => setTrying(false));
  };
  return (
    <Notice
      cause="session-host-failing"
      tone="trouble"
      label="Session host failing"
      fixes={[{ label: "Try again", onPress: tryAgain, busy: trying }]}
      under={
        failing.said.length > 0 ? (
          <pre className="ui-text-code">{failing.said.join("\n")}</pre>
        ) : undefined
      }
    >
      purlis's session host stopped unexpectedly {failing.exits} times in {failing.within_minutes}{" "}
      minutes, so purlis stopped starting it again. No chat starts until it runs.
      {failing.said.length > 0 && " The last lines it wrote to its log are below."}
    </Notice>
  );
}

/** Whether what the core answered is a failing host, as the window draws one. */
function isFailing(said: unknown): said is Failing {
  return (
    typeof said === "object" &&
    said !== null &&
    Array.isArray((said as Failing).said) &&
    typeof (said as Failing).exits === "number"
  );
}
