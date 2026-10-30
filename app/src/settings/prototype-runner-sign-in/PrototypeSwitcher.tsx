// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * PROTOTYPE, throwaway (branch prototype/runner-sign-in, never merged to main).
 *
 * The floating bar: ← and → (or the arrow keys) cycle the variants; the second control steps
 * through the sign-in states ([ and ] too); the third picks where the sign-in is asked. Hidden
 * in a production build.
 */
import { useEffect } from "react";
import { Button } from "../../ui/button/Button";

export interface PrototypeAxis<T extends string> {
  /** What the axis is, said before its value. */
  name: string;
  values: readonly T[];
  current: T;
  said: (value: T) => string;
  onChange: (to: T) => void;
}

function step<T extends string>(axis: PrototypeAxis<T>, by: 1 | -1) {
  const at = axis.values.indexOf(axis.current);
  const next = (at + by + axis.values.length) % axis.values.length;
  axis.onChange(axis.values[next]);
}

/** Whether a key press belongs to something typed in. */
function typing(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  return (
    target.closest("input, textarea, select, [contenteditable=''], [contenteditable='true']") !==
    null
  );
}

function Axis<T extends string>({ axis, keys }: { axis: PrototypeAxis<T>; keys: string }) {
  return (
    <div className="proto-rsi-bar-axis">
      <Button
        size="sm"
        variant="ghost"
        aria-label={`Previous ${axis.name}`}
        onPress={() => step(axis, -1)}
      >
        ‹
      </Button>
      <span className="proto-rsi-bar-said" aria-live="polite">
        <span className="proto-rsi-bar-name">
          {axis.name} <kbd>{keys}</kbd>
        </span>
        {axis.said(axis.current)}
      </span>
      <Button
        size="sm"
        variant="ghost"
        aria-label={`Next ${axis.name}`}
        onPress={() => step(axis, 1)}
      >
        ›
      </Button>
    </div>
  );
}

export function PrototypeSwitcher<V extends string, S extends string, M extends string>({
  variant,
  state,
  moment,
}: {
  variant: PrototypeAxis<V>;
  state: PrototypeAxis<S>;
  moment: PrototypeAxis<M>;
}) {
  useEffect(() => {
    const press = (event: KeyboardEvent) => {
      if (typing(event.target) || event.metaKey || event.ctrlKey || event.altKey) return;
      if (event.key === "ArrowLeft") step(variant, -1);
      else if (event.key === "ArrowRight") step(variant, 1);
      else if (event.key === "[") step(state, -1);
      else if (event.key === "]") step(state, 1);
      else return;
      event.preventDefault();
    };
    window.addEventListener("keydown", press);
    return () => window.removeEventListener("keydown", press);
  }, [variant, state]);
  if (import.meta.env.PROD) return null;
  return (
    <div className="proto-rsi-bar" role="toolbar" aria-label="PROTOTYPE switcher">
      <span className="proto-rsi-bar-tag">PROTOTYPE</span>
      <Axis axis={variant} keys="← →" />
      <Axis axis={state} keys="[ ]" />
      <Axis axis={moment} keys="" />
    </div>
  );
}
