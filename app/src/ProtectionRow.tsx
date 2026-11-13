// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import type { ProtectionWord } from "./bindings";
import { Choice, SettingRow } from "./settings/components";
import { LEVEL_SAID } from "./settings/yourLevel";

/** Where setup starts: Basic sandbox (Q115). */
export const PROTECTION_PRESET: ProtectionWord = "floor";

/** The options in the row's order: the recommended one first (R-061). */
const ORDER: readonly ProtectionWord[] = ["floor", "on", "off"];

/** One line of help per option (R-062), Full sandbox's with R-295's sentence (Q159). */
export const PROTECTION_HELP: Readonly<Record<ProtectionWord, string>> = {
  floor: "Chats can't read your vaults or change what later programs run. They reach any host.",
  on: "Chats also write only inside the project. New hosts are asked about the first time a chat reaches them.",
  off: "Chats run as you, with nothing between them and your files.",
};

/**
 * **The Protection row** of First run and New project (T1-24; Q115, Q159): Basic sandbox
 * preselected, a line of help under each option, and "Change any time in Settings". The answer
 * goes with the page's own main button, so setup still takes one press at every choice, and the
 * core keeps it as the person's own level for the project, the value Settings › Project ›
 * Sandbox › Your level reads (Q236). No sandbox's own lines are T1-25's.
 */
export function ProtectionRow({
  value,
  onChange,
  disabled,
}: {
  value: ProtectionWord;
  onChange: (to: ProtectionWord) => void;
  /** Held while the page's act is being done. */
  disabled?: boolean;
}) {
  return (
    <SettingRow
      label="Protection"
      setting="setup-protection"
      help="Change any time in Settings."
      control={(ids) => (
        <Choice
          ids={ids}
          kind="radio"
          value={value}
          onValueChange={(to) => onChange(to as ProtectionWord)}
          disabled={disabled}
          options={ORDER.map((one) => ({
            value: one,
            label: one === PROTECTION_PRESET ? `${LEVEL_SAID[one]} (recommended)` : LEVEL_SAID[one],
            says: PROTECTION_HELP[one],
          }))}
        />
      )}
    />
  );
}
