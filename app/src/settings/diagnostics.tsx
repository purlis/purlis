// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useState } from "react";
import { commands, type DetailedRemoteLog } from "../bindings";
import { Switch } from "../ui/choice/Switch";
import { DIAGNOSTICS_ALSO } from "./catalogue";
import type { LiveSetting, SettingsGroup } from "./groups";

/**
 * **You › Diagnostics** (#7864, decided in #7189): what purlis writes to the diagnostic log on
 * this machine, which nothing reads but a person and nothing sends (ADR 0075).
 *
 * **Detailed remote log for one hour** lets every step of a remote start through to the log, each
 * Docker call among them, and turns itself off an hour later. ssh's own verbose output is never
 * in it. The switch is held by the core in memory (`purlis_core::detailedlog`), so it is off
 * again once purlis starts again.
 */
export const DIAGNOSTICS_GROUP = "you.diagnostics";

/** The row's address. */
export const DETAILED_REMOTE_LOG_SETTING = "you.diagnostics.detailed-remote";

/** What the row's help says. */
export const DETAILED_REMOTE_LOG_SAYS =
  "Writes every step of starting a chat on a pool to this machine's log, each Docker call too, for an hour. ssh's own output is never written. Nothing is sent.";

export function diagnosticsGroup(): SettingsGroup {
  return {
    id: DIAGNOSTICS_GROUP,
    label: "Diagnostics",
    help: "What purlis writes to the diagnostic log on this machine. The log keeps a week and is never sent.",
    also: DIAGNOSTICS_ALSO,
    settings: [detailedRemoteLog],
  };
}

const OFF: DetailedRemoteLog = { until: null };

/** The longest a browser's timer waits: 2³¹ − 1 ms. */
const MOST_TIMER_WAIT = 2_147_483_647;

/** `said` as the core answers it; anything else, as no answer, is off. */
function read(said: unknown): DetailedRemoteLog {
  if (said === null || typeof said !== "object") return OFF;
  const { until } = said as { until?: unknown };
  return typeof until === "string" && !Number.isNaN(new Date(until).getTime()) ? { until } : OFF;
}

/** When `until` is, as the person's clock says it. */
export function untilSaid(until: string): string {
  return new Date(until).toLocaleTimeString(undefined, { hour: "numeric", minute: "2-digit" });
}

/** What the switch says, read from the core, and read again when its hour is up. */
function useDetailedRemoteLog(): [DetailedRemoteLog, (on: boolean) => void] {
  const [said, setSaid] = useState<DetailedRemoteLog>(OFF);
  useEffect(() => {
    let live = true;
    void commands.detailedRemoteLog().then(
      (now) => {
        if (live) setSaid(read(now));
      },
      () => undefined,
    );
    return () => {
      live = false;
    };
  }, []);
  useEffect(() => {
    if (said.until === null) return;
    // Asked again a moment after its hour, when the core has turned it off.
    // Held to what a timer can wait: a longer one fires at once.
    const wait = Math.min(
      Math.max(0, new Date(said.until).getTime() - Date.now()) + 1000,
      MOST_TIMER_WAIT,
    );
    const timer = setTimeout(() => {
      void commands.detailedRemoteLog().then(
        (now) => setSaid(read(now)),
        () => setSaid(OFF),
      );
    }, wait);
    return () => clearTimeout(timer);
  }, [said.until]);
  const turn = (on: boolean) => {
    void commands.setDetailedRemoteLog(on).then(
      (now) => setSaid(read(now)),
      () => undefined,
    );
  };
  return [said, turn];
}

const detailedRemoteLog: LiveSetting = {
  id: DETAILED_REMOTE_LOG_SETTING,
  label: "Detailed remote log for one hour",
  help: DETAILED_REMOTE_LOG_SAYS,
  writes: "this machine",
  useControl: function useDetailedRemoteLogControl() {
    const [said, turn] = useDetailedRemoteLog();
    const on = said.until !== null;
    return {
      control: (ids) => (
        <div>
          <Switch
            id={ids.id}
            isSelected={on}
            aria-describedby={[ids.describedBy, `${ids.id}-until`].filter(Boolean).join(" ")}
            onChange={turn}
          />
          <p
            id={`${ids.id}-until`}
            className="ui-setting-help ui-text-secondary"
            role="status"
          >
            {said.until === null ? "Off." : `On until ${untilSaid(said.until)}, then off.`}
          </p>
        </div>
      ),
    };
  },
};
