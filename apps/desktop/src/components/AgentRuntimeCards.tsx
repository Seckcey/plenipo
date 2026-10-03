import { useEffect, useRef, useState } from "react";
import type { AgentRuntimeInfo, AiToolsPage } from "@plenipo/types";
import { Button, LoadingState } from "@plenipo/ui";

import { checkAiToolVersions, setAiToolsAutoUpdate, toCommandError } from "../api/commands";
import { useAgents } from "../agents/useAgents";
import { useRun } from "../guard/useRun";
import { when } from "../pages/words";
import { useRouting } from "../routing/useRouting";
import { AiToolCard, type KeyCard } from "./aiTools/AiToolCard";
import { keyToolFor } from "./aiTools/keyFor";
import { useAiTools } from "./aiTools/useAiTools";
import { AUTO_UPDATE_HINT, AUTO_UPDATE_LABEL } from "./aiTools/words";
import { Toggle } from "./SwitchSettings";
import type { Go } from "./views";
import { systemWords } from "../system/words";

/**
 * Each AI tool in one place (Phase 19): sign in, see its usage, see how it is paid for, keep it up
 * to date, and see its models. The AI tools you sign in to come first, each with a key box for
 * paying per use instead (2026-09-30); then the paid AI tools that come with Plenipo (OpenRouter
 * and each AI company's own service, Phase 16 Wave 3). `focusId`: the tool whose card to show
 * (another page asked for it).
 */
export function AgentRuntimeCards({
  focusId = null,
  go,
}: {
  focusId?: string | null;
  go?: Go | undefined;
}) {
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

  // Going straight to a card: it scrolls into view and takes the keyboard, once it is shown in
  // its own list (a paid AI tool moves to its part of the page when the page's part arrives).
  const shown =
    focusId !== null &&
    state.runtimes.some((r) => r.id === focusId) &&
    (page !== null || ai.error !== null);
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

  const toolOf = (id: string) => page?.tools.find((t) => t.runtimeId === id);
  // Until the page's own part arrives, every card is listed with the AI tools you sign in to.
  const isPaid = (id: string) => toolOf(id)?.payment === "paidKey";
  const signedIn = state.runtimes.filter((r) => !isPaid(r.id));
  const paid = state.runtimes.filter((r) => isPaid(r.id));
  // The paid AI tool whose key a card's key box saves: the same key as on that tool's own card.
  const keyCardFor = (r: AgentRuntimeInfo): KeyCard | undefined => {
    if (isPaid(r.id)) return undefined;
    const id = keyToolFor(r.id);
    const info = paid.find((p) => p.id === id);
    const tool = toolOf(id);
    return info && tool ? { info, tool } : undefined;
  };
  const item = (r: AgentRuntimeInfo) => (
    <li key={r.id} aria-label={`${r.label} AI tool`} tabIndex={-1}>
      <AiToolCard
        info={r}
        tool={toolOf(r.id)}
        route={routing.snapshot?.tools.find((t) => t.runtimeId === r.id)}
        usageRevision={ai.usageRevision}
        onApply={ai.apply}
        onRouting={routing.apply}
        go={go}
        keyCard={keyCardFor(r)}
      />
    </li>
  );

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
        Plenipo uses the AI tools already signed in on this computer, with their subscriptions, and
        never sees your passwords. To pay per use instead, put your own key in the key box on a
        card: it is typed only here and kept in {systemWords().keyStore}. Plenipo never updates an
        AI tool while a task is using it.
      </p>
      {paid.length > 0 && (
        <div className="ai-tool__buttons">
          <Button
            size="sm"
            variant="quiet"
            onClick={() => {
              const heading = document.getElementById("paid-ai-tools");
              heading?.scrollIntoView?.({ block: "start" });
              heading?.focus({ preventScroll: true });
            }}
          >
            Go to the AI tools paid per use
          </Button>
        </div>
      )}
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
        {signedIn.map(item)}
        {state.runtimes.length === 0 && (
          <li>
            <LoadingState />
          </li>
        )}
      </ul>
      {paid.length > 0 && (
        <>
          <div className="section-header">
            <h2 id="paid-ai-tools" tabIndex={-1}>
              Paid per use with your key
            </h2>
          </div>
          <p className="muted">
            These come with Plenipo: OpenRouter, which reaches hundreds of models from many AI
            companies, and each AI company&apos;s own service. Each takes your key from that
            company, paid per use. A spending limit is up to you (Settings → Spending caps); every
            paid task is priced and listed there either way.
          </p>
          <ul className="ai-tools" aria-label="AI tools paid per use">
            {paid.map(item)}
          </ul>
        </>
      )}
    </>
  );
}
