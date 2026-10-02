# The phone relay — contract v1

This is the written agreement between Plenipo (the app and its phone page, this repository) and
8 West's relay (the one Milepost uses, in its own repository). Plenipo is made by 8 West Ventures,
LLC.

Plenipo's side is `crates/remote/src/contract.rs`, which writes the JSON files in this folder from
the code and fails if they drift. The relay pins the contract's version it was tested with. The
change the relay needs is described in plain words in
[`docs/phases/phase-14-relay-change-request.md`](../../../docs/phases/phase-14-relay-change-request.md).

**In plain words:** a Pro copy of Plenipo connects out to the relay, and the owner's phones
connect too. The relay passes **sealed** messages between a PC and its own phones, and does
nothing else with them. It cannot read them. The PC proves who it is with its own key and shows
8 West's signed weekly answer, so the relay knows it is Pro. A phone shows a **pass** its PC
signed. This file never holds the relay's real address or any sign-in.

Decisions: ADR-143 (the relay and the lock), ADR-146 (where the phone's page lives), ADR-147
(passes last 90 days).

## Where

- PCs: `wss://relay.getplenipo.com/plenipo/v1/pc`
- Phones: `wss://relay.getplenipo.com/plenipo/v1/phone`

`relay.getplenipo.com` is Plenipo's own name for the relay (a Cloudflare record the owner points
at it). WebSockets over HTTPS. Each message is one **text** frame holding one JSON object with a
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
     days ago. The relay must not keep the key ID.
3. The relay answers `{"t":"welcome","pc":"<fingerprint>"}`, where the fingerprint is SHA-256 of
   the key's 32 bytes, base64url (43 characters); or `{"t":"refused","code":"…"}` and closes.
4. From then on, the PC may send:
   - `{"t":"mailbox","mailbox":"<32 hex digits>"}`: open the pairing mailbox (one at a time; a
     new one replaces the old);
   - `{"t":"close_mailbox"}`;
   - `{"t":"drop","phone":"<phone ID>","until":<Unix seconds>}`: close that phone's connections
     now, and refuse its pass until then;
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
   `{"t":"pc_offline"}` and closes.

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

Connections and tries from each internet address (IPv6 counted by its /64), and per PC; messages
and bytes per PC and per phone, per minute; a cap on Plenipo's connections and memory in all; and
a switch that turns Plenipo's part off without touching Milepost. Logs hold counts and errors only:
never a message, a pass, a weekly answer, a key ID, or a challenge.

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
