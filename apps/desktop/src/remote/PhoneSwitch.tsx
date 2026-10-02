import { useState } from "react";

import { setRemoteSwitch, toCommandError } from "../api/commands";
import { Toggle } from "../components/SwitchSettings";
import { useRemote } from "./useRemote";

/**
 * Settings → Switches → **Use Plenipo from another device** (Phase 14, ADR-145 §1): off to begin
 * with, Pro only, and "Coming soon" until 8 West's relay is ready. Off cuts every phone off at
 * once; the phones stay listed in Settings → Devices.
 */
export function PhoneSwitch() {
  const { settings, error: loadError, show } = useRemote();
  const [pending, setPending] = useState(false);
  const [error, setError] = useState<string | null>(null);
  if (!settings) {
    return (
      <p className={loadError ? "form-error" : "muted"}>
        {loadError ?? "Loading Use Plenipo from another device…"}
      </p>
    );
  }
  const hint = !settings.pro
    ? "Part of Plenipo Pro (Settings → License): see your work, answer approvals, and give objectives from your phone."
    : settings.comingSoon
      ? "Coming soon: it needs 8 West's relay, which is being set up."
      : "Off to start with. On: phones you add in Settings → Devices can see your work, answer approvals, and give objectives, through 8 West's relay, sealed end to end. Turning it off cuts every phone off at once.";
  return (
    <>
      <Toggle
        label="Use Plenipo from another device"
        hint={hint}
        checked={settings.remote.switchedOn}
        disabled={pending || ((!settings.pro || settings.comingSoon) && !settings.remote.switchedOn)}
        onChange={(on) => {
          setPending(true);
          setError(null);
          setRemoteSwitch(on)
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
