import { useEffect, useState } from "react";
import type { AppInfo } from "@plenipo/types";

import { frontendReady, getAppInfo, type PlenipoCommandError } from "./api/commands";
import { BrandMark } from "./components/BrandMark";

type LoadState =
  | { status: "loading" }
  | { status: "ready"; info: AppInfo }
  | { status: "error"; error: PlenipoCommandError };

export function App() {
  const [state, setState] = useState<LoadState>({ status: "loading" });

  useEffect(() => {
    let cancelled = false;
    getAppInfo()
      .then((info) => {
        if (cancelled) return;
        setState({ status: "ready", info });
        // Best effort: only meaningful in smoke-test mode.
        frontendReady().catch(() => undefined);
      })
      .catch((error: PlenipoCommandError) => {
        if (!cancelled) setState({ status: "error", error });
      });
    return () => {
      cancelled = true;
    };
  }, []);

  return (
    <div className="shell">
      <header className="shell__header">
        <BrandMark />
        <span className="shell__wordmark">Plenipo</span>
        {state.status === "ready" && (
          <span className="shell__version" aria-label="Application version">
            v{state.info.version}
          </span>
        )}
      </header>

      <main className="shell__main">
        <section className="panel" aria-labelledby="welcome-title">
          <h1 id="welcome-title">Your AI workforce control plane</h1>
          <p className="panel__lead">
            Plenipo routes outcomes to managers, coordinators, and specialist workers — with
            explicit permissions and a complete audit trail.
          </p>

          {state.status === "loading" && <p className="status">Connecting to Plenipo Core…</p>}

          {state.status === "error" && (
            <p className="status status--error" role="alert">
              Plenipo Core is unavailable: {state.error.message}
            </p>
          )}

          {state.status === "ready" && (
            <dl className="facts" aria-label="Runtime details">
              <div>
                <dt>Core</dt>
                <dd className="ok">Connected</dd>
              </div>
              <div>
                <dt>Build</dt>
                <dd>{state.info.buildProfile}</dd>
              </div>
              <div>
                <dt>Platform</dt>
                <dd>
                  {state.info.os} / {state.info.arch}
                </dd>
              </div>
              <div>
                <dt>Providers</dt>
                <dd>None configured</dd>
              </div>
            </dl>
          )}
        </section>
      </main>

      <footer className="shell__footer">
        Foundation build · no providers or credentials required
      </footer>
    </div>
  );
}
