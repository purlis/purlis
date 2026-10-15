// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { setQuietHours, useNotificationPrefs } from "../notificationPrefs";
import { Choice } from "./components";
import type { LiveSetting, SettingsGroup } from "./groups";

/**
 * **You › Notifications** (#1989, Q66): the system notifications an ask raises. Only an ask
 * raises one, several chats waiting together are one, and a click opens the Inbox at the
 * first still waiting (`asknotify.rs`). Here: the quiet hours, kept in the layout file
 * (`notificationPrefs.ts`). Whether purlis may show notifications at all is the system's own
 * notification settings' to say.
 */
export function notificationsGroup(): SettingsGroup {
  return {
    id: "you.notifications",
    label: "Notifications",
    help: "The system notifications purlis shows when a chat asks you something, on this machine. Several chats asking together are one notification, and clicking it opens the Inbox at the first still waiting. Your system's own notification settings decide whether purlis may show them at all.",
    settings: [quiet, time("from"), time("to")],
  };
}

const quiet: LiveSetting = {
  id: "you.notifications.quiet",
  label: "Quiet hours",
  help: "Between the two times below, no notification is shown, and asks wait in the Inbox. When quiet hours end, one notification names the chats still waiting on you.",
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
