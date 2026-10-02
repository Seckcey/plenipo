import { useEffect, useId, useState } from "react";
import { Switch } from "@plenipo/ui";

import { lockScreenChoice, setLockScreenChoice } from "../keep";
import { buf, decode } from "../lock/bytes";
import type { LockScreen } from "../notice";
import { useSession } from "../session-context";
import { iPhoneOutsideHomeScreen } from "../words";

type State = "checking" | "off" | "on" | "blocked" | "unavailable";

/** Can this browser get notices while the page is closed? */
function canGetNotices(): boolean {
  return (
    typeof navigator !== "undefined" &&
    "serviceWorker" in navigator &&
    typeof window !== "undefined" &&
    "PushManager" in window &&
    "Notification" in window
  );
}

async function currentSubscription(): Promise<PushSubscription | null> {
  const registration = await navigator.serviceWorker.getRegistration();
  return (await registration?.pushManager.getSubscription()) ?? null;
}

/**
 * More → **Notices on this phone** (Phase 14 part 14C, ADR-144): notices from your PC even when
 * Plenipo's page is closed, sealed so only this phone can read them, and what the lock screen
 * shows. On an iPhone, they come only to Plenipo on the Home Screen.
 */
export function NoticesOnThisPhone() {
  const { meeting, ask } = useSession();
  const noticeKey = meeting?.welcome.noticeKey ?? null;
  const supported = canGetNotices();
  const [state, setState] = useState<State>(supported ? "checking" : "unavailable");
  const [lock, setLock] = useState<LockScreen>("show");
  const [busy, setBusy] = useState(false);
  const [said, setSaid] = useState<string | null>(null);
  const hint = useId();

  useEffect(() => {
    let live = true;
    void lockScreenChoice().then((choice) => {
      if (live) setLock(choice);
    });
    if (supported) {
      void (
        Notification.permission === "denied"
          ? Promise.resolve<State>("blocked")
          : currentSubscription().then<State, State>(
              (sub) => (sub ? "on" : "off"),
              () => "off",
            )
      ).then((s) => {
        if (live) setState(s);
      });
    }
    return () => {
      live = false;
    };
  }, [supported]);

  const turnOn = async () => {
    const key = noticeKey ? decode(noticeKey) : null;
    if (!key) return;
    setBusy(true);
    setSaid(null);
    try {
      const permission = await Notification.requestPermission();
      if (permission !== "granted") {
        setState(permission === "denied" ? "blocked" : "off");
        return;
      }
      const registration = await navigator.serviceWorker.ready;
      // A sign-up made with another key (an earlier pairing) is dropped first.
      await (await registration.pushManager.getSubscription())?.unsubscribe();
      const sub = await registration.pushManager.subscribe({
        userVisibleOnly: true,
        applicationServerKey: buf(key),
      });
      const { endpoint, keys } = sub.toJSON();
      if (!endpoint || !keys?.p256dh || !keys.auth) throw new Error("no notice keys");
      const reply = await ask({
        kind: "noticesOn",
        subscription: { endpoint, p256dh: keys.p256dh, auth: keys.auth },
      });
      if (reply.ok === undefined) {
        await sub.unsubscribe();
        setSaid(reply.refused?.message ?? reply.failed ?? "Your PC did not take that.");
        setState("off");
        return;
      }
      setState("on");
    } catch {
      setSaid("This phone could not sign up for notices. Try again.");
    } finally {
      setBusy(false);
    }
  };

  const turnOff = async () => {
    setBusy(true);
    setSaid(null);
    try {
      await (await currentSubscription())?.unsubscribe();
      await ask({ kind: "noticesOff" });
      setState("off");
    } catch {
      setSaid("We don't know if your PC got this. Check again when it's back.");
    } finally {
      setBusy(false);
    }
  };

  const choose = (choice: LockScreen) => {
    setLock(choice);
    void setLockScreenChoice(choice).catch(() => setSaid("This phone could not keep that choice."));
  };

  let body;
  if (!noticeKey) {
    body = <p className="muted">Update Plenipo on your PC to get notices on this phone.</p>;
  } else if (iPhoneOutsideHomeScreen()) {
    body = (
      <p className="notice-box" role="note">
        <strong>Add Plenipo to your Home Screen.</strong> On an iPhone, notices come only to Plenipo
        on the Home Screen: tap Share, then Add to Home Screen, and open Plenipo from there.
      </p>
    );
  } else if (state === "unavailable") {
    body = <p className="muted">This browser can’t get notices. Use Safari or Chrome.</p>;
  } else {
    body = (
      <>
        <div className="notices__switch">
          <Switch
            label="Notices on this phone"
            checked={state === "on"}
            disabled={busy || state === "checking"}
            describedBy={hint}
            onChange={(on) => void (on ? turnOn() : turnOff())}
          />
          <span>Notices on this phone</span>
        </div>
        <p id={hint} className="muted">
          When something needs you, your PC sends this phone a notice, even when Plenipo is closed.
          Only this phone can read it.
        </p>
        {state === "blocked" && (
          <p className="form-error" role="alert">
            Notices are blocked for Plenipo on this phone. Allow them in your phone’s settings, then
            try again.
          </p>
        )}
        <fieldset className="notices__lock">
          <legend>On the lock screen</legend>
          <label>
            <input
              type="radio"
              name="lock-screen"
              checked={lock === "show"}
              onChange={() => choose("show")}
            />{" "}
            Show what it is
          </label>
          <label>
            <input
              type="radio"
              name="lock-screen"
              checked={lock === "hide"}
              onChange={() => choose("hide")}
            />{" "}
            Show only “Something needs you”
          </label>
        </fieldset>
      </>
    );
  }
  return (
    <>
      {body}
      {said && (
        <p className="form-error" role="alert">
          {said}
        </p>
      )}
    </>
  );
}
