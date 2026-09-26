import { useEffect, useRef } from "react";
import type { ExecutionRecord } from "@plenipo/types";

import type { OutputView } from "../runtime/store";

export function OutputPanel({
  record,
  output,
}: {
  record: ExecutionRecord;
  output: OutputView | undefined;
}) {
  const ref = useRef<HTMLDivElement>(null);
  const stick = useRef(true);
  const count = output?.lines.length ?? 0;

  // Follow new output unless the user scrolled up to read.
  useEffect(() => {
    const el = ref.current;
    if (el && stick.current) el.scrollTop = el.scrollHeight;
  }, [count]);

  return (
    <div
      ref={ref}
      className="output"
      role="log"
      aria-label={`Output of ${record.label}`}
      onScroll={(e) => {
        const el = e.currentTarget;
        stick.current = el.scrollHeight - el.scrollTop - el.clientHeight < 24;
      }}
    >
      {output && output.dropped > 0 && (
        <div className="output__note">{output.dropped} earlier lines not shown</div>
      )}
      {output && !output.available && (
        <div className="output__note">Output from before Plenipo last started is not kept.</div>
      )}
      {output?.lines.map((line) => (
        <div
          key={line.seq}
          className={`output__line output__line--${line.stream}`}
          data-stream={line.stream}
        >
          {line.text}
          {line.truncated && <span className="output__truncated"> …[truncated]</span>}
        </div>
      ))}
      {count === 0 && output?.available !== false && (
        <div className="output__note">No output yet.</div>
      )}
    </div>
  );
}
