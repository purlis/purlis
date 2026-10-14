// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useState } from "react";
import { commands } from "../bindings";
import { setQuietHours, useNotificationPrefs } from "../notificationPrefs";
import { Choice, SettingActions } from "./components";
import type { LiveSetting, SettingsGroup } from "./groups";

/**
 * **You › Notifications** (#1989, Q66): the system notifications an ask raises. Only an ask
 * raises one, several chats waiting together are one, and a click opens the Inbox at the
 * first (`asknotify.rs`). Here: the quiet hours, kept in the layout file
 * (`notificationPrefs.ts`), and the system's permission, asked for only when the person
 * presses Allow here, never at launch.
 */
export function notificationsGroup(): SettingsGroup {
  return {
    id: "you.notifications",
    label: "Notifications",
    help: "The system notifications purlis shows when a chat asks you something, on this machine. Several chats asking together are one notification, and clicking it opens the Inbox at the first.",
    settings: [quiet, time("from"), time("to"), permission],
  };
}

const quiet: LiveSetting = {
  id: "you.notifications.quiet",
  label: "Quiet hours",
  help: "Between the two times below, no notification is shown. Asks still wait in the Inbox, and are not shown when quiet hours end.",
  writes: "this machine",
  useControl: function useQuiet() {
    const { on } = useNotificationPrefs().quiet;
    return {
      control: (ids) => (
        <Choice
          kind="switch"
          ids={ids}
          checked={on}
          onCheckedChange={(to) => setQuietHours({ on: to })}
        />
      ),
    };
  },
};

/** Every half hour of the day, `00:00` to `23:30`. */
const HALF_HOURS: readonly string[] = Array.from({ length: 48 }, (_, at) => {
  const hours = String(Math.floor(at / 2)).padStart(2, "0");
  return `${hours}:${at % 2 === 0 ? "00" : "30"}`;
});

function time(which: "from" | "to"): LiveSetting {
  return {
    id: `you.notifications.quiet.${which}`,
    label: which === "from" ? "Quiet from" : "Quiet until",
    help:
      which === "from"
        ? "When quiet hours start, on this machine's clock."
        : "When they end. A time before the start runs across midnight.",
    writes: "this machine",
    useControl: function useTime() {
      const quiet = useNotificationPrefs().quiet;
      const now = quiet[which];
      // A time written by hand off the half hour is still offered, as it is.
      const times = HALF_HOURS.includes(now) ? HALF_HOURS : [...HALF_HOURS, now].sort();
      return {
        control: (ids) => (
          <Choice
            kind="select"
            ids={ids}
            options={times.map((one) => ({ value: one, label: one }))}
            value={now}
            disabled={!quiet.on}
            onValueChange={(to) => setQuietHours({ [which]: to })}
          />
        ),
      };
    },
  };
}

/** What the system says of purlis's notifications (`notification_permission`). */
type Permission = "granted" | "denied" | "prompt";

const asPermission = (said: string): Permission =>
  said === "granted" || said === "denied" ? said : "prompt";

const permission: LiveSetting = {
  id: "you.notifications.permission",
  label: "System permission",
  help: "Whether your system lets purlis show notifications. purlis asks only when you press Allow.",
  writes: "nothing",
  useControl: function usePermission() {
    const [state, setState] = useState<Permission | undefined>(undefined);
    useEffect(() => {
      let gone = false;
      void commands
        .notificationPermission()
        .then((said) => {
          if (!gone) setState(asPermission(said));
        })
        .catch(() => undefined);
      return () => {
        gone = true;
      };
    }, []);
    return {
      control: (ids) => (
        <span id={ids.id} aria-describedby={ids.describedBy}>
          {state === "granted" && "Allowed. Your system's notification settings can turn them off."}
          {state === "denied" &&
            "Not allowed. Allow purlis in your system's notification settings to see them."}
          {state === "prompt" && (
            <SettingActions>
              <button
                type="button"
                tabIndex={0}
                className="ui-button"
                onClick={() => {
                  void commands
                    .askNotificationPermission()
                    .then((said) => setState(asPermission(said)))
                    .catch(() => undefined);
                }}
              >
                Allow
              </button>
            </SettingActions>
          )}
        </span>
      ),
    };
  },
};
