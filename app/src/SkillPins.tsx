// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **A skill's pins in the window** (SK-15d, #3232; ADR 0102 §4): the Pin switch on a Skill studio
 * row filtered to a workspace or a persona, and the list on a skill's own tab of every workspace
 * and persona that pins it, each with Unpin. Each change is the core's `skills::verbs::pin`, which
 * `purlis skill pin` calls too, one commit. A chat may write its persona's pins, so a pin is
 * always seen and can always be undone.
 */
import { type SkillPinner, type SkillPinsRefused } from "./bindings";
import { pinnerSaid, pinsRefusedSaid } from "./skillStudio";
import { Button } from "./ui/button/Button";
import { Switch } from "./ui/choice/Switch";

/** Whose chats a pin keeps a skill listed for, as a sentence says it. */
export function pinnedChats(by: SkillPinner): string {
  return by.at === "workspace" ? `${by.name}'s chats` : `chats as ${by.name}`;
}

/**
 * **The Pin switch** of a studio row: on while `by` pins `name`. Where a chat reads none of
 * `by`'s pins (`refused`), it says "not used" and why, so a pin that has no effect never looks
 * as if it works.
 */
export function SkillPinSwitch({
  name,
  by,
  pinned,
  refused,
  busy,
  onPin,
}: {
  name: string;
  /** The workspace or the persona the studio is filtered to. */
  by: SkillPinner;
  pinned: boolean;
  refused?: SkillPinsRefused | null;
  busy: boolean;
  /** Pins it (`true`) or unpins it. */
  onPin: (on: boolean) => void;
}) {
  const unused = refused !== undefined && refused !== null;
  return (
    <Switch
      aria-label={`Pin ${name} for ${pinnerSaid(by)}`}
      isSelected={pinned}
      isDisabled={busy}
      tooltip={
        unused
          ? pinsRefusedSaid(refused)
          : pinned
            ? `Pinned for ${pinnedChats(by)}: no router hides it from them`
            : `Keep it listed for ${pinnedChats(by)}, past any budget`
      }
      onChange={onPin}
    >
      {unused && pinned ? "Pinned, not used" : "Pinned"}
    </Switch>
  );
}

/**
 * **Why a pinner's pins aren't used**, with the way out: Remove for the entry that is not a
 * skill's name, one change, or the file to edit where the list is too long.
 */
export function SkillPinsRefusedLine({
  refused,
  busy,
  onRemove,
}: {
  refused: SkillPinsRefused;
  busy: boolean;
  /** Unpins `bad`, the entry that is not a skill's name. */
  onRemove: (bad: string) => void;
}) {
  const bad = refused.bad;
  return (
    <span className="honest" data-testid="skill-pins-refused">
      {pinsRefusedSaid(refused)}{" "}
      {bad === null ? (
        <>
          Unpin some, or edit <code>{refused.file}</code>.
        </>
      ) : (
        <Button
          size="sm"
          aria-label={`Remove it, the pin ${JSON.stringify(bad)} of ${pinnerSaid(refused.by)}`}
          isDisabled={busy}
          onPress={() => onRemove(bad)}
        >
          Remove it
        </Button>
      )}
    </span>
  );
}

/** **Who pins a skill**, on its tab: every workspace and persona, each with Unpin. */
export function SkillPinnedBy({
  name,
  pinners,
  refused = [],
  busy,
  onUnpin,
  onRemove,
}: {
  name: string;
  pinners: readonly SkillPinner[];
  /** Each of `pinners` whose pins a chat reads none of, and why. */
  refused?: readonly SkillPinsRefused[];
  busy: boolean;
  onUnpin: (by: SkillPinner) => void;
  /** Unpins `bad`, an entry of `by`'s that is not a skill's name. */
  onRemove?: (by: SkillPinner, bad: string) => void;
}) {
  return (
    <section aria-labelledby="skill-pinned-by-heading" data-testid="skill-pinned-by">
      <h3 id="skill-pinned-by-heading">Pinned by</h3>
      {pinners.length === 0 ? (
        <p className="honest">No workspace or persona pins {name}.</p>
      ) : (
        <ul aria-label={`Who pins ${name}`}>
          {pinners.map((by) => {
            const unused = refused.find((one) => one.by.at === by.at && one.by.name === by.name);
            return (
              <li key={`${by.at}\u0000${by.name}`}>
                Pinned for {pinnerSaid(by)}{" "}
                <Button
                  size="sm"
                  aria-label={`Unpin ${name} for ${pinnerSaid(by)}`}
                  isDisabled={busy}
                  onPress={() => onUnpin(by)}
                >
                  Unpin
                </Button>
                {unused !== undefined && (
                  <>
                    {" "}
                    <SkillPinsRefusedLine
                      refused={unused}
                      busy={busy}
                      onRemove={(bad) => onRemove?.(by, bad)}
                    />
                  </>
                )}
              </li>
            );
          })}
        </ul>
      )}
    </section>
  );
}
