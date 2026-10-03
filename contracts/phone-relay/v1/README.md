# The phone relay — contract v1

This is the written agreement between the two sides of Plenipo on your phone: the PC and the
phone's page on one side, and **Plenipo's own relay** on the other (`crates/relay`, run by 8 West at
`relay.getplenipo.com`; ADR-149, Plenipo runs its own relay). Plenipo is made by 8 West Ventures,
LLC.

Both sides speak from one piece of code, `crates/relay-contract` (the relay's messages and codes,
passes, fingerprints, and base64url), so they cannot drift apart. The PC's side
(`crates/remote/src/contract.rs`) writes the JSON files in this folder from the code and fails if
they drift; the relay's tests (`crates/relay/tests/contract.rs`) read them back and check every
example message, the example pass, and the example proof. The relay's own hardening is written up
in `crates/relay/src/lib.rs`, and its setup in [`crates/relay/deploy/README.md`](../../../crates/relay/deploy/README.md).

**In plain words:** a Pro copy of Plenipo connects out to the relay, and the owner's phones
connect too. The relay passes **sealed** messages between a PC and its own phones, and does
nothing else with them. It cannot read them. The PC proves who it is with its own key and shows
8 West's signed weekly answer, so the relay knows it is Pro. A phone shows a **pass** its PC
signed. This file never holds the relay's server address or any sign-in.

Decisions: ADR-143 (the relay and the lock), ADR-146 (where the phone's page lives), ADR-147
(passes last 90 days), ADR-149 (Plenipo runs its own relay; it replaces the earlier
[change request for Milepost's relay](../../../docs/phases/phase-14-relay-change-request.md)).

## Where

- PCs: `wss://relay.getplenipo.com/plenipo/v1/pc`
- Phones: `wss://relay.getplenipo.com/plenipo/v1/phone`

`relay.getplenipo.com` is Plenipo's name for its relay (a Cloudflare record the owner points at 8
West's server; a proxy there ends TLS and hands the connection to the relay, which listens on that
machine only). WebSockets over HTTPS. `GET /healthz` on the same name answers `ok` in plain text
(or `off` while the relay's off switch is on); every other plain request gets `404`. Each message is one **text** frame holding one JSON object with a
`t` field. A message is at most 96 KB; a sealed message (`data`) is at most 65,535 bytes before
base64url.

**base64url** means RFC 4648 §5, with no `=` padding, everywhere.

## A PC

1. The relay sends `{"t":"challenge","nonce":"<32 random bytes>"}`.
2. The PC answers `{"t":"hello","v":1,"key":"<Ed25519 public key>","proof":"<signature>","answer":{"answer":"…","signature":"…"}}`:
   - `key`: the PC's own relay key (32 bytes).
   - `proof`: an Ed25519 signature by that key over the ASCII bytes of `plenipo-relay-pc.v1.`
     followed by the nonce's text, exactly as sent.
   - `answer`: 8 West's newest signed weekly answer for the PC's license, exactly as the PC keeps
     it ([`contracts/license-check/v1`](../../license-check/v1)). The relay checks it with 8 West's
     public license keys (the ones in `crates/licensing/src/trust.rs`): the signature is right;
     `state` is `active`, or `cancelled` with `ends_at` still ahead; and `as_of` is less than 30
     days ago. The relay must not keep the key ID. (To count the PCs on one license, it keeps a
     salted hash of the key ID in memory while the PC is connected, with a salt made when the
     relay starts and known to nothing else: it cannot be turned back into the key ID, and it
     matches nothing outside that one run of the relay.)
3. The relay answers `{"t":"welcome","pc":"<fingerprint>"}`, where the fingerprint is SHA-256 of
   the key's 32 bytes, base64url (43 characters); or `{"t":"refused","code":"…"}` and closes.
4. From then on, the PC may send:
   - `{"t":"mailbox","mailbox":"<32 hex digits>"}`: open the pairing mailbox (one at a time; a
     new one replaces the old);
   - `{"t":"close_mailbox"}`;
   - `{"t":"drop","phone":"<phone ID>","until":<Unix seconds>}`: send each of that phone's
     connections `{"t":"refused","code":"bad_pass"}` and close it now, and refuse its pass until
     then (the relay may forget this when the PC disconnects; the PC drops it again if the phone
     comes back);
   - `{"t":"send","conn":"<connection>","data":"<sealed>"}`: one sealed message to one of its
     phones' connections;
   - `{"t":"close","conn":"<connection>"}`.
5. And the relay sends:
   - `{"t":"joined","conn":"<connection>","phone":"<phone ID>"}`: a phone with a good pass
     connected; or `{"t":"joined","conn":"<connection>","mailbox":true}`: a phone joined the
     pairing mailbox;
   - `{"t":"data","conn":"<connection>","data":"<sealed>"}`: a sealed message from that phone;
   - `{"t":"left","conn":"<connection>"}`;
   - `{"t":"error","code":"…"}`: something the PC sent was not passed on.

One live connection per PC key: a new one replaces the old. When the PC's connection ends, each of
its phones gets `{"t":"pc_offline"}` and is closed. Nothing is stored or queued, ever.

## A phone

1. The phone's first message is `{"t":"pass","pass":"<pass>"}` (a paired phone) or
   `{"t":"mailbox","mailbox":"<32 hex digits>"}` (pairing).
2. The relay answers `{"t":"ready"}`, or `{"t":"refused","code":"…"}` and closes.
3. Then `{"t":"data","data":"<sealed>"}` both ways. If the PC goes away, the relay sends
   `{"t":"pc_offline"}` and closes. If the PC drops this phone's pass, the relay sends
   `{"t":"refused","code":"bad_pass"}` and closes.

A pairing mailbox takes at most 3 phone connections in all, and only while the PC keeps it open
(at most 10 minutes).

## The pass

`<payload>.<signature>`, both base64url:

- `payload` is JSON `{"v":1,"pc":"<fingerprint>","phone":"<16 random bytes>","exp":<Unix seconds>}`;
- `signature` is the PC's relay key's Ed25519 signature over the ASCII bytes of
  `plenipo-relay-pass.v1.` followed by the payload's text, exactly as sent.

The relay finds the PC by `pc`, checks the signature with the key that PC showed when it
connected, and refuses a pass that is expired (`exp`) or dropped. A pass lasts 90 days and the PC
renews it at every sign-in (ADR-147). A pass is at most 512 characters.

## Codes

| Code             | Meaning                                                  |
| ---------------- | -------------------------------------------------------- |
| `not_pro`        | The weekly answer did not show a paid, current license   |
| `bad_proof`      | The PC's proof did not check                             |
| `bad_hello`      | The PC's hello could not be read                         |
| `pc_offline`     | The phone's PC is not connected                          |
| `bad_pass`       | The pass did not check, expired, or was dropped          |
| `mailbox_closed` | No PC is waiting at that mailbox                         |
| `too_many_tries` | Too many connections or tries; wait                      |
| `too_big`        | A message was over the limit                             |
| `unknown_conn`   | The PC named a connection that is not one of its phones' |

## Limits the relay keeps

The relay's defaults (`crates/relay/src/limits.rs`; the operator may change them):

| Limit                                                                    | Default                   |
| ------------------------------------------------------------------------ | ------------------------- |
| Open connections in all (over it, the door answers `503`)                | 512                       |
| Open connections from one address (IPv6 by its /64; over it, `429`)      | 32                        |
| New connections from one address in a minute                             | 120                       |
| Refusals for one address in a minute (then `429` for the rest of it)     | 30                        |
| Phone connections one PC may have at once (`too_many_tries` beyond)      | 40                        |
| PCs one license may have connected at once (`too_many_tries` beyond)     | 10                        |
| PCs one address may have connected at once (`too_many_tries` beyond)     | 8                         |
| Messages one connection may send in a minute (`too_many_tries`, closed)  | 1,200                     |
| Bytes one connection may send in a minute                                | 16 MB                     |
| Bytes waiting to go out to one connection (over it, the peer is closed)  | 1 MB                      |
| Time for the first message (the PC's hello, the phone's pass or mailbox) | 10 seconds                |
| A connection quiet this long (not even a pong) is closed                 | 90 seconds (pings at 30)  |
| A pairing mailbox stays open at most                                     | 10 minutes, 3 connections |
| Dropped passes remembered per PC                                         | 1,000                     |

An **off switch** turns the relay off without touching the machine's other services: every
connection closes and new ones are turned away (`503`) until it is on again. Logs hold counts,
codes, and addresses only: never a message, a pass, a weekly answer, a key ID, a mailbox name, or a
challenge. The relay stores nothing on disk.

## Inside the sealed messages (for the phone's page, not the relay)

The relay never reads these; they are here so the phone's page and the PC agree.

- **Pairing:** `Noise_XXpsk3_25519_AESGCM_SHA256`, prologue `plenipo-remote.v1/pair`. The phone
  starts. The shared key is made from the pairing code with HKDF-SHA256 (salt
  `plenipo-remote-pairing.v1`, info `psk`, 32 bytes), and the mailbox name with info `mailbox` (16
  bytes, lower-case hex). [`pairing-code.json`](pairing-code.json) has an example.
- **Every meeting after:** `Noise_KK_25519_AESGCM_SHA256`, prologue
  `plenipo-remote.v1/kk/<PC fingerprint>/<phone ID>`. The phone starts.
- **Pieces:** each sealed message's plain text starts with one byte: `1` when more pieces follow,
  `0` for the last (or only) piece. The rest of the pieces, put together, is one JSON object.
- [`noise-vectors.json`](noise-vectors.json) has every message of both meetings, made with fixed
  keys, and sealed messages after them, including one in pieces.
- What the JSON objects say is in `crates/remote/src/protocol.rs` (and its TypeScript in
  `packages/types/src/generated`).

## Files

- [`relay-messages.json`](relay-messages.json): every relay message above, an example PC key and
  its fingerprint, and an example pass that checks with it.
- [`pairing-code.json`](pairing-code.json): a code, and its mailbox name and shared key.
- [`noise-vectors.json`](noise-vectors.json): the lock's fixed test answers.
