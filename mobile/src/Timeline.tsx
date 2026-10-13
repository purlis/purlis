import { useEffect, useRef } from "react";

import type { Row } from "./timeline.ts";

/** The strip above the terminal (iOS design §6.1): the chat's story, newest at the bottom. */
export function Timeline({ rows }: { rows: Row[] }) {
  const end = useRef<HTMLDivElement>(null);
  useEffect(() => {
    end.current?.scrollIntoView?.({ block: "end" });
  }, [rows.length]);
  return (
    <div className="timeline" role="log" aria-label="Timeline">
      {rows.map((row) => (
        <div key={row.key} className={`row row-${row.kind}`}>
          {row.text}
        </div>
      ))}
      <div ref={end} />
    </div>
  );
}
