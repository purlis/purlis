// DEBUG-4351: temporary probe, not to land.
import { Profiler, type ReactNode } from "react";

export const said4351 = (text: string) => {
  const g = globalThis as unknown as { __calls?: string[]; __t0?: number };
  g.__calls?.push(`${text} @${Math.round(performance.now() - (g.__t0 ?? 0))}`);
};

export function P4351({ id, children }: { id: string; children: ReactNode }) {
  return (
    <Profiler
      id={id}
      onRender={(_id, phase, actual) => {
        if (actual >= 2) said4351(`PROF ${id} ${phase} ${Math.round(actual)}`);
      }}
    >
      {children}
    </Profiler>
  );
}
