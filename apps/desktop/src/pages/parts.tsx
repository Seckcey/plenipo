/** Parts the pages of one thing share: "not found", screenshots, and a strip with its words. */

import type { ActivitySeries, ArtifactView } from "@plenipo/types";
import {
  ActivityStrip,
  ActivityStripPlaceholder,
  Button,
  EmptyState,
  ErrorState,
  PageHeader,
} from "@plenipo/ui";

import { ScreenshotView } from "../components/ScreenshotView";
import { ago } from "../org/format";
import type { Live } from "./useLive";

/**
 * A page whose department, project, worker, or task is gone (or never was), or couldn't be
 * read (`error`, with Try again).
 */
export function PageMissing({
  kind,
  onBack,
  onHome,
  error = null,
  onRetry,
}: {
  /** "department", "project", "worker", "task". */
  kind: string;
  onBack?: (() => void) | undefined;
  onHome: () => void;
  error?: string | null;
  onRetry?: (() => void) | undefined;
}) {
  return (
    <div className="page">
      <PageHeader
        kicker={kind}
        title={error ? `Couldn't open this ${kind}` : `This ${kind} isn't here`}
        onBack={onBack}
      />
      {error ? (
        <ErrorState
          pip="support"
          title={`Plenipo couldn't read this ${kind}`}
          message={error}
          onRetry={onRetry}
        />
      ) : (
        <EmptyState
          pip="support"
          title={`Plenipo can't find this ${kind}`}
          action={
            <Button size="sm" onClick={onHome}>
              Go to Home
            </Button>
          }
        >
          It may have been removed, or it belongs to a company this copy of Plenipo doesn't have.
        </EmptyState>
      )}
    </div>
  );
}

/** Screenshots workers kept as evidence, newest first; each loads when asked for. */
export function Screenshots({
  artifacts,
  now,
}: {
  artifacts: readonly ArtifactView[];
  now: number;
}) {
  const shots = artifacts.filter((a) => a.kind === "screenshot");
  if (shots.length === 0) return <EmptyState compact title="No screenshots yet" />;
  return (
    <ul className="page__shots" aria-label="Screenshots">
      {shots.map((a) => (
        <li key={a.id}>
          <div className="page__shot-label">
            <span>{a.label ?? "Screenshot"}</span>
            <span className="page__meta">{ago(a.createdAt, now)}</span>
          </div>
          <ScreenshotView id={a.id} label={a.label ?? "A screenshot a worker kept"} />
        </li>
      ))}
    </ul>
  );
}

/** A strip of activity with its heading ("Last 24 hours", "This week"). */
export function Strip({
  label,
  heading,
  live,
  now,
}: {
  label: string;
  heading: string;
  live: Pick<Live<ActivitySeries>, "status" | "value">;
  now: number;
}) {
  return (
    <div className="page__strip">
      <div className="page__strip-heading">{heading}</div>
      {live.status === "loading" ? (
        <ActivityStripPlaceholder state="loading" />
      ) : live.value ? (
        <ActivityStrip series={live.value} now={now} label={label} />
      ) : (
        <ActivityStripPlaceholder state="error" />
      )}
    </div>
  );
}
