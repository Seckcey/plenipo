import { useState } from "react";

import { setPhoneNotices, toCommandError } from "../api/commands";
import { Toggle } from "../components/SwitchSettings";
import { useRemote } from "./useRemote";

/**
 * Settings → Notifications → **Notices on my phones** (Phase 14 part 14C, ADR-144 §7): the same
 * notices go to each phone that asked for them, sealed so only that phone can read them. On to
 * begin with; each phone still asks for notices itself.
 */
export function PhoneNotices() {
  const { settings, error: loadError, show } = useRemote();
  const [pending, setPending] = useState(false);
  const [error, setError] = useState<string | null>(null);
  if (!settings) {
    return (
      <p className={loadError ? "form-error" : "muted"}>
        {loadError ?? "Loading notices on your phones…"}
      </p>
    );
  }
  const asked = settings.remote.devices.filter((d) => d.notices).length;
  const hint = !settings.pro
    ? "Part of Plenipo Pro (Settings → License), with Plenipo on your phone."
    : !settings.remote.switchedOn
      ? "Turn on Use Plenipo from another device first (Settings → Switches)."
      : asked === 0
        ? "No phone has asked for notices yet. On your phone, open Plenipo, then More → Notices on this phone."
        : `${asked === 1 ? "1 phone gets" : `${asked} phones get`} the notices you choose above, sealed so only that phone can read them.`;
  return (
    <>
      <Toggle
        label="Notices on my phones"
        hint={hint}
        checked={settings.remote.phoneNotices}
        disabled={pending}
        onChange={(on) => {
          setPending(true);
          setError(null);
          setPhoneNotices(on)
            .then(show)
            .catch((reason: unknown) => setError(toCommandError(reason).message))
            .finally(() => setPending(false));
        }}
      />
      {error && (
        <p className="form-error" role="alert">
          {error}
        </p>
      )}
    </>
  );
}
