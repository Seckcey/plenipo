import { useEffect, useState } from "react";
import { Button } from "@plenipo/ui";

import { getScreenshot, toCommandError } from "../api/commands";

/**
 * A kept screenshot (Phase 10): what the page or screen looked like when a worker acted or
 * asked. `startOpen` shows it at once (approval cards); otherwise it waits for a click (the
 * Activity trail, so a long trail doesn't load every picture). Click the picture to enlarge it.
 */
export function ScreenshotView({
  id,
  label,
  startOpen = false,
}: {
  id: string;
  label: string;
  startOpen?: boolean;
}) {
  const [open, setOpen] = useState(startOpen);
  const [large, setLarge] = useState(false);
  const [url, setUrl] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    if (!open || url) return;
    let live = true;
    getScreenshot(id)
      .then((s) => {
        if (live) setUrl(s.dataUrl);
      })
      .catch((e: unknown) => {
        if (live) setError(toCommandError(e).message);
      });
    return () => {
      live = false;
    };
  }, [id, open, url]);
  if (!open) {
    return (
      <Button variant="quiet" size="sm" className="shot__show" onClick={() => setOpen(true)}>
        Show screenshot
      </Button>
    );
  }
  if (error) return <span className="form-error">Screenshot not available: {error}</span>;
  if (!url) return <span className="muted">Loading the screenshot…</span>;
  return (
    <img
      className={`shot${large ? "" : " shot--thumb"}`}
      src={url}
      alt={label}
      title={large ? "Click to make it smaller" : "Click to enlarge"}
      onClick={() => setLarge((v) => !v)}
    />
  );
}
