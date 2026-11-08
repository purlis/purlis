// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { Notice } from "./Notice";
import { answerReports, useReportConsent } from "./reportAnswers";
import { CRASH_REPORTS_SETTING, REPORTS_GROUP } from "./settings/reports";

/**
 * **Asked again once, after a crash** (#5700, spec #5683, story 8): for someone who chose Not now
 * at the first-run question, the next crash purlis catches raises this one Notice in the Inbox.
 * The core's Reports, refusing that crash for want of a yes, keeps it due on this machine
 * (`crash_notice: "due"` in `<config>/reports.json`), so the window opened next says it.
 *
 * Its fix turns Crash reports on; its link opens Settings › You › Reports at that setting; its
 * Dismiss says purlis will not ask again. Any of them, or an answer given in Settings, settles it,
 * and it is never raised a second time.
 */
export function ReportCrashNotice({
  onOpenSettingsAt,
}: {
  /** Opens Settings at a group, focused on one of its settings. */
  onOpenSettingsAt: (group: string, setting: string) => void;
}) {
  const { consent, known } = useReportConsent();
  if (!known || consent.crash_reports !== null || consent.crash_notice !== "due") return null;
  return (
    <Notice
      cause="crash-reports-asked-again"
      label="purlis crashed when it last ran"
      fixes={[
        {
          label: "Turn on crash reports",
          allows: true,
          onPress: () => void answerReports({ crash_reports: true, crash_notice: "settled" }),
        },
      ]}
      link={{
        label: "Open Reports settings",
        onPress: () => onOpenSettingsAt(REPORTS_GROUP, CRASH_REPORTS_SETTING),
      }}
      onDismiss={() => void answerReports({ crash_notice: "settled" })}
      dismissSays="purlis will not ask again. Settings › You › Reports still turns them on."
    >
      purlis crashed when it last ran. Crash reports are off, so nobody who can fix it heard about
      it.
    </Notice>
  );
}
