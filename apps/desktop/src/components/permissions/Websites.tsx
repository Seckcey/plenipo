import { useEffect, useState } from "react";
import type { BrowserStatus, OtherSites, PermissionsSnapshot } from "@plenipo/types";

import { getBrowserStatus, openBrowser, setWebsiteRules, toCommandError } from "../../api/commands";
import { useRun } from "../../guard/useRun";
import { Refusal } from "../models/shared";

type Apply = (s: PermissionsSnapshot) => void;

const lines = (text: string) =>
  text
    .split("\n")
    .map((l) => l.trim())
    .filter(Boolean);

/**
 * Settings → Permissions → Websites (Phase 10): which websites workers may open in Plenipo's
 * browser, which never, and what happens with the others; the terms-of-use warning; and
 * Plenipo's browser itself, which you can open to sign in to a website yourself.
 */
export function Websites({ snapshot, onApply }: { snapshot: PermissionsSnapshot; onApply: Apply }) {
  const rules = snapshot.settings.websites;
  const [allowed, setAllowed] = useState(rules.allowed.join("\n"));
  const [blocked, setBlocked] = useState(rules.blocked.join("\n"));
  const [others, setOthers] = useState<OtherSites>(rules.others);
  const { pending, error, run } = useRun(onApply);
  return (
    <section aria-labelledby="websites-title" className="websites">
      <h3 id="websites-title">Websites</h3>
      <p className="muted">
        Workers with <em>Visit websites</em> or <em>Use websites</em> open pages only in
        Plenipo&apos;s own browser, never in yours, and only as these lists allow. One website per
        line, like <code>example.com</code> (it covers <code>www.example.com</code> too). Addresses
        on this computer or your local network, such as a router, open only when the allowed list
        names them.
      </p>
      <p className="notice-box" role="note">
        <strong>Check a website&apos;s terms before you allow it.</strong> Many websites forbid
        automated use in their terms, for example LinkedIn, Facebook, Instagram, X, TikTok, and
        Amazon (blocked to start with), and most banks and search engines. Plenipo&apos;s browser
        acts under your name, and never gets past a CAPTCHA or other check that a person is using
        the site. Prefer a website&apos;s official connection (API) when it has one.
      </p>
      <form
        aria-label="Website lists"
        onSubmit={(e) => {
          e.preventDefault();
          void run(() =>
            setWebsiteRules({ allowed: lines(allowed), blocked: lines(blocked), others }),
          );
        }}
      >
        <div className="websites__lists">
          <label className="field">
            <span>Allowed (open without asking)</span>
            <textarea rows={6} value={allowed} onChange={(e) => setAllowed(e.target.value)} />
          </label>
          <label className="field">
            <span>Blocked (never open)</span>
            <textarea rows={6} value={blocked} onChange={(e) => setBlocked(e.target.value)} />
          </label>
        </div>
        <label className="field">
          <span>Other websites</span>
          <select value={others} onChange={(e) => setOthers(e.target.value as OtherSites)}>
            <option value="ask">Ask me the first time a worker opens one</option>
            <option value="block">Blocked</option>
          </select>
        </label>
        <div className="actions">
          <button type="submit" className="button" disabled={pending}>
            Save websites
          </button>
        </div>
        <Refusal error={error} />
      </form>
      <PlenipoBrowser />
    </section>
  );
}

/** Plenipo's browser: which one, its own profile, and opening it to sign in yourself. */
function PlenipoBrowser() {
  const [status, setStatus] = useState<BrowserStatus | null>(null);
  const [address, setAddress] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [pending, setPending] = useState(false);
  useEffect(() => {
    let live = true;
    getBrowserStatus()
      .then((s) => {
        if (live) setStatus(s);
      })
      .catch((e: unknown) => {
        if (live) setError(toCommandError(e).message);
      });
    return () => {
      live = false;
    };
  }, []);
  const open = async () => {
    setPending(true);
    setError(null);
    try {
      setStatus(await openBrowser(address));
    } catch (e) {
      setError(toCommandError(e).message);
    } finally {
      setPending(false);
    }
  };
  return (
    <div className="plenipo-browser" aria-label="Plenipo's browser">
      <h4>Plenipo&apos;s browser</h4>
      {status?.name ? (
        <p className="muted">
          {status.name}, with its own profile (your own browser, its sign-ins, and its saved
          passwords are never used). It never saves passwords.{" "}
          <span className={`pill ${status.running ? "pill--ok" : ""}`}>
            {status.running ? "Open" : "Not open"}
          </span>
        </p>
      ) : (
        status && <p className="form-error">{status.problem}</p>
      )}
      <p className="muted">
        Workers never sign in or type passwords. When a website needs you signed in, open it here
        and sign in yourself; workers then use that sign-in until it expires.
      </p>
      <div className="actions">
        <input
          aria-label="Website to open"
          placeholder="https://example.com (optional)"
          value={address}
          onChange={(e) => setAddress(e.target.value)}
        />
        <button
          type="button"
          className="button button--small"
          disabled={pending || !status?.name}
          onClick={() => void open()}
        >
          Open Plenipo&apos;s browser
        </button>
      </div>
      <Refusal error={error} />
    </div>
  );
}
