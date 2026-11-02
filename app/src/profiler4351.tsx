// DEBUG-4351: temporary, not to land.
import { Profiler, type ReactNode } from "react";

export function Profiler4351({ id, children }: { id: string; children: ReactNode }) {
  return (
    <Profiler
      id={id}
      onRender={(name, phase, actual, _base, start, commit) => {
        const g = globalThis as unknown as { __calls?: string[]; __t0?: number };
        const t0 = g.__t0 ?? 0;
        g.__calls?.push(
          `C ${name}/${phase} react=${Math.round(actual)} at+${Math.round(start - t0)} done+${Math.round(commit - t0)}`,
        );
      }}
    >
      {children}
    </Profiler>
  );
}
