// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useCallback, useEffect, useMemo, useState } from "react";
import { KeyRound } from "lucide-react";
import { PanelList } from "./PanelList";
import { catalogueMenu } from "./Menus";
import { HeadingOffer, PanelSection } from "./PanelSection";
import { Notice } from "./Notice";
import type { Catalogued, Offer } from "./actions";
import { commands, type PanelRow, type VaultSummary } from "./bindings";
import { providerName } from "./vaultProviders";
import { Dialog } from "./ui/dialog/Dialog";
import { ListFilter, noMatchSaid, useListFilter } from "./ui/list/ListFilter";

/**
 * The plane's vaults, in the Attention region: each vault with its provider and how many
 * secrets it holds (charter-app#234), and **pressing one opens its tab** (charter-app#235).
 *
 * **Names and counts, and never a value.** `vault_list` answers with nothing else
 * (`app/src-tauri/src/vaults.rs`, whose test serializes every answer and looks for the values
 * it wrote), so there is nothing here that could draw one.
 *
 * **Made and deleted from here too** (SI-3): the heading's `+` is `vault.create`, and each row's
 * context menu offers `vault.remove:<name>`, which asks in `DeleteVault` before anything goes.
 *
 * **The plane's, not the workspace's.** A vault is registered once per plane, so this section
 * is drawn whichever workspace is focused, and with none.
 *
 * **The window asks, and this draws** (`useVaults`, held by `PlaneView`): the same answer is
 * the palette's `vault.open:<name>` rows and the picker's list, and a vault's tab asks the window
 * to read it again after a write — so one list, read in one place, serves all four.
 */
export function Vaults({
  said,
  offers,
  onPress,
}: {
  /** `reload` is a refusal's Read again: the list is read once per project (NO-8, #1296). */
  said: Pick<VaultsSaid, "vaults" | "trouble" | "reload">;
  /** The catalogue by id, which is what a row's verb is looked up in. */
  offers: Catalogued;
  onPress: (offer: Offer) => void;
}) {
  const { vaults, trouble, reload } = said;
  return (
    <PanelSection
      testid="panel-vaults"
      mark={KeyRound}
      title="Vaults"
      // **New vault… on the heading** (SI-3): the palette's row, one press from where the vaults
      // are listed. It needs a plane and not a vault, so it is there on an empty section too.
      actions={<HeadingOffer offer={offers.get("vault.create")} onPress={onPress} />}
    >
      {trouble !== undefined ? (
        <Notice
          cause="vaults-unread"
          tone="trouble"
          fixes={[{ label: "Read again", onPress: reload }]}
        >
          {trouble}
        </Notice>
      ) : vaults === undefined ? (
        // Said while it is read (#1719): a heading with nothing under it reads as no vaults.
        <p className="pending" role="status" aria-busy="true">
          Reading the vaults…
        </p>
      ) : (
        <PanelList
          rows={vaults.map(rowOf)}
          empty={NO_VAULTS}
          label="Vaults"
          testid="list-vaults"
          // No row opens a card: pressing one opens the vault's tab, which says the rest.
          open={undefined}
          onOpen={() => undefined}
          onRun={(id) => {
            // **The catalogue's row or nothing**, `Panels.tsx`'s rule: an id the catalogue
            // has stopped offering runs nothing rather than something else.
            const offer = offers.get(id);
            if (offer) onPress(offer);
          }}
          // Right-click on a vault: open it, make another, delete it (SI-3). The rows are the
          // catalogue's (`menuOn`), so the menu and the palette say the same thing.
          menuOf={(row) => catalogueMenu({ on: "vault", vault: row.key }, offers, onPress)}
        />
      )}
    </PanelSection>
  );
}

/** How many secrets, in words: `1 secret`, `2 secrets`. The panel and a vault's tab both say it. */
export function counted(n: number): string {
  return `${n} ${n === 1 ? "secret" : "secrets"}`;
}

/** What a list of vaults says when the plane has none. */
const NO_VAULTS = {
  headline: "No vaults in this project",
  body: "Make one with the + above, or New vault… in the palette.",
  offer: null,
};

/** What Open vault says when there is none: the dialog has no + to point at (#630). */
const NO_VAULTS_TO_OPEN = { ...NO_VAULTS, body: "Make one with New vault… in the palette." };

/**
 * One vault as a row: its name, then its provider and count. Pressing it runs the catalogue's
 * `vault.open:<name>`, which opens the vault's tab. A vault charter cannot read is marked, and
 * its tab says why — the card that used to say it would be a second surface for one vault.
 */
function rowOf(vault: VaultSummary): PanelRow {
  const count = vault.count === null ? "" : ` · ${counted(vault.count)}`;
  return {
    key: vault.name,
    text: vault.name,
    note: `${providerName(vault.provider)}${count}`,
    mark: "vault",
    tone: vault.health.ok ? "plain" : "trouble",
    detail: null,
    runs: `vault.open:${vault.name}`,
    actions: [],
  };
}

/**
 * **"Open vault…"**: the plane's vaults in a dialog, one press from the palette when the name is
 * not what the operator has in mind. Each row runs the same `vault.open:<name>` the panel's rows
 * and the palette's own rows run. The keyboard lands on the first vault, and Up, Down and Enter
 * are all it takes.
 */
export function OpenVault({
  vaults,
  offers,
  onPress,
  onCancel,
}: {
  vaults: readonly VaultSummary[];
  offers: Catalogued;
  onPress: (offer: Offer) => void;
  onCancel: () => void;
}) {
  // More than about seven vaults are filtered (#2910). The keyboard lands on the filter, or on a
  // short list's first vault, and Up, Down and Enter are all it takes.
  const { query, setQuery, shown, filtered } = useListFilter(vaults, vaultName);
  return (
    <Dialog
      onClose={onCancel}
      title="Open vault"
      initialFocus={(dialog) =>
        filtered
          ? dialog.querySelector<HTMLElement>(".list-filter input")
          : dialog.querySelector<HTMLElement>('[data-scope="list"][data-part="root"]')
      }
    >
      {filtered && (
        <ListFilter noun={VAULTS} total={vaults.length} query={query} onQuery={setQuery} />
      )}
      {filtered && shown.length === 0 ? (
        <p className="came-back" role="status">
          {noMatchSaid(VAULTS, query)}
        </p>
      ) : (
        <PanelList
          rows={shown.map(rowOf)}
          empty={NO_VAULTS_TO_OPEN}
          label="Vaults to open"
          open={undefined}
          onOpen={() => undefined}
          onRun={(id) => {
            const offer = offers.get(id);
            if (!offer) return;
            onCancel();
            onPress(offer);
          }}
        />
      )}
    </Dialog>
  );
}

/** What a vault is filtered by: its name. */
const vaultName = (vault: VaultSummary) => vault.name;

const VAULTS = { one: "vault", many: "vaults" };

/** What `vault_list` said for a plane, and a way to ask it again. */
export type VaultsSaid = { vaults?: VaultSummary[]; trouble?: string; reload: () => void };

/**
 * `vault_list` for one plane, asked when the plane is and whenever `reload` is called — which a
 * vault's tab does after every write, since the counts here are that vault's.
 *
 * **An answer that is not a list is no answer**, the rule `useDoctor` keeps: whole-window tests
 * mock every command they do not care about with `null` or `[]`.
 */
export function useVaults(plane: string): VaultsSaid {
  const [said, setSaid] = useState<{ plane: string; vaults?: VaultSummary[]; trouble?: string }>();
  const [asked, setAsked] = useState(0);
  useEffect(() => {
    let gone = false;
    void commands
      .vaultList(plane)
      .then((answer) => {
        if (gone || answer == null) return;
        if (answer.status === "error") setSaid({ plane, trouble: answer.error });
        else if (Array.isArray(answer.data)) setSaid({ plane, vaults: answer.data });
      })
      // Said, not swallowed (#1719): an ask that failed outright is a refusal like any other,
      // with Read again, where an empty section would read as no vaults at all.
      .catch((err: unknown) => {
        if (!gone) setSaid({ plane, trouble: String(err) });
      });
    return () => {
      gone = true;
    };
  }, [plane, asked]);
  const reload = useCallback(() => setAsked((was) => was + 1), []);
  // An answer for the plane before is not this plane's. Memoised so the object's identity only
  // changes when the answer does: it is passed down to `Panels` and read into a `useMemo`, and a
  // fresh object every render would re-run both on every unrelated window change.
  const mine = said?.plane === plane ? said : undefined;
  return useMemo(
    () => (mine ? { vaults: mine.vaults, trouble: mine.trouble, reload } : { reload }),
    [mine, reload],
  );
}
