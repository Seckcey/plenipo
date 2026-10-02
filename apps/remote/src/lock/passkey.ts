import type { NewPasskey, PasskeyAnswer, PasskeyRequest } from "@plenipo/types";

import { buf, decode, encode } from "./bytes";

/**
 * Passkeys (ADR-142): a key in the phone's own keychain that works only after your face,
 * fingerprint, or phone passcode. Your PC checks every answer itself; the relay and 8 West never
 * see it. Made once when the phone is paired; asked at each sign-in.
 */

/** Why a passkey step did not happen, in plain words. */
export class PasskeyProblem extends Error {}

function bytes(b64: string, what: string): Uint8Array {
  const out = decode(b64);
  if (!out) throw new PasskeyProblem(`Your PC sent a ${what} this phone could not read.`);
  return out;
}

function explain(e: unknown): PasskeyProblem {
  const name = e instanceof DOMException ? e.name : "";
  if (name === "NotAllowedError") {
    return new PasskeyProblem("The check was cancelled, or took too long. Try again.");
  }
  if (name === "NotSupportedError" || name === "SecurityError") {
    return new PasskeyProblem(
      "This phone's browser can't make a passkey here. Set a screen lock on your phone first, and use Safari or Chrome.",
    );
  }
  if (name === "InvalidStateError") {
    return new PasskeyProblem("This phone already has a passkey for this PC. Try again.");
  }
  return new PasskeyProblem("This phone could not check that it is you. Try again.");
}

/** Make the passkey for this phone (`navigator.credentials.create()`), as the PC asked. */
export async function makePasskey(request: PasskeyRequest): Promise<NewPasskey> {
  let credential: Credential | null;
  try {
    credential = await navigator.credentials.create({
      publicKey: {
        challenge: buf(bytes(request.challenge, "challenge")),
        rp: { id: request.rpId, name: "Plenipo" },
        user: {
          id: buf(bytes(request.user, "user")),
          name: request.userName,
          displayName: request.userName,
        },
        pubKeyCredParams: [
          { type: "public-key", alg: -7 },
          { type: "public-key", alg: -8 },
        ],
        authenticatorSelection: {
          authenticatorAttachment: "platform",
          userVerification: "required",
          residentKey: "preferred",
        },
        attestation: "none",
        timeout: 120_000,
      },
    });
  } catch (e) {
    throw explain(e);
  }
  if (!(credential instanceof PublicKeyCredential)) {
    throw new PasskeyProblem("This phone did not make a passkey. Try again.");
  }
  const response = credential.response as AuthenticatorAttestationResponse;
  const publicKey = response.getPublicKey?.();
  if (!publicKey) {
    throw new PasskeyProblem(
      "This phone's browser is too old for Plenipo. Update it, then pair again.",
    );
  }
  return {
    id: encode(new Uint8Array(credential.rawId)),
    publicKey: encode(new Uint8Array(publicKey)),
    algorithm: response.getPublicKeyAlgorithm(),
    authenticatorData: encode(new Uint8Array(response.getAuthenticatorData())),
    clientData: encode(new Uint8Array(response.clientDataJSON)),
  };
}

/** Sign in: the passkey's answer to the PC's challenge (`navigator.credentials.get()`). */
export async function answerChallenge(
  challenge: string,
  credentialId: string,
): Promise<PasskeyAnswer> {
  let credential: Credential | null;
  try {
    credential = await navigator.credentials.get({
      publicKey: {
        challenge: buf(bytes(challenge, "challenge")),
        allowCredentials: [{ type: "public-key", id: buf(bytes(credentialId, "passkey")) }],
        userVerification: "required",
        timeout: 120_000,
      },
    });
  } catch (e) {
    throw explain(e);
  }
  if (!(credential instanceof PublicKeyCredential)) {
    throw new PasskeyProblem("This phone did not answer. Try again.");
  }
  const response = credential.response as AuthenticatorAssertionResponse;
  return {
    id: encode(new Uint8Array(credential.rawId)),
    authenticatorData: encode(new Uint8Array(response.authenticatorData)),
    clientData: encode(new Uint8Array(response.clientDataJSON)),
    signature: encode(new Uint8Array(response.signature)),
  };
}
