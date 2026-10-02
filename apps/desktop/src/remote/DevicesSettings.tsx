import { useEffect, useState } from "react";
import type {
  DeviceView,
  KeptOnPc,
  PairingView,
  Qr,
  RemoteSettings,
  SensitiveKind,
} from "@plenipo/types";
import { Button, Checkbox, ErrorState, LoadingState, StatusPill, TextField } from "@plenipo/ui";

import {
  answerPhonePairing,
  cancelPhonePairing,
  removeDevice,
  renameDevice,
  setKeptOnPc,
  startPhonePairing,
  toCommandError,
  unpauseDevice,
} from "../api/commands";
import { PartOfPro } from "../license/PartOfPro";
import type { Go } from "../components/views";
import { when } from "../pages/words";
import { useRemote } from "./useRemote";

/** The picture code (QR code): dark squares on a light ground, with its quiet border. */
export function PictureCode({ qr, label }: { qr: Qr; label: string }) {
  const border = 4;
  const size = qr.size + border * 2;
  const squares: string[] = [];
  for (let y = 0; y < qr.size; y++) {
    for (let x = 0; x < qr.size; x++) {
      if (qr.cells[y * qr.size + x] === "1") squares.push(`M${x + border} ${y + border}h1v1h-1z`);
    }
  }
  return (
    <svg
      className="picture-code"
      role="img"
      aria-label={label}
      viewBox={`0 0 ${size} ${size}`}
      width={size * 6}
      height={size * 6}
      shapeRendering="crispEdges"
    >
      {/* A picture code is always dark on light, in both themes, so a phone can read it. */}
      <rect className="picture-code__ground" width={size} height={size} />
      <path className="picture-code__squares" d={squares.join("")} />
    </svg>
  );
}

/** "9 minutes", "1 minute", "less than a minute". */
export function minutesLeft(endsAt: number, now: number): string {
  const ms = endsAt - now;
  if (ms < 60_000) return "less than a minute";
  const m = Math.floor(ms / 60_000);
  return m === 1 ? "1 minute" : `${m} minutes`;
}

function Pairing({
  pairing,
  page,
  busy,
  run,
}: {
  pairing: PairingView;
  page: string;
  busy: boolean;
  run: (work: () => Promise<RemoteSettings>) => void;
}) {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    const t = window.setInterval(() => setNow(Date.now()), 1000);
    return () => window.clearInterval(t);
  }, []);
  if (pairing.step === "asking") {
    return (
      <div className="notice-box pairing" role="alertdialog" aria-labelledby="pairing-ask">
        <h4 id="pairing-ask">Is this your phone?</h4>
        <p>
          <strong>{pairing.name}</strong> · {pairing.browser} · {when(pairing.since)}
        </p>
        <p className="muted">
          Add it only if this is the phone in your hand. Nothing is added until you say yes.
        </p>
        <div className="settings-section__actions">
          <Button
            variant="primary"
            icon="check"
            disabled={busy}
            onClick={() => run(() => answerPhonePairing(true))}
          >
            Add
          </Button>
          <Button disabled={busy} onClick={() => run(() => answerPhonePairing(false))}>
            Cancel
          </Button>
        </div>
      </div>
    );
  }
  if (pairing.step === "makingPasskey") {
    return (
      <div className="notice-box pairing" role="status">
        <p>
          <strong>Finish on {pairing.name}.</strong> Set up your face, fingerprint, or passcode for
          Plenipo there. Your PC checks it every time the phone signs in.
        </p>
      </div>
    );
  }
  return (
    <div className="pairing pairing--showing">
      <PictureCode qr={pairing.qr} label="Picture code (QR code) to add your phone" />
      <div className="pairing__words">
        <p>
          On your phone, open <strong>{page.replace(/^https?:\/\//, "")}</strong> and tap{" "}
          <strong>Pair this phone</strong>. Then scan this picture code (QR code), or type the
          code:
        </p>
        <p className="pairing__code ui-num" aria-label={`The code: ${pairing.code}`}>
          {pairing.code}
        </p>
        <p className="muted">
          It works once, for {minutesLeft(Number(pairing.endsAt), now)} more.
          {pairing.wrong > 0 &&
            ` ${pairing.wrong} wrong ${pairing.wrong === 1 ? "try" : "tries"}: it stops working after 3.`}
        </p>
        <p className="muted">
          On an iPhone, first add the page to your Home Screen (Share → Add to Home Screen), open
          it from there, and pair from inside it.
        </p>
        <div className="settings-section__actions">
          <Button disabled={busy} onClick={() => run(cancelPhonePairing)}>
            Cancel
          </Button>
        </div>
      </div>
    </div>
  );
}

function Device({
  device,
  busy,
  run,
}: {
  device: DeviceView;
  busy: boolean;
  run: (work: () => Promise<RemoteSettings>) => void;
}) {
  const [renaming, setRenaming] = useState(false);
  const [name, setName] = useState(device.name);
  const [removing, setRemoving] = useState(false);
  return (
    <li className="device">
      <div className="device__head">
        <strong>{device.name}</strong>
        {device.signedIn && <StatusPill status="ok" label="Signed in" />}
        {device.paused && <StatusPill status="error" label="Paused" />}
      </div>
      <p className="muted">
        {device.browser} · Added {when(Number(device.addedAt))}
        {device.lastSeenAt ? ` · Last signed in ${when(Number(device.lastSeenAt))}` : ""}
        {device.notices ? " · Gets notices" : ""}
      </p>
      {device.paused && (
        <p className="form-error" role="note">
          Paused after 3 failed checks of its face, fingerprint, or passcode. Un-pause it only if
          you have this phone with you.
        </p>
      )}
      {renaming ? (
        <form
          className="device__rename"
          onSubmit={(e) => {
            e.preventDefault();
            setRenaming(false);
            run(() => renameDevice(device.id, name));
          }}
        >
          <TextField label={`New name for ${device.name}`} value={name} onChange={setName} />
          <Button type="submit" size="sm" variant="primary" disabled={busy}>
            Save
          </Button>
          <Button size="sm" onClick={() => setRenaming(false)}>
            Cancel
          </Button>
        </form>
      ) : removing ? (
        <div className="notice-box" role="alertdialog" aria-label={`Remove ${device.name}?`}>
          <p>
            Remove <strong>{device.name}</strong>? It is cut off at once. To use it again, add it
            again.
          </p>
          <div className="settings-section__actions">
            <Button
              size="sm"
              variant="danger"
              disabled={busy}
              onClick={() => {
                setRemoving(false);
                run(() => removeDevice(device.id));
              }}
            >
              Remove
            </Button>
            <Button size="sm" onClick={() => setRemoving(false)}>
              Keep it
            </Button>
          </div>
        </div>
      ) : (
        <div className="settings-section__actions">
          <Button size="sm" disabled={busy} onClick={() => setRenaming(true)}>
            Rename
          </Button>
          {device.paused && (
            <Button size="sm" disabled={busy} onClick={() => run(() => unpauseDevice(device.id))}>
              Un-pause
            </Button>
          )}
          <Button size="sm" variant="danger" disabled={busy} onClick={() => setRemoving(true)}>
            Remove
          </Button>
        </div>
      )}
    </li>
  );
}

function KeptList({
  settings,
  busy,
  run,
}: {
  settings: RemoteSettings;
  busy: boolean;
  run: (work: () => Promise<RemoteSettings>) => void;
}) {
  const kept = settings.remote.kept;
  const save = (next: KeptOnPc) => run(() => setKeptOnPc(next));
  const flipKind = (kind: SensitiveKind) => (on: boolean) =>
    save({
      ...kept,
      kinds: on ? [...kept.kinds, kind] : kept.kinds.filter((k) => k !== kind),
    });
  return (
    <fieldset className="kept-on-pc" disabled={busy}>
      <legend>Keep these approvals on my PC only</legend>
      <p className="muted">
        A phone shows these with “Approve on your PC”, and cannot answer them. None to begin with.
      </p>
      <Checkbox
        label="Every approval"
        checked={kept.every}
        onChange={(on) => save({ ...kept, every: on })}
      />
      <Checkbox
        label="Commands on a Production server"
        checked={kept.productionServers}
        onChange={(on) => save({ ...kept, productionServers: on })}
      />
      {settings.sensitive.map((s) => (
        <Checkbox
          key={s.kind}
          label={s.label}
          checked={kept.kinds.includes(s.kind)}
          onChange={flipKind(s.kind)}
        />
      ))}
    </fieldset>
  );
}

/**
 * Settings → Devices (Phase 14, ADR-141, ADR-145): use Plenipo from your phone. Add a phone with a
 * picture code or a typed code, answer "Is this your phone?", see your phones (rename, remove,
 * un-pause), and choose which approvals stay on this PC. Turning phone access on is in Settings →
 * Switches. Everything here is the PC's alone: a phone cannot reach it.
 */
export function DevicesSettings({ go }: { go: Go }) {
  const { settings, error: loadError, reload, show } = useRemote();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  if (!settings) {
    return loadError ? (
      <ErrorState title="Couldn't load your devices" message={loadError} onRetry={reload} />
    ) : (
      <LoadingState label="Loading your devices" />
    );
  }
  const run = (work: () => Promise<RemoteSettings>) => {
    setBusy(true);
    setError(null);
    work()
      .then(show)
      .catch((reason: unknown) => setError(toCommandError(reason).message))
      .finally(() => setBusy(false));
  };
  const r = settings.remote;
  const canAdd = settings.pro && !settings.comingSoon && r.switchedOn && !r.pairing;
  return (
    <div className="devices">
      {!settings.pro && (
        <PartOfPro go={go}>
          Using Plenipo from your phone is part of Pro: see your work, answer approvals, and give
          objectives from your phone, while your PC stays in charge.
        </PartOfPro>
      )}
      {settings.comingSoon && (
        <p className="notice-box" role="note">
          <strong>Coming soon.</strong> Using Plenipo from your phone needs 8 West&rsquo;s relay,
          which is being set up. This page works as soon as it is ready.
        </p>
      )}
      <section aria-labelledby="devices-status">
        <h3 id="devices-status">Your phone and this PC</h3>
        <p>
          {r.switchedOn ? (
            <>
              <strong>On.</strong>{" "}
              {r.connected
                ? "Your PC is connected to 8 West’s relay: your phones can reach it."
                : "Your PC is not connected to 8 West’s relay right now."}
            </>
          ) : (
            <>
              <strong>Off.</strong> Turn it on in{" "}
              <Button size="sm" onClick={() => go({ view: "settings", id: "switches" })}>
                Settings → Switches
              </Button>
            </>
          )}
        </p>
        {r.relayProblem && r.switchedOn && <p className="muted">{r.relayProblem}</p>}
        {r.meetingsStoppedUntil && (
          <p className="form-error" role="alert">
            Someone keeps trying to reach your PC as a phone. Your PC stopped answering new phones
            until {when(Number(r.meetingsStoppedUntil))}. Phones already signed in keep working.
          </p>
        )}
        <p className="muted">
          Your phone talks to this PC through 8 West&rsquo;s relay, sealed so the relay cannot read
          or change anything. Your PC stays in charge: Guard checks every request, and Activity
          shows each one with the phone that sent it.
        </p>
      </section>
      <section aria-labelledby="devices-add">
        <h3 id="devices-add">Add a phone</h3>
        {r.pairing ? (
          <Pairing pairing={r.pairing} page={settings.page} busy={busy} run={run} />
        ) : (
          <>
            <div className="settings-section__actions">
              <Button
                variant="primary"
                icon="plus"
                disabled={!canAdd || busy || r.pairingPausedUntil != null}
                onClick={() => run(startPhonePairing)}
              >
                Add a phone
              </Button>
            </div>
            {r.pairingPausedUntil != null && (
              <p className="form-error" role="note">
                Someone tried wrong codes. Add a phone works again at{" "}
                {when(Number(r.pairingPausedUntil))}.
              </p>
            )}
          </>
        )}
      </section>
      <section aria-labelledby="devices-list">
        <h3 id="devices-list">Your phones</h3>
        {r.devices.length === 0 ? (
          <p className="muted">No phones yet.</p>
        ) : (
          <ul className="devices__list">
            {r.devices.map((d) => (
              <Device key={d.id} device={d} busy={busy} run={run} />
            ))}
          </ul>
        )}
      </section>
      <section aria-labelledby="devices-kept">
        <h3 id="devices-kept">Approvals</h3>
        <KeptList settings={settings} busy={busy} run={run} />
      </section>
      <p className="muted">
        A phone can never reach the terminal, files, the screen, Plenipo&rsquo;s browser, or your
        secrets, and can never change permissions, switches, Guard&rsquo;s rules, or this list.
      </p>
      {error && (
        <p className="form-error" role="alert">
          {error}
        </p>
      )}
    </div>
  );
}
