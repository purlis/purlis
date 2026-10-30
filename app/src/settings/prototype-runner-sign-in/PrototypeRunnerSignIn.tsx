// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/**
 * PROTOTYPE, throwaway (branch prototype/runner-sign-in, never merged to main).
 *
 * Three variants of signing a profile in on a pool's runner, in Settings › You › This machine ›
 * Pools, switchable from the floating bar: A (inline in the row), B (one dialog per sign-in),
 * C (a panel per pool with one queue). The bar also steps through the sign-in states and where
 * the sign-in is asked (Settings, the end of Add a pool, Move to runner).
 */
import "./prototype.css";
import {
  PROTOTYPE_MOMENT_SAID,
  PROTOTYPE_MOMENTS,
  SIGN_IN_STATE_SAID,
  SIGN_IN_STATES,
  type PrototypeMoment,
  type SignInState,
} from "./data";
import { PrototypeSwitcher } from "./PrototypeSwitcher";
import { VARIANT_A_NAME, VariantA } from "./VariantA";
import { VARIANT_B_NAME, VariantB } from "./VariantB";
import { VARIANT_C_NAME, VariantC } from "./VariantC";

export const PROTOTYPE_VARIANTS = ["A", "B", "C"] as const;
export type PrototypeVariant = (typeof PROTOTYPE_VARIANTS)[number];

const NAMES: Record<PrototypeVariant, string> = {
  A: VARIANT_A_NAME,
  B: VARIANT_B_NAME,
  C: VARIANT_C_NAME,
};

export function PrototypeRunnerSignIn({
  variant,
  state,
  moment,
  onChange,
}: {
  variant: PrototypeVariant;
  state: SignInState;
  moment: PrototypeMoment;
  onChange: (to: {
    variant?: PrototypeVariant;
    state?: SignInState;
    moment?: PrototypeMoment;
  }) => void;
}) {
  const Variant = variant === "A" ? VariantA : variant === "B" ? VariantB : VariantC;
  return (
    <div className="proto-rsi-root">
      <p className="proto-rsi-banner" role="note">
        PROTOTYPE · Variant {variant}: {NAMES[variant]} · State: {SIGN_IN_STATE_SAID[state]} ·
        Where: {PROTOTYPE_MOMENT_SAID[moment]}
      </p>
      {/* Keyed, so a field typed in one state starts empty in the next. */}
      <Variant key={`${variant}-${state}-${moment}`} state={state} moment={moment} />
      <PrototypeSwitcher
        variant={{
          name: "Variant",
          values: PROTOTYPE_VARIANTS,
          current: variant,
          said: (one) => `${one} (${NAMES[one]})`,
          onChange: (to) => onChange({ variant: to }),
        }}
        state={{
          name: "State",
          values: SIGN_IN_STATES,
          current: state,
          said: (one) => SIGN_IN_STATE_SAID[one],
          onChange: (to) => onChange({ state: to }),
        }}
        moment={{
          name: "Where",
          values: PROTOTYPE_MOMENTS,
          current: moment,
          said: (one) => PROTOTYPE_MOMENT_SAID[one],
          onChange: (to) => onChange({ moment: to }),
        }}
      />
    </div>
  );
}
