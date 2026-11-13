// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **A settings group on its page, for a story** (#6301): the group as Settings › You draws it,
 * in the aligned form (#2919, R37–R54) beside the side column, at the width of a Settings tab in
 * the default window, so a story's picture is the page the person sees. Each live setting is
 * drawn as the Settings tab draws it: its row, its label and help, and its control.
 */
import type { ReactNode } from "react";
import { SettingGroup, SettingRow, SettingsLayout } from "./components";
import type { LiveSetting, SettingsGroup } from "./groups";

/** A live setting's row, as the Settings tab draws one. */
export function LiveRow({ setting }: { setting: LiveSetting }) {
  const { control, grouped, error, undo } = setting.useControl();
  return (
    <SettingRow
      setting={setting.id}
      label={setting.label}
      help={setting.help}
      grouped={grouped}
      error={error}
      undo={undo}
      control={control}
    />
  );
}

/** `group` on its page at You, its live settings drawn by their rows, then `after`. */
export function OnItsPage({ group, after }: { group: SettingsGroup; after?: ReactNode }) {
  return (
    // The view's body, as a Settings tab is drawn in it: the side column takes its height.
    <div className="view-body" style={{ blockSize: 720, inlineSize: 1040 }}>
      <SettingsLayout
        levels={[{ id: "you", label: "You" }]}
        level="you"
        onLevelChange={() => undefined}
        groups={[{ id: group.id, label: group.label }]}
        group={group.id}
        onGroupChange={() => undefined}
        filter=""
        onFilterChange={() => undefined}
      >
        <SettingGroup label={group.label} help={group.help}>
          {group.settings.map((one) => (
            <LiveRow key={one.id} setting={one as LiveSetting} />
          ))}
        </SettingGroup>
        {after}
      </SettingsLayout>
    </div>
  );
}
