// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useState } from "react";
import type { PlaneId } from "../bindings";
import {
  isMinutes,
  setExpiringSound,
  setFinishedAfter,
  setQuietHours,
  setSourceOn,
  useNotificationPrefs,
  type Source,
} from "../notificationPrefs";
import { setQuietNow, useQuiet } from "../quiet";
import { ATTENTION_ALSO } from "./catalogue";
import { Choice, Field } from "./components";
import { stuckSetting } from "./dispatch";
import type { LiveSetting, SettingsGroup } from "./groups";

/**
 * **Settings › You › Attention** (#6019, ADR 0121 decision 22; it took over #1989's
 * Notifications, whose address it keeps): every control over what reaches the person and when.
 *
 * - **Quiet**: the schedule (quiet hours), "Quiet now", and quiet while typing, which is the
 *   core's and has no setting (`asknotify.rs`, `TYPING_QUIET`).
 * - **Notifications per source**, for Should know only, since a Blocking ask always notifies and
 *   Just so you know never does: a failed task, and a long finish while away, after how many
 *   minutes.
 * - **The Expiring sound** (decision 12), off until turned on.
 * - **Stuck?** (#6014): the project's thresholds, kept in its settings files, where they stay
 *   while the operator decides whether they become this machine's. Their row says so; every other
 *   row is this machine's, in the layout file (`notificationPrefs.ts`).
 */
export function notificationsGroup(plane?: PlaneId): SettingsGroup {
  return {
    id: "you.notifications",
    label: "Attention",
    help: "What reaches you, and when. Asks that stop a chat always notify. Everything here is kept on this machine, except Stuck?, which is this project's.",
    also: ATTENTION_ALSO,
    settings: [
      quiet,
      time("from"),
      time("to"),
      quietNow,
      typing,
      blocking,
      source("task-failed"),
      source("task-done"),
      finishedAfter,
      expiringSound,
      ...(plane === undefined ? [] : [stuckSetting(plane)]),
    ],
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

const quietNow: LiveSetting = {
  id: "you.notifications.quietNow",
  label: "Quiet now",
  help: "Hold notifications until you turn this off, or until purlis quits. The ✋ still counts. The same switch is at the top of Asks.",
  writes: "this machine",
  useControl: function useQuietNow() {
    const { by_hand } = useQuiet();
    return {
      control: (ids) => (
        <Choice kind="switch" ids={ids} checked={by_hand} onCheckedChange={setQuietNow} />
      ),
    };
  },
};

const typing: LiveSetting = {
  id: "you.notifications.typing",
  label: "Quiet while you type",
  help: "Any key you type in a chat holds notifications until 30 seconds after the last one, so nothing interrupts you mid-sentence.",
  writes: "nothing",
  useControl: function useTyping() {
    return {
      control: (ids) => (
        <p id={ids.id} className="ui-setting-status" aria-labelledby={ids.labelledBy}>
          Always on
        </p>
      ),
    };
  },
};

const blocking: LiveSetting = {
  id: "you.notifications.blocking",
  label: "Asks that stop a chat",
  help: "A Blocking ask always notifies, one notification per chat, and can never be turned off here, since the chat waits until you answer. Quiet holds the notification, never the ✋ count.",
  writes: "nothing",
  useControl: function useBlocking() {
    return {
      control: (ids) => (
        <p id={ids.id} className="ui-setting-status" aria-labelledby={ids.labelledBy}>
          Always notify
        </p>
      ),
    };
  },
};

/** What each source's switch is called and says. */
const SOURCE_SAID: Readonly<Record<Source, { label: string; help: string }>> = {
  "task-failed": {
    label: "Notify when a task fails",
    help: "A task you asked for that fails notifies you, naming it and the first line of its report. It is in Updates either way.",
  },
  "task-done": {
    label: "Notify when a long task finishes",
    help: "A task you asked for that worked longer than the minutes below notifies you as it finishes, while no purlis window is in front. It is in Updates either way.",
  },
};

function source(kind: Source): LiveSetting {
  return {
    id: `you.notifications.${kind}`,
    ...SOURCE_SAID[kind],
    writes: "this machine",
    useControl: function useSource() {
      const on = !useNotificationPrefs().off.includes(kind);
      return {
        control: (ids) => (
          <Choice
            kind="switch"
            ids={ids}
            checked={on}
            onCheckedChange={(to) => setSourceOn(kind, to)}
          />
        ),
      };
    },
  };
}

const finishedAfter: LiveSetting = {
  id: "you.notifications.finishedAfter",
  label: "Long means more than",
  help: "How many whole minutes a task works before its finish notifies you. Five unless you change it.",
  writes: "this machine",
  useControl: function useFinishedAfter() {
    const prefs = useNotificationPrefs();
    const set = String(prefs.finishedAfter);
    const [typed, setTyped] = useState<string>();
    return {
      control: (ids) => (
        <Field
          kind="text"
          ids={ids}
          inputMode="numeric"
          size={4}
          value={typed ?? set}
          disabled={prefs.off.includes("task-done")}
          validate={(value) =>
            isMinutes(Number(value.trim())) && /^\d+$/.test(value.trim())
              ? undefined
              : "Type a whole number of minutes, 1 or more."
          }
          onChange={setTyped}
          onCommit={() => {
            const minutes = Number(typed?.trim());
            if (typed !== undefined && /^\d+$/.test(typed.trim()) && isMinutes(minutes))
              setFinishedAfter(minutes);
            setTyped(undefined);
          }}
        />
      ),
    };
  },
};

// The notifier plays it (#6110, `asknotify.rs`, `SOUND_BEFORE`): the chat's notification, shown
// again with the system's own sound in the ask's last 15 seconds.
const expiringSound: LiveSetting = {
  id: "you.notifications.expiringSound",
  label: "Sound for an expiring ask",
  help: "A sound as an ask with a countdown has 15 seconds left, once for each ask, and never while it's Quiet. Off unless you turn it on.",
  writes: "this machine",
  useControl: function useExpiringSound() {
    const { expiringSound: on } = useNotificationPrefs();
    return {
      control: (ids) => (
        <Choice kind="switch" ids={ids} checked={on} onCheckedChange={setExpiringSound} />
      ),
    };
  },
};
