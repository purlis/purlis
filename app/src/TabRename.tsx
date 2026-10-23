// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useId, useRef, useState, type KeyboardEvent } from "react";
import { AnchoredPopover } from "./ui/popover/Popover";
import { RENAMES_ON_F2 } from "./actions";
import { TextField } from "./ui/field/Field";

/**
 * A chat tab's name, open for editing in place on the strip (charter-app#254).
 *
 * **Enter saves, Escape leaves it as it was, and losing the keyboard saves** — the platform's
 * rules for an inline rename, the ones Finder and every file tree follow. A name that did not
 * change is not sent anywhere, so opening the box and leaving it costs nothing and never turns
 * a default into a name the operator did not give.
 *
 * **Every key typed here is the box's.** The tab it replaces closes on Delete (and on a Mac on
 * Backspace, `tabKeys.closeOnDelete`) and moves along the strip on the arrows; none of that
 * may happen to a name being typed, so no keystroke leaves the box — and the palette, which
 * claims `F2` from the whole window, stands back here as it does on the tab (`RENAMES_ON_F2`):
 * opening it would take the keyboard and save a half-typed name. The box is not inside the
 * tab's button — an input inside a button is two controls in one — it takes the button's place
 * while it is open.
 *
 * **The core's refusal is said here, beside the name**, in its own words, and the box stays
 * open on what was typed: a name charter will not draw is one the operator has to change, and
 * a refusal they cannot see next to the name is one they cannot act on. **Except on a blur**:
 * the operator has gone somewhere else, and a box left open behind them, holding the tab's
 * place with nothing to focus, would be a tab they cannot reach. A blur the core refuses leaves
 * the name as it was and closes, as Escape does. It hangs under the box
 * in the popover family's `AnchoredPopover`, because the strip clips what it holds to its own
 * height; the keyboard stays in the box.
 *
 * **It takes the keyboard twice, and the second time is the one that holds.** A rename asked
 * for from the palette or from the tab's menu opens this while that surface is still closing,
 * and a closing surface hands the keyboard back on a timer of its own (its focus scope does it
 * a moment after it unmounts). So the box takes the keyboard as it mounts, and
 * again on a timer set after that one; a blur before then is the surface leaving, not the
 * operator, and saves nothing.
 */
export function TabRename({
  name,
  onSave,
  onDone,
}: {
  /** The name the tab has now, which the box opens on. */
  name: string;
  /** Asks for `typed` to be the name. Answers the core's refusal, or nothing when it held. */
  onSave: (typed: string) => Promise<string | undefined>;
  /** The box is finished with: saved, left, or the name was not changed. `back` is whether
   *  the keyboard goes back to the tab — after Enter and Escape, and not after a blur, which
   *  is the keyboard already somewhere the operator chose. */
  onDone: (back: boolean) => void;
}) {
  const [typed, setTyped] = useState(name);
  const [refused, setRefused] = useState<string>();
  const box = useRef<HTMLInputElement>(null);
  /** Whether a blur is the operator's, which it is from the second focus on. */
  const armed = useRef(false);
  /** Whether a save is on its way, so a blur that lands while it is does not send it twice. */
  const saving = useRef(false);
  /** Whether the box is finished with, so the blur its own removal can cause saves nothing. */
  const finished = useRef(false);
  const whyId = useId();

  useEffect(() => {
    box.current?.focus();
    box.current?.select();
    const again = setTimeout(() => {
      armed.current = true;
      box.current?.focus();
    });
    return () => clearTimeout(again);
  }, []);

  const finish = (back: boolean) => {
    finished.current = true;
    onDone(back);
  };

  const save = async (back: boolean) => {
    if (saving.current || finished.current) return;
    if (typed.trim() === name) {
      finish(back);
      return;
    }
    saving.current = true;
    const why = await onSave(typed);
    saving.current = false;
    // Enter keeps the box open on a refusal, to be fixed; a blur does not (see above).
    if (why === undefined || !back) finish(back);
    else setRefused(why);
  };

  const key = (event: KeyboardEvent<HTMLInputElement>) => {
    event.stopPropagation();
    if (event.key === "Enter") {
      event.preventDefault();
      void save(true);
    } else if (event.key === "Escape") {
      event.preventDefault();
      finish(true);
    } else if (event.key === "F2") {
      // Already renaming: the palette stood back for it, and nothing else is asked for.
      event.preventDefault();
    }
  };

  return (
    <>
      {/* The field family's small box, in the tab's place. */}
      <TextField
        className="tab-rename"
        size="sm"
        inputRef={box}
        value={typed}
        aria-label={`Rename chat ${name}`}
        isInvalid={refused !== undefined}
        aria-describedby={refused === undefined ? undefined : whyId}
        onChange={(to) => {
          setTyped(to);
          setRefused(undefined);
        }}
        onKeyDown={key}
        onBlur={() => {
          if (!armed.current) return;
          if (refused === undefined) void save(false);
          else finish(false);
        }}
        {...{ [RENAMES_ON_F2]: "" }}
      />
      {/* Under the box, lined up with its start; the keyboard stays in the box, where the name
          is being fixed. */}
      <AnchoredPopover
        triggerRef={box}
        isOpen={refused !== undefined}
        className="row-card tab-rename-refused"
      >
        <div id={whyId} role="alert">
          {refused}
        </div>
      </AnchoredPopover>
    </>
  );
}
