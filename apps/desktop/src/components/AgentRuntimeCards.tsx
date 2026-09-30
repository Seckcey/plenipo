import { useEffect, useRef, useState } from "react";
import type { AiToolsPage } from "@plenipo/types";
import { Button, LoadingState } from "@plenipo/ui";

import { checkAiToolVersions, setAiToolsAutoUpdate, toCommandError } from "../api/commands";
import { useAgents } from "../agents/useAgents";
import { useRun } from "../guard/useRun";
import { when } from "../pages/words";
import { useRouting } from "../routing/useRouting";
import { AiToolCard } from "./aiTools/AiToolCard";
import { useAiTools } from "./aiTools/useAiTools";
import { AUTO_UPDATE_HINT, AUTO_UPDATE_LABEL } from "./aiTools/words";
import { Toggle } from "./SwitchSettings";

/**
 * Each AI tool (Claude Code, Codex, Grok, Kimi, Ollama, Antigravity, GitHub Copilot) in one place (Phase 19): sign in, see its
 * usage, see how it is paid for, keep it up to date, and see its models. `focusId`: the tool whose
 * card to show (another page asked for it).
 */
export function AgentRuntimeCards({ focusId = null }: { focusId?: string | null }) {
  const { state, refresh } = useAgents();
  const ai = useAiTools();
  const routing = useRouting();
  const [checking, setChecking] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const looking = useRun<AiToolsPage>(ai.apply);
  const switching = useRun<AiToolsPage>(ai.apply);
  const page = ai.page;

  async function recheck() {
    setChecking(true);
    setError(null);
    try {
      await refresh();
    } catch (reason) {
      setError(toCommandError(reason).message);
    } finally {
      setChecking(false);
    }
  }

  // Going straight to a card: it scrolls into view and takes the keyboard, once it is shown.
  const shown = focusId !== null && state.runtimes.some((r) => r.id === focusId);
  const handled = useRef<string | null>(null);
  useEffect(() => {
    if (!focusId) {
      handled.current = null;
      return;
    }
    if (!shown || handled.current === focusId) return;
    handled.current = focusId;
    const card = document.getElementById(`ai-tool-${focusId}`)?.closest("li");
    card?.scrollIntoView?.({ block: "start" });
    card?.focus({ preventScroll: true });
  }, [focusId, shown]);

  const isLooking = looking.pending || page?.looking === true;
  const refusal = error ?? looking.error ?? switching.error;

  return (
    <>
      <div className="section-header">
        <h2>Your AI tools</h2>
      </div>
      <div className="ai-tools__bar">
        <Button
          size="sm"
          variant="primary"
          icon="refresh"
          disabled={isLooking}
          onClick={() => void looking.run(checkAiToolVersions)}
        >
          {isLooking ? "Looking for new versions…" : "Check for new versions"}
        </Button>
        <Button size="sm" disabled={checking} onClick={() => void recheck()}>
          {checking ? "Checking…" : "Check again"}
        </Button>
        <span className="muted">
          Last looked for new versions:{" "}
          {page?.lastLookedAt ? when(page.lastLookedAt) : page ? "not yet" : "…"}
        </span>
      </div>
      <Toggle
        label={AUTO_UPDATE_LABEL}
        hint={AUTO_UPDATE_HINT}
        checked={page?.autoUpdate ?? false}
        disabled={!page || switching.pending}
        onChange={(on) => void switching.run(() => setAiToolsAutoUpdate(on))}
      />
      <p className="muted">
        Plenipo uses the AI tools already signed in on this computer, with their subscriptions. It
        never asks for passwords or API keys, and never updates an AI tool while a task is using it.
      </p>
      {refusal && (
        <p className="status status--error" role="alert">
          {refusal}
        </p>
      )}
      {ai.error && !page && (
        <p className="status status--error" role="alert">
          Could not load the AI tools&apos; updates and usage: {ai.error}
        </p>
      )}
      <ul className="ai-tools" aria-label="AI tools">
        {state.runtimes.map((r) => (
          <li key={r.id} aria-label={`${r.label} AI tool`} tabIndex={-1}>
            <AiToolCard
              info={r}
              tool={page?.tools.find((t) => t.runtimeId === r.id)}
              route={routing.snapshot?.tools.find((t) => t.runtimeId === r.id)}
              usageRevision={ai.usageRevision}
              onApply={ai.apply}
              onRouting={routing.apply}
            />
          </li>
        ))}
        {state.runtimes.length === 0 && (
          <li>
            <LoadingState />
          </li>
        )}
      </ul>
    </>
  );
}
