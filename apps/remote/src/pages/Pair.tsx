import { useEffect, useRef, useState } from "react";
import type { PairStep } from "@plenipo/types";
import { Button, TextField } from "@plenipo/ui";
import jsQR from "jsqr";

import type { Keep, Kept } from "../keep";
import { codeFromHash, parseCode } from "../lock/code";
import { newKeyPair, type KeyPair } from "../lock/noise";
import { makePasskey } from "../lock/passkey";
import { Pairing } from "../line/phone";
import type { SocketMaker } from "../line/relay";
import { pairingProblem } from "./pairing-words";
import { guessPhone, iPhoneOutsideHomeScreen } from "../words";

type Step =
  | { kind: "start" }
  | { kind: "scanning" }
  | { kind: "meeting" }
  | { kind: "waiting" }
  | { kind: "accepted"; accepted: Extract<PairStep, { step: "accepted" }> }
  | { kind: "finishing" };

/** The camera, reading a picture code (QR code) until one holds a pairing code. */
function Scanner({
  onCode,
  onStop,
}: {
  onCode: (code: string) => void;
  onStop: (why?: string) => void;
}) {
  const video = useRef<HTMLVideoElement>(null);
  const canvas = useRef<HTMLCanvasElement>(null);
  useEffect(() => {
    let stream: MediaStream | null = null;
    let frame = 0;
    let done = false;
    const look = () => {
      if (done) return;
      const v = video.current;
      const c = canvas.current;
      if (v && c && v.readyState >= 2 && v.videoWidth > 0) {
        c.width = v.videoWidth;
        c.height = v.videoHeight;
        const g = c.getContext("2d", { willReadFrequently: true });
        if (g) {
          g.drawImage(v, 0, 0);
          const image = g.getImageData(0, 0, c.width, c.height);
          const found = jsQR(image.data, image.width, image.height);
          if (found) {
            let code: string | null;
            try {
              code = codeFromHash(new URL(found.data).hash);
            } catch {
              code = parseCode(found.data);
            }
            if (code) {
              done = true;
              onCode(code);
              return;
            }
          }
        }
      }
      frame = requestAnimationFrame(look);
    };
    navigator.mediaDevices
      ?.getUserMedia({ video: { facingMode: "environment" }, audio: false })
      .then((s) => {
        stream = s;
        if (video.current) {
          video.current.srcObject = s;
          void video.current.play();
        }
        frame = requestAnimationFrame(look);
      })
      .catch(() => onStop("This phone did not let Plenipo use its camera. Type the code instead."));
    return () => {
      done = true;
      cancelAnimationFrame(frame);
      stream?.getTracks().forEach((t) => t.stop());
    };
  }, [onCode, onStop]);
  return (
    <div className="scanner">
      <video ref={video} playsInline muted aria-label="The camera, looking for the picture code" />
      <canvas ref={canvas} hidden />
      <Button onClick={() => onStop()}>Type the code instead</Button>
    </div>
  );
}

/**
 * Pair this phone with your PC (ADR-141): on your PC, Settings → Devices → Add a phone shows a
 * picture code and a typed code. Scan it here, or type it; your PC asks "Is this your phone?",
 * and then this phone makes its passkey.
 */
export function PairPage({
  keep,
  onPaired,
  make,
}: {
  keep: Keep;
  onPaired: (kept: Kept) => void;
  make?: SocketMaker;
}) {
  const guess = guessPhone();
  const [name, setName] = useState(guess.name);
  const [typed, setTyped] = useState(() => codeFromHash(window.location.hash) ?? "");
  const [step, setStep] = useState<Step>({ kind: "start" });
  const [problem, setProblem] = useState<string | null>(null);
  const pairing = useRef<Pairing | null>(null);
  const keys = useRef<KeyPair | null>(null);
  const outsideHomeScreen = iPhoneOutsideHomeScreen();

  useEffect(() => () => pairing.current?.cancel(), []);

  const begin = (code: string) => {
    setProblem(null);
    setStep({ kind: "meeting" });
    void (async () => {
      try {
        keys.current = await newKeyPair(false);
        pairing.current = await Pairing.start({
          code,
          name: name.trim() || guess.name,
          browser: guess.browser,
          keys: keys.current,
          ...(make ? { make } : {}),
        });
        // The code is used: take it out of the address.
        if (window.location.hash) history.replaceState(null, "", window.location.pathname);
        setStep({ kind: "waiting" });
        const answer = await pairing.current.ownersAnswer();
        if (answer.step === "accepted") {
          setStep({ kind: "accepted", accepted: answer });
        } else {
          setProblem(
            answer.step === "refused" || answer.step === "failed"
              ? answer.message
              : "Your PC did not add this phone.",
          );
          setStep({ kind: "start" });
        }
      } catch (e) {
        setProblem(pairingProblem(e));
        setStep({ kind: "start" });
      }
    })();
  };

  const finish = (accepted: Extract<PairStep, { step: "accepted" }>) => {
    setProblem(null);
    setStep({ kind: "finishing" });
    void (async () => {
      try {
        const passkey = await makePasskey(accepted.passkey);
        const paired = await pairing.current!.finish(accepted, passkey);
        const kept: Kept = { paired, keys: keys.current! };
        await keep.save(kept);
        onPaired(kept);
      } catch (e) {
        setProblem(pairingProblem(e));
        setStep({ kind: "start" });
      }
    })();
  };

  const code = parseCode(typed);
  return (
    <section className="page pair" aria-labelledby="pair-title">
      <h1 id="pair-title">Pair this phone</h1>
      <p className="lead">
        On your PC, open Plenipo: <strong>Settings → Devices → Add a phone</strong>. It shows a
        picture code and a typed code. Your PC stays in charge; this phone talks to it sealed end to
        end, through 8 West&rsquo;s relay.
      </p>
      {outsideHomeScreen && (
        <p className="notice-box" role="note">
          <strong>On an iPhone, add this page to your Home Screen first:</strong> tap Share, then
          Add to Home Screen. Open Plenipo from there, and pair from inside it, so notices work
          later.
        </p>
      )}
      {step.kind === "start" && (
        <form
          className="pair__form"
          onSubmit={(e) => {
            e.preventDefault();
            if (code) begin(code);
          }}
        >
          <TextField label="What to call this phone" value={name} onChange={setName} />
          <Button variant="primary" icon="search" onClick={() => setStep({ kind: "scanning" })}>
            Scan the code on your PC
          </Button>
          <TextField
            label="Or type the code"
            value={typed}
            onChange={setTyped}
            placeholder="XXXX-XXXX-XXXX-XXXX"
            hint="16 letters and digits, as your PC shows them."
          />
          <Button type="submit" disabled={!code}>
            Pair this phone
          </Button>
        </form>
      )}
      {step.kind === "scanning" && (
        <Scanner
          onCode={(c) => {
            setTyped(c);
            begin(c);
          }}
          onStop={(why) => {
            if (why) setProblem(why);
            setStep({ kind: "start" });
          }}
        />
      )}
      {step.kind === "meeting" && <p role="status">Meeting your PC…</p>}
      {step.kind === "waiting" && (
        <p className="notice-box" role="status">
          <strong>Your PC is asking: Is this your phone?</strong> Click <strong>Add</strong> on your
          PC.
        </p>
      )}
      {step.kind === "accepted" && (
        <div className="notice-box" role="status">
          <p>
            <strong>Your PC said yes.</strong> Now set up your face, fingerprint, or passcode for
            Plenipo. Your PC checks it every time this phone signs in.
          </p>
          <Button variant="primary" icon="lock" onClick={() => finish(step.accepted)}>
            Set up Face ID, fingerprint, or passcode
          </Button>
        </div>
      )}
      {step.kind === "finishing" && <p role="status">Finishing…</p>}
      {problem && (
        <p className="form-error" role="alert">
          {problem}
        </p>
      )}
    </section>
  );
}
