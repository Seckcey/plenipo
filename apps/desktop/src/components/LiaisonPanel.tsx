import { useCallback, useEffect, useState } from "react";
import type { LiaisonOverview } from "@plenipo/types";

import { getLiaisonOverview, toCommandError } from "../api/commands";

/** Liaison's protocol, limits, destinations, and notices, for the Diagnostics view. */
export function LiaisonPanel() {
  const [overview, setOverview] = useState<LiaisonOverview | null>(null);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async () => {
    try {
      setOverview(await getLiaisonOverview());
      setError(null);
    } catch (reason) {
      setError(toCommandError(reason).message);
    }
  }, []);

  useEffect(() => {
    let cancelled = false;
    getLiaisonOverview()
      .then((o) => !cancelled && setOverview(o))
      .catch((reason) => !cancelled && setError(toCommandError(reason).message));
    return () => {
      cancelled = true;
    };
  }, []);

  return (
    <>
      <div className="section-header">
        <h2>Liaison</h2>
        <button type="button" className="link" onClick={() => void load()}>
          Refresh
        </button>
      </div>
      {error && (
        <p className="status status--error" role="alert">
          Could not load Liaison: {error}
        </p>
      )}
      {overview && (
        <>
          <dl className="facts" aria-label="Liaison details">
            <div>
              <dt>Protocol</dt>
              <dd>{overview.protocol}</dd>
            </div>
            <div>
              <dt>Context packets</dt>
              <dd>{overview.contextFormat}</dd>
            </div>
            <div>
              <dt>Open handoffs</dt>
              <dd>{overview.openHandoffs}</dd>
            </div>
            <div>
              <dt>Limits</dt>
              <dd>
                depth {overview.limits.maxDepth} · {overview.limits.maxRequestsPerAnswer} per answer
                · {overview.limits.maxRounds} reply rounds · {overview.limits.maxWorkflowHandoffs}{" "}
                per workflow
              </dd>
            </div>
          </dl>
          <p className="muted">
            Destinations:{" "}
            {overview.destinations.length === 0
              ? "none"
              : overview.destinations
                  .map((d) => `${d.label} (${d.address}${d.ready ? ", ready" : ", not ready"})`)
                  .join(" · ")}
          </p>
          {overview.notices.length > 0 && (
            <ul className="notices" aria-label="Liaison notices">
              {overview.notices.map((n) => (
                <li key={n}>{n}</li>
              ))}
            </ul>
          )}
        </>
      )}
    </>
  );
}
