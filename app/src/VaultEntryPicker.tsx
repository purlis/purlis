// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { Select, type SelectOption } from "./ui/select/Select";

/** One vault's entries the person may pick, as the app lists them: names only, never a value. */
export type VaultEntries = {
  vault: string;
  keys: readonly string[];
  /** Why its keys could not be listed, where that is so: the provider's own first line. */
  unlisted?: string | null;
};

/** An entry of a vault: a vault and a key in it. Names, never a value. */
export type VaultEntry = { vault: string; key: string };

/** The value an entry travels as inside the select: the vault and the key, apart. */
const valueOf = (entry: VaultEntry): string => JSON.stringify([entry.vault, entry.key]);

const entryOf = (value: string): VaultEntry | undefined => {
  try {
    const [vault, key] = JSON.parse(value) as [unknown, unknown];
    return typeof vault === "string" && typeof key === "string" ? { vault, key } : undefined;
  } catch {
    return undefined;
  }
};

/** What a vault whose keys could not be listed stands for in the list: a row out of reach, saying
 *  why. Its value names no entry. */
const UNLISTED = "unlisted:";

/**
 * **A picker over the person's vault entries** (#5073; the operator's ruling on #5070): the keys
 * of every vault, **grouped by vault with a filter on top**, the select family's groups. A vault
 * whose keys could not be listed stands in its place, out of reach, saying why. Names only: a pick
 * is a vault and a key, never a value. Built for any place an entry is named rather than typed:
 * a Secret ask's "Use an entry I have", and a settings field naming a secret (#3186).
 */
export function VaultEntryPicker({
  entries,
  picked,
  onPick,
  className,
  "aria-label": label = "Entry",
  placeholder = "Pick an entry from your vaults",
  size,
}: {
  entries: readonly VaultEntries[];
  picked: VaultEntry | undefined;
  onPick: (entry: VaultEntry) => void;
  className?: string;
  "aria-label"?: string;
  placeholder?: string;
  size?: "sm" | "md" | "lg";
}) {
  const options: SelectOption[] = entries.flatMap(({ vault, keys, unlisted }) =>
    unlisted != null
      ? [
          {
            value: `${UNLISTED}${vault}`,
            label: "Its keys could not be listed",
            group: vault,
            isDisabled: true,
            tooltip: unlisted,
          },
        ]
      : keys.map((key) => ({ value: valueOf({ vault, key }), label: key, group: vault })),
  );
  return (
    <Select
      className={className}
      aria-label={label}
      placeholder={placeholder}
      size={size}
      filtered
      value={picked === undefined ? "" : valueOf(picked)}
      options={options}
      onValueChange={(to) => {
        const entry = entryOf(to);
        if (entry !== undefined) onPick(entry);
      }}
    />
  );
}
