// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useId, useState } from "react";
import { Notice } from "../Notice";
import { Button } from "../ui/button/Button";
import { Radio, RadioGroup } from "../ui/choice/Radio";
import { Recommended } from "./parts";

/**
 * **Who can sign in to a pool's other machine** (#6271, ADR 0123 §5), as the accepted prototype
 * asks it (#6265, choice 9): the last question of Connect, and again on the pool's row in
 * Settings › Pools. Not sure is recommended and is the default; only the person's own press says
 * that only they, and people they trust with their logins, sign in there, which is what lets the
 * pool's runner receive their saved logins. The answer is for the machine as pinned: a host key
 * or CA that changes takes it back to not sure (the core clears it). The word "owned" never
 * shows.
 *
 * **Nothing is sent until Save is pressed** (#6614's review): the arrow keys move a radio
 * group's choice, so a keyboard or screen reader reading the answers would otherwise set the mark
 * and, arrowing back, forget the pool's logins. Where taking the mark back forgets a made pool's
 * logins (`forgetsLoginsOf`), Save asks first.
 */
export function WhoCanSignIn({
  host,
  owned,
  over,
  onSave,
  forgetsLoginsOf,
  onARow = false,
}: {
  host: string;
  /** Only the person, and people they trust with their logins, sign in there, as kept. */
  owned: boolean;
  /** How the machine is pinned: by its host key over SSH, by its CA over Docker's TLS API. */
  over: "ssh" | "tls";
  /** Keeps the answer chosen, on Save alone. Settles once it is kept or refused. */
  onSave: (owned: boolean) => Promise<unknown>;
  /** The made pool whose logins taking the mark back forgets: Save asks first. */
  forgetsLoginsOf?: string;
  /** Drawn on the pool's row in Settings › Pools, under no heading of a step's. */
  onARow?: boolean;
}) {
  const id = useId();
  const [chosen, setChosen] = useState(owned);
  const [saving, setSaving] = useState(false);
  const [asking, setAsking] = useState(false);
  // What is kept moved (saved, or the pin trusted again): the choice shows it again, set as the
  // answer kept is drawn, not after it.
  const [kept, setKept] = useState(owned);
  if (kept !== owned) {
    setKept(owned);
    setChosen(owned);
    setAsking(false);
  }
  const question = `Who can sign in to ${host}?`;
  const pinned = over === "tls" ? "CA" : "key";
  const save = () => {
    setAsking(false);
    setSaving(true);
    void onSave(chosen).finally(() => setSaving(false));
  };
  const unchanged = chosen === owned;
  const press = () => {
    if (unchanged || saving) return;
    if (owned && !chosen && forgetsLoginsOf !== undefined) setAsking(true);
    else save();
  };
  const choices = (
    <>
      <RadioGroup
        aria-labelledby={id}
        value={chosen ? "mine" : "not"}
        onChange={(to) => {
          setAsking(false);
          setChosen(to === "mine");
        }}
      >
        <Radio
          value="not"
          description={
            onARow
              ? "Chats there never get your saved logins."
              : "Chats there never get your saved logins. You can change this later, in Settings › Pools, on its row."
          }
        >
          <span className="pool-setup-choice">
            Others might, or I&apos;m not sure <Recommended />
          </span>
        </Radio>
        <Radio
          value="mine"
          description={`Chats there may use your saved logins, each only when you allow it. If ${host}'s ${pinned} ever changes, this goes back to not sure.`}
        >
          Only me, and people I trust with my logins
        </Radio>
      </RadioGroup>
      <Button
        size="sm"
        aria-label={`Save who can sign in to ${host}`}
        aria-disabled={unchanged || saving || undefined}
        onPress={press}
      >
        {saving ? "Saving…" : "Save"}
      </Button>
      {asking && forgetsLoginsOf !== undefined && (
        <Notice
          cause={`pool-forget-logins:${forgetsLoginsOf}`}
          tone="attention"
          fixes={[{ label: "Forget its logins", onPress: save }]}
          onDismiss={() => {
            setAsking(false);
            setChosen(owned);
          }}
        >
          {`Saying others might sign in to ${host} forgets the logins you allowed on pool ${forgetsLoginsOf}. Each asks you again once you say only you sign in there.`}
        </Notice>
      )}
    </>
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
