// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { FolderX, LoaderCircle, TriangleAlert } from "lucide-react";
import { Menued } from "./Menus";
import { SettingActions } from "./settings/components";
import {
  CLONE_ALL_ID,
  cloneMissingId,
  dropMembershipId,
  type Catalogued,
  type Offer,
} from "./actions";
import type { CloneState } from "./repoClones";
import { Button } from "./ui/button/Button";
import { PanelSectionTitle } from "./ui/panel/Panel";

/** What this window is cloning into the focused workspace, by repo (`repoClones`). */
export type Cloning = ReadonlyMap<string, CloneState>;

const NOTHING_CLONING: Cloning = new Map();

/**
 * **The repos a workspace names that are not cloned here, and the way to clone them** (#1215).
 *
 * Membership without a clone: `workspace.json` names the repo and this machine has no folder
 * for it. Each row offers Clone, as a button that shows on hover or focus and as the row's
 * menu. The heading offers Clone all when more than one repo is missing. Every button presses
 * a catalogue row (`absent.clone:<repo>`, `absent.cloneAll`), so the palette, the menus and
 * these buttons are one action, and the clone is the one path Settings › Repos takes too.
 *
 * While a clone runs, the row says so. When it fails, the row shows the core's sentence and a
 * Retry that presses the same row again. When it lands, the panels are read again and the repo
 * leaves this list for the clones above it. Nothing here asks for a credential: git and the
 * forge's own helper answer that, or the core says why it could not.
 *
 * **Each row also offers Remove…, which takes the repo out of the workspace** (#1228): beside
 * Clone on hover or focus, and below the line of the row's menu. It presses the catalogue's
 * `absent.drop:<repo>`, which asks first; only the repo's row in `workspace.json` goes, since
 * nothing of it is here to delete. While its clone is under way it is not offered.
 */
export function NotClonedHere({
  absent,
  cloning = NOTHING_CLONING,
  offers,
  onPress,
}: {
  absent: readonly string[];
  cloning?: Cloning;
  offers: Catalogued;
  onPress: (offer: Offer) => void;
}) {
  if (absent.length === 0) return null;
  const all = offers.get(CLONE_ALL_ID);
  return (
    <section className="absent" data-testid="absent">
      <div className="absent-heading">
        <PanelSectionTitle>Not cloned here</PanelSectionTitle>
        {absent.length > 1 && all !== undefined && (
          <div className="absent-actions">
            <SettingActions>
              <Button
                isDisabled={!all.available}
                tooltip={all.available ? all.title : all.reason}
                onPress={() => onPress(all)}
              >
                Clone all
              </Button>
            </SettingActions>
          </div>
        )}
      </div>
      <ul>
        {absent.map((repo) => (
          <AbsentRow
            key={repo}
            repo={repo}
            state={cloning.get(repo)}
            offer={offers.get(cloneMissingId(repo))}
            drop={offers.get(dropMembershipId(repo))}
            offers={offers}
            onPress={onPress}
          />
        ))}
      </ul>
    </section>
  );
}

function AbsentRow({
  repo,
  state,
  offer,
  drop,
  offers,
  onPress,
}: {
  repo: string;
  state: CloneState | undefined;
  offer: Offer | undefined;
  /** The row that asks to take the repo out of the workspace (#1228). */
  drop: Offer | undefined;
  offers: Catalogued;
  onPress: (offer: Offer) => void;
}) {
  // Cloned is busy too, until the panels read again drop the repo from this list: its Clone
  // must not be pressable in between.
  const busy = state !== undefined && state.state !== "failed";
  const press = () => {
    if (offer !== undefined) onPress(offer);
  };
  return (
    <li data-testid={`absent-${repo}`}>
      <Menued on={{ on: "absent", repo }} offers={offers} onPress={onPress}>
        <div className="absent-row">
          <FolderX className="node-icon" />
          <span className="repo ui-text-body ui-text-strong">{repo}</span>
          {busy ? (
            <span className="pending ui-text-secondary" aria-busy="true">
              <LoaderCircle className="node-icon spinning" />
              <span>
                {state.state === "cloning"
                  ? `Cloning ${repo}…`
                  : state.state === "cloned"
                    ? `Cloned ${repo}.`
                    : `${repo} is waiting to be cloned.`}
              </span>
            </span>
          ) : (
            (offer !== undefined || drop !== undefined) && (
              <div className={`absent-actions${state?.state === "failed" ? "" : " row-action"}`}>
                <SettingActions>
                  {offer !== undefined && (
                    <Button
                      aria-label={state?.state === "failed" ? `Retry ${repo}` : offer.title}
                      isDisabled={!offer.available}
                      tooltip={offer.available ? offer.note : offer.reason}
                      onPress={press}
                    >
                      {state?.state === "failed" ? "Retry" : "Clone"}
                    </Button>
                  )}
                  {drop !== undefined && (
                    <Button
                      variant="danger"
                      aria-label={drop.title}
                      isDisabled={!drop.available}
                      tooltip={drop.available ? drop.note : drop.reason}
                      onPress={() => onPress(drop)}
                    >
                      Remove…
                    </Button>
                  )}
                </SettingActions>
              </div>
            )
          )}
        </div>
      </Menued>
      {state?.state === "failed" && (
        // The core's own sentence, never a summary of it: it says whether it was access, the
        // network or a forge the operator is not logged in to.
        <p className="trouble" role="alert">
          <TriangleAlert className="node-icon" />
          <span>{state.said}</span>
        </p>
      )}
    </li>
  );
}
