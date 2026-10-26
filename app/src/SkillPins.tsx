// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * **A skill's pins in the window** (SK-15d, #3232; ADR 0102 §4): the Pin switch on a Skill studio
 * row filtered to a workspace or a persona, and the list on a skill's own tab of every workspace
 * and persona that pins it, each with Unpin. Each change is the core's `skills::verbs::pin`, which
 * `purlis skill pin` calls too, one commit. A chat may write its persona's pins, so a pin is
 * always seen and can always be undone.
 */
import { type SkillPinner } from "./bindings";
import { pinnerSaid } from "./skillStudio";
import { Button } from "./ui/button/Button";
import { Switch } from "./ui/choice/Switch";

/** Whose chats a pin keeps a skill listed for, as a sentence says it. */
export function pinnedChats(by: SkillPinner): string {
  return by.at === "workspace" ? `${by.name}'s chats` : `chats as ${by.name}`;
}

/** **The Pin switch** of a studio row: on while `by` pins `name`. */
export function SkillPinSwitch({
  name,
  by,
  pinned,
  busy,
  onPin,
}: {
  name: string;
  /** The workspace or the persona the studio is filtered to. */
  by: SkillPinner;
  pinned: boolean;
  busy: boolean;
  /** Pins it (`true`) or unpins it. */
  onPin: (on: boolean) => void;
}) {
  return (
    <Switch
      aria-label={`Pin ${name} for ${by.name}`}
      isSelected={pinned}
      isDisabled={busy}
      tooltip={
        pinned
          ? `Pinned for ${pinnedChats(by)}: no router hides it from them`
          : `Keep it listed for ${pinnedChats(by)}, past any budget`
      }
      onChange={onPin}
    >
      Pinned
    </Switch>
  );
}

/** **Who pins a skill**, on its tab: every workspace and persona, each with Unpin. */
export function SkillPinnedBy({
  name,
  pinners,
  busy,
  onUnpin,
}: {
  name: string;
  pinners: readonly SkillPinner[];
  busy: boolean;
  onUnpin: (by: SkillPinner) => void;
}) {
  return (
    <section aria-labelledby="skill-pinned-by-heading" data-testid="skill-pinned-by">
      <h3 id="skill-pinned-by-heading">Pinned by</h3>
      {pinners.length === 0 ? (
        <p className="honest">No workspace or persona pins {name}.</p>
      ) : (
        <ul aria-label={`Who pins ${name}`}>
          {pinners.map((by) => (
            <li key={`${by.at}\u0000${by.name}`}>
              Pinned for {pinnerSaid(by)}{" "}
              <Button
                size="sm"
                aria-label={`Unpin ${name} for ${by.name}`}
                isDisabled={busy}
                onPress={() => onUnpin(by)}
              >
                Unpin
              </Button>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}
