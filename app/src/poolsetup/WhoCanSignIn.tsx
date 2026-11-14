// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useId } from "react";
import { Radio, RadioGroup } from "../ui/choice/Radio";
import { Recommended } from "./parts";

/**
 * **Who can sign in to a pool's other machine** (#6271, ADR 0123 §5), as the accepted prototype
 * asks it (#6265, choice 9): the last question of Connect, and again on the pool's row in
 * Settings › Pools. Not sure is recommended and is the default; only the person's own press says
 * that only they, and people they trust with their logins, sign in there, which is what lets the
 * pool's runner receive their saved logins. The answer is for the machine as pinned: a key that
 * changes takes it back to not sure (the core clears it). The word "owned" never shows.
 */
export function WhoCanSignIn({
  host,
  owned,
  onChange,
  busy,
  onARow = false,
}: {
  host: string;
  /** Only the person, and people they trust with their logins, sign in there. */
  owned: boolean;
  onChange: (owned: boolean) => void;
  busy?: boolean;
  /** Drawn on the pool's row in Settings › Pools, under no heading of a step's. */
  onARow?: boolean;
}) {
  const id = useId();
  const question = `Who can sign in to ${host}?`;
  const choices = (
    <RadioGroup
      aria-labelledby={id}
      value={owned ? "mine" : "not"}
      isDisabled={busy}
      onChange={(to) => onChange(to === "mine")}
    >
      <Radio
        value="not"
        description={
          onARow
            ? "Chats there never get your saved logins."
            : "Chats there never get your saved logins. You can change this later, on the pool's page."
        }
      >
        <span className="pool-setup-choice">
          Others might, or I&apos;m not sure <Recommended />
        </span>
      </Radio>
      <Radio
        value="mine"
        description={`Chats there may use your saved logins, each only when you allow it. If ${host}'s key ever changes, this goes back to not sure.`}
      >
        Only me, and people I trust with my logins
      </Radio>
    </RadioGroup>
  );
  return onARow ? (
    <div>
      <p id={id} className="ui-setting-help">
        {question}
      </p>
      {choices}
    </div>
  ) : (
    <section className="pool-setup-action">
      <h2 id={id} className="pool-setup-h2">
        {question}
      </h2>
      {choices}
    </section>
  );
}
