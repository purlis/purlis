// Copyright 2026 Purlis
// SPDX-License-Identifier: Apache-2.0

/** PROTOTYPE, throwaway. The prototype's own bar, not part of any design: the variant (← →, or
 *  its arrows), the moment, and dark or light. Reads and writes `?variant=` and `?moment=`. */
import { useEffect, useState } from "react";
import { ChevronLeft, ChevronRight } from "lucide-react";
import { Button, ButtonRow, IconButton } from "../ui/button/Button";

export type Look = { appearance: string; theme: string };
export type Variant = { key: string; name: string; says: string };
export type Moment = { key: string; name: string };

function param(name: string, fallback: string): string {
  try {
    return new URLSearchParams(window.location.search).get(name) ?? fallback;
  } catch {
    return fallback;
  }
}

function write(name: string, value: string) {
  try {
    const url = new URL(window.location.href);
    url.searchParams.set(name, value);
    window.history.replaceState(null, "", url);
  } catch {
    /* a story's frame may refuse it */
  }
}

/** A value kept in the URL's search, so a variant is shareable and stays on reload. */
export function useParam(name: string, keys: readonly string[]): [string, (to: string) => void] {
  const [value, setValue] = useState(() => {
    const given = param(name, keys[0]!);
    return keys.includes(given) ? given : keys[0]!;
  });
  return [
    value,
    (to: string) => {
      write(name, to);
      setValue(to);
    },
  ];
}

export function ProtoBar({
  variants,
  variant,
  onVariant,
  moments,
  moment,
  onMoment,
  look,
  onLook,
}: {
  variants: readonly Variant[];
  variant: string;
  onVariant: (to: string) => void;
  moments?: readonly Moment[];
  moment?: string;
  onMoment?: (to: string) => void;
  look?: Look;
  onLook?: (to: Look) => void;
}) {
  const at = Math.max(
    0,
    variants.findIndex((one) => one.key === variant),
  );
  const step = (by: number) =>
    onVariant(variants[(at + by + variants.length) % variants.length]!.key);
  useEffect(() => {
    const key = (event: KeyboardEvent) => {
      const target = event.target as HTMLElement | null;
      if (
        target?.closest("input, textarea, [contenteditable], [role=menu], [role=listbox]") ||
        event.metaKey ||
        event.ctrlKey ||
        event.altKey
      )
        return;
      if (event.key === "ArrowLeft") step(-1);
      if (event.key === "ArrowRight") step(1);
    };
    window.addEventListener("keydown", key);
    return () => window.removeEventListener("keydown", key);
  });
  const shown = variants[at]!;
  return (
    <div className="proto-bar" role="group" aria-label="Prototype controls, not part of the design">
      <div className="proto-bar-line">
        <IconButton
          size="sm"
          aria-label="Previous variant"
          icon={<ChevronLeft aria-hidden="true" />}
          onPress={() => step(-1)}
        />
        <strong className="proto-bar-variant">
          {shown.key} · {shown.name}
        </strong>
        <IconButton
          size="sm"
          aria-label="Next variant"
          icon={<ChevronRight aria-hidden="true" />}
          onPress={() => step(1)}
        />
        <span className="proto-bar-says">{shown.says}</span>
      </div>
      {moments !== undefined && onMoment !== undefined && (
        <div className="proto-bar-line">
          <span className="proto-bar-label">Moment</span>
          <ButtonRow wraps>
            {moments.map((one) => (
              <Button
                key={one.key}
                size="sm"
                variant={one.key === moment ? "primary" : "ghost"}
                aria-pressed={one.key === moment}
                onPress={() => onMoment(one.key)}
              >
                {one.name}
              </Button>
            ))}
          </ButtonRow>
          {look !== undefined && onLook !== undefined && (
            <Button
              size="sm"
              variant="ghost"
              onPress={() =>
                onLook({ ...look, appearance: look.appearance === "dark" ? "light" : "dark" })
              }
            >
              {look.appearance === "dark" ? "Light" : "Dark"}
            </Button>
          )}
        </div>
      )}
    </div>
  );
}
