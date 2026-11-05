// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useState, type Ref } from "react";
import type { SecretShown } from "./bindings";
import { SecretNeedLine } from "./SecretNeedLine";
import { Checkbox } from "./ui/choice/Checkbox";
import { Segment, SegmentedControl } from "./ui/choice/Radio";
import { TextField } from "./ui/field/Field";
import { VaultEntryPicker, type VaultEntries, type VaultEntry } from "./VaultEntryPicker";

/** How a Secret ask is filled: a new value, or an entry the person keeps already (#5073). */
export type FillWith = "value" | "entry";

/**
 * **What a Secret ask fills, and how** (#5068, #5073, ADR 0109): the key, the vault and the
 * persona by name, then one of two ways. **New value**: a masked box that holds its own words, so
 * the value is never in React's state or the page's markup; Fill reads it from `box`. **Use an
 * entry I have**: a picker over the person's vaults, grouped by vault with a filter on top (the
 * operator's ruling on #5070), whose pick the key comes to read as a Linked entry, never a copy.
 * The chat hears filled or declined and the key's name, never the value and never the entry.
 * Where Fill could also write to the persona, an unticked **Remember this need for the persona**
 * shows the exact line once ticked (#5240, spec #5235). A plain body for now: the ask's own part
 * family replaces it (#5080).
 */
export function SecretFill({
  secret,
  box,
  entries,
  picked,
  onPicked,
  onFillWith,
  remember,
  onRemember,
}: {
  secret: SecretShown;
  box: Ref<HTMLInputElement>;
  /** The entries the person keeps, vault by vault: read once they choose to use one. */
  entries: () => Promise<readonly VaultEntries[]>;
  /** The entry picked, while "Use an entry I have" is chosen; `undefined` otherwise. */
  picked: VaultEntry | undefined;
  /** A pick, or `undefined` as the person goes back to a new value. */
  onPicked: (entry: VaultEntry | undefined) => void;
  /** Which way the person chose, so Fill knows whether a pick is missing. */
  onFillWith?: (to: FillWith) => void;
  /** The purpose of the line Fill writes, while Remember is ticked; `undefined` unticked. */
  remember?: string;
  onRemember?: (to: string | undefined) => void;
}) {
  const [fillWith, setFillWith] = useState<FillWith>("value");
  const listed = useEntries(fillWith === "entry" ? entries : undefined);
  return (
    <div data-testid="secret-fill">
      <p className="ui-text-meta">
        Fills <code>{secret.key}</code> in {secret.persona}'s vault <code>{secret.vault}</code>
        {secret.tool != null && <> for {secret.tool}</>}, on this machine. The chat hears filled or
        declined, never the value.
      </p>
      {secret.replace != null && (
        // The chat's words, said as the chat's (#5075).
        <p className="ui-text-meta" data-testid="secret-replace">
          The chat says the current value doesn't work: <q>{secret.replace}</q>
        </p>
      )}
      {secret.reads != null && (
        <p className="ui-text-meta">
          <code>{secret.key}</code> reads <code>{secret.reads.key}</code> in vault{" "}
          <code>{secret.reads.vault}</code>. A new value goes there and reaches every key linked to
          it; an entry you pick re-points the link.
        </p>
      )}
      {secret.gone != null && (
        <p className="ui-text-meta" data-testid="secret-gone">
          <code>{secret.key}</code> was linked to <code>{secret.gone.key}</code> in vault{" "}
          <code>{secret.gone.vault}</code>, which is gone. Give it a value, or pick another entry.
        </p>
      )}
      <SegmentedControl
        size="sm"
        aria-label={`How to fill ${secret.key}`}
        value={fillWith}
        onChange={(to) => {
          const next = to as FillWith;
          setFillWith(next);
          onFillWith?.(next);
          if (next === "value") onPicked(undefined);
        }}
      >
        <Segment value="value">New value</Segment>
        <Segment value="entry">Use an entry I have</Segment>
      </SegmentedControl>
      {fillWith === "value" ? (
        <TextField
          inputRef={box}
          type="password"
          aria-label={`Value of ${secret.key}`}
          placeholder="Type or paste the value"
        />
      ) : (
        <>
          <VaultEntryPicker
            aria-label={`Entry ${secret.key} reads`}
            entries={listed.entries}
            picked={picked}
            onPick={onPicked}
          />
          <p className="ui-text-meta" role="status">
            {listed.refused ??
              (listed.entries.length === 0 && listed.read
                ? "No vault holds an entry to link to yet. Fill it with a new value instead."
                : picked !== undefined
                  ? `${secret.key} becomes a link to ${picked.key} in ${picked.vault}: the value isn't copied, and a rotation of ${picked.key} reaches it.${
                      // A replace of a key that holds a value of its own deletes that value
                      // (#5075): said before the person links.
                      secret.replace != null && secret.reads == null
                        ? ` The value ${secret.key} holds now is deleted from ${secret.vault}.`
                        : ""
                    }`
                  : "The value isn't copied: the key reads the entry, so a rotation reaches it.")}
          </p>
        </>
      )}
      {secret.remember != null && (
        <>
          <Checkbox
            isSelected={remember !== undefined}
            onChange={(ticked) =>
              onRemember?.(ticked ? (secret.remember?.purpose ?? "") : undefined)
            }
          >
            Remember this need for the persona
          </Checkbox>
          {remember !== undefined && (
            <SecretNeedLine
              secretKey={secret.key}
              file={secret.remember.file}
              need={secret.remember.need}
              vault={secret.remember.vault}
              purpose={remember}
              onPurpose={(to) => onRemember?.(to)}
            />
          )}
        </>
      )}
    </div>
  );
}

/** The entries `read` lists, read once it is given; why not, where it was refused. */
function useEntries(read: (() => Promise<readonly VaultEntries[]>) | undefined): {
  entries: readonly VaultEntries[];
  read: boolean;
  refused?: string;
} {
  const [listed, setListed] = useState<{
    entries: readonly VaultEntries[];
    read: boolean;
    refused?: string;
  }>({ entries: [], read: false });
  const wanted = read !== undefined;
  useEffect(() => {
    if (!wanted || read === undefined) return;
    let live = true;
    read().then(
      (entries) => {
        if (live) setListed({ entries, read: true });
      },
      (why: unknown) => {
        if (live)
          setListed({
            entries: [],
            read: true,
            refused: `purlis could not list your vaults' entries: ${why instanceof Error ? why.message : String(why)}. Fill it with a new value instead.`,
          });
      },
    );
    return () => {
      live = false;
    };
    // Read once each time the person chooses to use an entry.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [wanted]);
  return listed;
}
