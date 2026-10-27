// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useCallback, useEffect, useState } from "react";
import { OctagonX, Play } from "lucide-react";
import { commands } from "./bindings";
import { listen } from "./here";
import { StatusItem } from "./ui/status/StatusItem";
import { Tooltip } from "./ui/tooltip/Tooltip";

/**
 * **The kill switch** (OV-1, ADR 0071): stop every chat and shell charter started — in every
 * project and window — and start no chat until the operator re-arms it. Agents charter did not
 * start, headless ones among them, are not reached (OV-2).
 *
 * On the title bar because it is about the whole app and not one project, and because the
 * title bar is on screen in every window. **One press, with no question in between**: a switch
 * that asks first is not one an operator can reach for. Nothing is lost by a press made by
 * mistake — the chats stay as tabs, each reading as one whose program ended, and re-arming lets
 * the operator reopen them.
 *
 * **What it draws is what the core says**, never what the press hoped for: the core's answer
 * to `agents_stopped` after every press, and every `kill-switch` event. A stop the core could
 * not keep on disk is still a stop in the app, and the bar says so out loud, because the next
 * launch may not know.
 *
 * `charter stop --all` in a terminal throws the same switch; the core hears it and every
 * window's bar is told. Re-arming is here and only here: a command any agent could run cannot be
 * the thing that lets agents start again.
 */
export function KillSwitch() {
  const [stopped, setStopped] = useState(false);
  const [busy, setBusy] = useState(false);
  const [trouble, setTrouble] = useState<string>();
  /** The runners a re-arm re-arms beside this machine (#2051): named before the press. */
  const [runners, setRunners] = useState<readonly string[]>([]);

  const ask = useCallback(
    () =>
      commands
        .agentsStopped()
        .then((said) => {
          if (typeof said === "boolean") setStopped(said);
        })
        // Said, never swallowed (#1719): a switch whose state could not be read would draw
        // as armed, the one thing it must never show of a stopped app.
        .catch((err: unknown) =>
          setTrouble(`purlis could not read whether chats are stopped: ${String(err)}`),
        ),
    [],
  );

  // Read again each time the switch is thrown: which runners its stop stopped is the host's.
  useEffect(() => {
    if (!stopped) return;
    let current = true;
    commands.runnersARearmReaches().then(
      (said) => {
        if (current && said.status === "ok" && Array.isArray(said.data)) setRunners(said.data);
      },
      () => undefined,
    );
    return () => {
      current = false;
    };
  }, [stopped]);

  useEffect(() => {
    let gone = false;
    let stop: (() => void) | undefined;
    void (async () => {
      try {
        const unlisten = await listen<boolean>("kill-switch", (event) => {
          if (!gone) setStopped(event.payload === true);
        });
        if (gone) unlisten();
        else stop = unlisten;
      } catch {
        // No window to listen in: a unit test, or a webview being torn down.
      }
    })();
    void ask();
    return () => {
      gone = true;
      stop?.();
    };
  }, [ask]);

  const press = () => {
    setBusy(true);
    setTrouble(undefined);
    const asked = stopped ? commands.rearmAgents() : commands.stopEveryAgent();
    void asked
      .then((said) => {
        if (said.status === "error") setTrouble(said.error);
      })
      .catch((why: unknown) => setTrouble(String(why)))
      .finally(() => {
        setBusy(false);
        void ask();
      });
  };

  // An armed switch names none: what was read while it was stopped is not shown.
  const reaches = stopped ? runners : [];
  const label = stopped
    ? reaches.length === 0
      ? "Every chat purlis started is stopped — re-arm to let chats start again"
      : `Every chat purlis started is stopped — re-arm to let chats start again here and on ${
          reaches.length === 1 ? "runner" : "runners"
        } ${reaches.join(", ")}`
    : "Stop every chat and shell purlis started, in every project and window";
  return (
    <>
      {/* A status pill (R115), in the danger colours once it is thrown, so a stopped app is
          never mistaken for an idle one. Being a button is what keeps it out of the title bar's
          drag region (`TitleBar.tsx`). */}
      <StatusItem
        variant="pill"
        tone={stopped ? "danger" : undefined}
        data-testid="title-kill-switch"
        data-stopped={stopped ? "yes" : "no"}
        // Named by what it shows (WCAG 2.5.3, the copy guide's "an aria-label says what the
        // visible label says"): the icon alone needs the sentence as its name, but once thrown
        // the words "Stopped · Re-arm" are the name and the sentence is its tooltip, which a
        // screen reader reads as its description (#630).
        aria-label={stopped ? undefined : label}
        tooltip={label}
        isDisabled={busy}
        onPress={press}
        // An icon alone while nothing is stopped: the bar's right-hand end never gives way
        // (ADR 0054), and what it spends the project tabs lose in a 1024 px window. Its name is
        // its `aria-label`, which its tooltip shows. Once thrown it says so in words, because a stopped
        // app must never read as an idle one.
        icon={stopped ? <Play aria-hidden="true" /> : <OctagonX aria-hidden="true" />}
      >
        {stopped ? "Stopped · Re-arm" : undefined}
      </StatusItem>
      {trouble !== undefined && (
        <Tooltip tip={trouble}>
          <span role="alert" className="title-kill-switch-trouble">
            {trouble}
          </span>
        </Tooltip>
      )}
    </>
  );
}
