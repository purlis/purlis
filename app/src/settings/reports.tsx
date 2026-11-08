// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import {
  AgentDraftsChoice,
  ALWAYS_SEND_SAYS,
  AlwaysSendChoice,
  CRASH_SAYS,
  CrashReportsChoice,
  DRAFTS_SAYS,
  NEVER_SENT,
  WhatReportsSend,
} from "../ReportConsent";
import { answerReports, useReportConsent } from "../reportAnswers";
import { REPORTS_ALSO } from "./catalogue";
import { SentReports } from "./SentReports";
import type { LiveSetting, SettingsGroup } from "./groups";

/**
 * **You › Reports** (#5688, spec #5683): whether purlis sends its crash reports, and whether
 * agents may write report drafts, as the person answered them at first run, changed here
 * whenever they want; and, while drafts are allowed, whether they go unread by Always send
 * (#5695). Kept on this machine by the core (`<config>/reports.json`), which refuses
 * every draft and send of a stream without its yes. Found by the palette and the filter by what
 * else a person calls it ({@link REPORTS_ALSO}): telemetry, error reporting.
 */
export const REPORTS_GROUP = "you.reports";

/** The Crash reports row, where the crash Notice's link lands (#5700). */
export const CRASH_REPORTS_SETTING = "you.reports.crash";

export function reportsGroup(): SettingsGroup {
  return {
    id: REPORTS_GROUP,
    label: "Reports",
    help: "Crash reports, and report drafts agents write, which help fix purlis. Nothing is sent before you choose.",
    also: REPORTS_ALSO,
    settings: [crashReports, agentDrafts, alwaysSend, whatIsSent, sentList],
  };
}

const crashReports: LiveSetting = {
  id: CRASH_REPORTS_SETTING,
  label: "Crash reports",
  help: CRASH_SAYS,
  writes: "this machine",
  useControl: function useCrashReports() {
    const { consent, trouble } = useReportConsent();
    return {
      grouped: true,
      error: trouble === undefined ? undefined : [trouble],
      control: (ids) => (
        <CrashReportsChoice
          ids={ids}
          value={consent.crash_reports}
          onChange={(send) => void answerReports({ crash_reports: send })}
        />
      ),
    };
  },
};

const agentDrafts: LiveSetting = {
  id: "you.reports.drafts",
  label: "Agent drafts",
  help: DRAFTS_SAYS,
  writes: "this machine",
  useControl: function useAgentDrafts() {
    const { consent } = useReportConsent();
    return {
      grouped: true,
      control: (ids) => (
        <AgentDraftsChoice
          ids={ids}
          value={consent.agent_drafts}
          onChange={(allow) => void answerReports({ agent_drafts: allow })}
        />
      ),
    };
  },
};

/** The Always send row (#5695): drawn only while Agent drafts is allowed. */
export const ALWAYS_SEND_SETTING = "you.reports.always";

const alwaysSend: LiveSetting = {
  id: ALWAYS_SEND_SETTING,
  label: "Always send drafts",
  help: ALWAYS_SEND_SAYS,
  writes: "this machine",
  useShown: function useDraftsAllowed() {
    return useReportConsent().consent.agent_drafts === true;
  },
  useControl: function useAlwaysSend() {
    const { consent } = useReportConsent();
    return {
      grouped: true,
      control: (ids) => (
        <AlwaysSendChoice
          ids={ids}
          value={consent.always_send}
          onChange={(always) => void answerReports({ always_send: always })}
        />
      ),
    };
  },
};

const whatIsSent: LiveSetting = {
  id: "you.reports.what",
  label: "What is sent",
  help: NEVER_SENT,
  writes: "nothing",
  useControl: () => ({ control: (ids) => <WhatReportsSend ids={ids} /> }),
};

const sentList: LiveSetting = {
  id: "you.reports.sent",
  label: "Sent from this machine",
  help: "Every report sent in the last 90 days. Open one to see exactly what left.",
  writes: "nothing",
  useControl: () => ({ control: (ids) => <SentReports ids={ids} /> }),
};
