// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useId } from "react";
import { Radio, RadioGroup } from "../ui/choice/Radio";
import { TextField } from "../ui/field/Field";
import { Recommended } from "./parts";

/**
 * **What Connect asks beside the key, once Connect and Pool are one** (#7191, Refined 2): the
 * pool's name, and who can sign in to its machine. Both are answered on the page that trusts the
 * machine, so one press, Trust and create the pool, runs every check from first contact to a
 * linked runner. The image and limits are what purlis recommends; the Pool step's Options still
 * has them for a person who trusts the machine only.
 */
export type Merged = {
  name: string;
  /** What no pool could be named, said as the box is left. */
  recommended: string;
  onName: (to: string) => void;
  owned: boolean;
  onOwned: (to: boolean) => void;
};

export function MergedFields({ merged, host }: { merged: Merged; host: string }) {
  const id = useId();
  return (
    <>
      <TextField
        label="Name the pool"
        value={merged.name}
        onChange={merged.onName}
        placeholder={merged.recommended}
        help={`Small letters, digits and dashes. Recommended: ${merged.recommended}. The image and limits are what purlis recommends; the Pool step's Options has them.`}
      />
      <h2 className="pool-setup-h2" id={id}>{`Who can sign in to ${host}?`}</h2>
      <RadioGroup
        aria-labelledby={id}
        value={merged.owned ? "mine" : "not"}
        onChange={(to) => merged.onOwned(to === "mine")}
      >
        <Radio
          value="not"
          description="Chats there never get your saved logins. You can change this later, in Settings › Pools, on its row."
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
    </>
  );
}
