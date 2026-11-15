// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useState } from "react";
import { listen } from "./here";
import { commands, type Failing, type NotInTheBackground } from "./bindings";
import { Notice } from "./Notice";

/** The event the core sends when the session host's breaker trips, and when it is tried again. */
export const FAILING = "session-host-failing";

/** The event the core sends when the system would not run the session host in the background. */
export const NOT_IN_THE_BACKGROUND = "session-host-not-in-the-background";

/**
 * **The session host is failing** (#2007, ADR 0068 §3): the line at the top of the window once
 * the window's supervisor has stopped starting its session host again.
 *
 * purlis starts its session host again after each unexpected exit, until three come within five
 * minutes; then it stops, so a host that cannot run does not run in a loop. Until it runs, no
 * chat starts, and each start says why. The line says so, with the last lines the host wrote to
 * its log folded away under it, and offers **Try again**, which starts it once more with the count started over.
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
          <details>
            <summary>Its last log lines</summary>
            <pre className="ui-text-code">{failing.said.join("\n")}</pre>
          </details>
        ) : undefined
      }
    >
      {failing.stopped_by === "system" ? (
        <>
          Your system stopped starting purlis's session host after it stopped unexpectedly{" "}
          {failing.exits} times in {failing.within_minutes} minutes. No chat starts until it runs.
        </>
      ) : (
        <>
          purlis's session host stopped unexpectedly {failing.exits} times in{" "}
          {failing.within_minutes} minutes, so purlis stopped starting it again. No chat starts
          until it runs.
        </>
      )}
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

/**
 * **purlis can't run in the background** (FD-17): something keeps purlis running after a quit,
 * but the system would not start it at login (its background item switched off, or the
 * system's service manager failed), so the window runs the session host as before. The line
 * says what the person loses and what to check, in plain words, with what the system said
 * folded away under Details. Dismiss puts it away until the next launch.
 */
export function NotInTheBackgroundNotice() {
  const [said, setSaid] = useState<NotInTheBackground | null>(null);
  const [dismissed, setDismissed] = useState(false);

  useEffect(() => {
    let gone = false;
    void commands
      .sessionHostNotInTheBackground()
      .then((now) => {
        if (!gone && isNotInTheBackground(now)) setSaid((was) => was ?? now);
      })
      .catch(() => undefined);
    const listening = listen<NotInTheBackground>(NOT_IN_THE_BACKGROUND, (event) => {
      if (!gone && isNotInTheBackground(event.payload)) setSaid(event.payload);
    }).catch(() => undefined);
    return () => {
      gone = true;
      void listening.then((stop) => stop?.()).catch(() => undefined);
    };
  }, []);

  if (!said || dismissed) return null;
  return (
    <Notice
      cause="session-host-not-in-the-background"
      tone="news"
      label="Not running in the background"
      onDismiss={() => setDismissed(true)}
      under={
        <details>
          <summary>Details</summary>
          <pre className="ui-text-code">{said.why}</pre>
        </details>
      }
    >
      purlis couldn't set itself up to start when you log in, so your chats keep running only until
      you log out or restart. Check that purlis is allowed to run in the background in your system
      settings (on a Mac: System Settings, General, Login Items). purlis tries again the next time
      it starts.
    </Notice>
  );
}

/** Whether what the core answered says why the host is not run in the background. */
function isNotInTheBackground(said: unknown): said is NotInTheBackground {
  return (
    typeof said === "object" &&
    said !== null &&
    typeof (said as NotInTheBackground).why === "string"
  );
}
