# The weekly license check — contract v1

This is the one written agreement between Plenipo (the app, this repository) and the 8 West
account service (`Seckcey/plenipo-account`, private). Plenipo is made by 8 West Ventures, LLC.

Both sides test against the files in this folder. Plenipo's side is
`crates/licensing/src/contract.rs`, which writes these files from the code and fails if they
drift. The account service keeps a copy, pinned to the commit it was tested with (ADR-101 §3).

**In plain words:** once a week, a Pro copy of Plenipo sends 8 West its license key's ID and its
version number, and nothing else. 8 West answers whether the subscription is still paid, and signs
the answer so it can't be faked. A Free copy never sends anything.

## What Plenipo sends

- `POST https://account.getplenipo.com/v1/check`. The address is built into every copy and never
  changes (ADR-105).
- Headers: `Content-Type: application/json`, `Accept: */*`, `User-Agent: Plenipo/<version>`, and
  the ones HTTP itself needs (`Host`, `Content-Length`). No cookies, no sign-in, no other headers.
- Body: exactly `{"key_id":"<key ID>","app_version":"<version>"}`. That means those two fields,
  in that order, with no spaces and no trailing newline. [`request.json`](request.json) is the
  example, byte for byte.
- At most once every 7 days after a successful check. After a failed check, it tries again after
  1, 3, 6, 12, and then every 24 hours.
- Redirects are never followed. A redirect counts as a failed check.

## What the service answers

`200 OK` with `Content-Type: application/json` and the body:

```json
{ "answer": "<payload, base64url>", "signature": "<signature, base64url>" }
```

(No spaces in the real body. The files in [`answers/`](answers) are the examples, byte for byte.)

- **base64url** means RFC 4648 §5, with no `=` padding.
- **The payload** is JSON with exactly these fields:

  | Field          | Type           | Meaning                                                                |
  | -------------- | -------------- | ---------------------------------------------------------------------- |
  | `v`            | number         | `1`                                                                    |
  | `key_id`       | string         | The key asked about                                                    |
  | `state`        | string         | `active`, `cancelled`, `ended`, or `unknown`                           |
  | `paid_through` | number or null | Unix seconds                                                           |
  | `ends_at`      | number or null | When Pro ends (`cancelled`, required) or ended (`ended`), Unix seconds |
  | `as_of`        | number         | The service's clock when it answered, Unix seconds                     |
  | `signer`       | string         | Which signing key signed it                                            |

- **The signature** is Ed25519 over the ASCII bytes of `plenipo-license-answer.v1.` followed by
  the payload's base64url text, exactly as sent. It is made by the signing key in AWS KMS
  (`ED25519_SHA_512`, message type `RAW`, ADR-104). The service may reuse one signed answer per
  key per day.
- `state`:
  - `active`: paid, or Stripe is still retrying a failed payment.
  - `cancelled`: cancelled; Pro stays until `ends_at`, the end of the paid period.
  - `ended`: over; Pro ends now.
  - `unknown`: the service doesn't know this key. Plenipo treats this as a failed check, not as
    "ended".

Anything else counts as a failed check, and Pro stays on (fail-open, ADR-022 §3). That includes
no answer, another status, a body that doesn't parse, a wrong signature, a key that isn't
trusted, or an answer about another key. The service should answer `400` to any other body,
`415` to another content type, and `429` when asked too often.

## How Plenipo decides

Pro drops in exactly two cases:

- the newest signed answer is `ended`, or is `cancelled` and past `ends_at`;
- 30 days pass after the newest signed answer's `as_of` with no new one.

Plenipo's "now" is never earlier than the newest `as_of`, or than any clock time it has seen.
So winding the clock back never extends the 30 days, and replaying an old answer never resets
them (ADR-116).

## The license key

The key is `plenipo1.<payload>.<signature>`, with both parts in base64url.

The payload is JSON with exactly these fields:

| Field          | Type   | Meaning                                                                     |
| -------------- | ------ | --------------------------------------------------------------------------- |
| `v`            | number | `1`                                                                         |
| `edition`      | string | `pro`                                                                       |
| `key_id`       | string | `lk_` and 26 characters of Crockford base 32, capitals (no I, L, O, U)      |
| `holder`       | string | The buyer's name or company as they typed it; 1–120 characters; never email |
| `plan`         | string | `monthly` or `yearly`                                                       |
| `paid_through` | number | Unix seconds                                                                |
| `issued_at`    | number | Unix seconds                                                                |
| `signer`       | string | Which signing key signed it                                                 |

The signature is Ed25519 over `plenipo-license-key.v1.` followed by the payload's base64url
text. The two context strings keep a key from ever passing as an answer, and the other way
round. Plenipo removes spaces and line breaks an email may add before checking a key.

## Signing keys

Plenipo trusts the public halves of 8 West's production keys (the key in use and one spare) by
their `signer` name. The private halves never leave AWS KMS (ADR-104).

[`test-signing-key.json`](test-signing-key.json) is **the test key**. Its private half is
published on purpose, so both sides can make and check the examples. Only copies of Plenipo built
for the tests trust it, never a release build.

## The example files

| File                                                                          | What it is                                                  |
| ----------------------------------------------------------------------------- | ----------------------------------------------------------- |
| [`request.json`](request.json)                                                | The exact body Plenipo sends                                |
| [`keys/valid.txt`](keys/valid.txt)                                            | A valid key (the test key signed it)                        |
| [`keys/tampered.txt`](keys/tampered.txt)                                      | A changed payload under the valid key's signature: refused  |
| [`keys/unknown-signer.txt`](keys/unknown-signer.txt)                          | Signed by a key Plenipo doesn't trust: refused              |
| [`answers/active.json`](answers/active.json), `cancelled`, `ended`, `unknown` | Signed answers, one per state                               |
| [`answers/wrong-signature.json`](answers/wrong-signature.json)                | Signed by another key claiming the test key: refused        |
| [`schema/`](schema)                                                           | JSON Schemas for the request, the answer body, and payloads |

A change to this contract is a new folder (`v2`), never an edit to `v1` once released.
