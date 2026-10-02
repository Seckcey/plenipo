# Plenipo Community — contract v1

This is the written agreement between Plenipo (the app, this repository) and the 8 West account
service (`Seckcey/plenipo-account`, private) about **Community**: signing in to your 8 West
account from Plenipo, profiles and the directory, sealed private messages, blocks, linked
organizations, collaborators, reports, points, and GIFs. Plenipo is made by 8 West Ventures, LLC.

The decisions behind it are ADR-161 to ADR-169 (Phase 24). The account service keeps a copy of
this folder, pinned to the commit it was tested with (ADR-101 §3), and tests its answers against
the schemas and examples here. Plenipo's side (part 24C) tests against the same files.

**In plain words:** Plenipo signs in to your 8 West account with a short code you type on the
account site. Each of your PCs makes its own keys, and tells 8 West only the public halves. A
message is sealed on your PC for the other person's PCs, so 8 West carries it but cannot read it.
8 West stamps each sealed message when it takes it, so if the person who got it reports it, 8 West
can tell the report is real. Everything else here is the small set of facts 8 West needs to run
Community: who is a member, who blocked whom, which organizations are linked, and points.

**Status:** v1 is not released yet. Until a Plenipo release uses it, v1 may still change, with a
new pinned copy in the account service each time. After that, a change is a new folder (`v2`).

## Contents

1. [How to talk to the service](#1-how-to-talk-to-the-service)
2. [Signing in from a PC](#2-signing-in-from-a-pc)
3. [You, your membership, and your profile](#3-you-your-membership-and-your-profile)
4. [Finding people](#4-finding-people)
5. [Conversations](#5-conversations)
6. [Sealed items: sending, picking up, and the stamp](#6-sealed-items-sending-picking-up-and-the-stamp)
7. [What is inside a seal](#7-what-is-inside-a-seal)
8. [Blocks](#8-blocks)
9. [Linked organizations](#9-linked-organizations)
10. [Collaborators](#10-collaborators)
11. [Reports](#11-reports)
12. [Thanks, points, badges, and the leaderboard](#12-thanks-points-badges-and-the-leaderboard)
13. [Inviting by email](#13-inviting-by-email)
14. [GIFs and stickers](#14-gifs-and-stickers)
15. [Limits](#15-limits)
16. [The files in this folder](#16-the-files-in-this-folder)

## 1. How to talk to the service

- **Address:** `https://account.getplenipo.com`, built into every copy (ADR-105). Every path below
  starts with `/v1/community/`. Plenipo never follows a redirect; a redirect is a failure.
- **Headers from Plenipo:** `Accept: application/json`, `User-Agent: Plenipo/<version>`, and, with
  a body, `Content-Type: application/json` (UTF-8). Every request except the two sign-in requests
  (§2) carries `Authorization: Bearer <pass>`, where the pass is the Community pass this PC got
  when it signed in. No cookies.
- **Bodies:** JSON objects. A request with a field this contract does not name is refused
  (`bad_request`). Plenipo ignores fields it does not know in an answer, so the service can add
  fields without a new version.
- **Times** are whole Unix seconds. **Keys, signatures, and sealed bytes** are base64url (RFC 4648
  §5) with no `=` padding. **Text** is UTF-8, counted in Unicode characters (code points).
- **IDs** are a prefix, `_`, and 26 characters of Crockford base 32 in capitals (`0-9`, `A-Z`
  without `I`, `L`, `O`, `U`), made at random:

  | Prefix | What                                                    | Made by             |
  | ------ | ------------------------------------------------------- | ------------------- |
  | `cm_`  | a member of Community                                   | the service         |
  | `cd_`  | a PC signed in to Community                             | the service         |
  | `ci_`  | a sealed item (a message, an objective, …)              | the sending PC      |
  | `cl_`  | a link between two organizations                        | the service         |
  | `cc_`  | a collaboration (a person helping with an organization) | the service         |
  | `cr_`  | a report                                                | the service         |
  | `co_`  | an organization, as one link or collaboration knows it  | the PC that owns it |

  A PC makes a fresh `co_` for each link and each collaboration, never its organization's real ID,
  so the service cannot tell that two links are the same organization.

- **When something goes wrong** the answer is not 2xx, with this body:

  ```json
  { "error": "<code>", "message": "<one plain sentence Plenipo may show>" }
  ```

  `429` and some `503` answers carry `Retry-After` (seconds). The codes:

  | Code                       | Status | Meaning                                                                              |
  | -------------------------- | ------ | ------------------------------------------------------------------------------------ |
  | `bad_request`              | 400    | The body or a value is not what this contract says                                   |
  | `too_large`                | 400    | A text, a picture, or a sealed copy is over its limit (§15)                          |
  | `unauthorized`             | 401    | No pass, or a pass the service doesn't know. The PC signs in again                   |
  | `not_member`               | 403    | This account hasn't joined Community (§3)                                            |
  | `email_not_confirmed`      | 403    | The account's email isn't confirmed yet                                              |
  | `account_limited`          | 403    | 8 West limited this account                                                          |
  | `too_young`                | 403    | Under 13. Nothing was kept                                                           |
  | `adults_only`              | 403    | Starting things needs 18 or older                                                    |
  | `needs_pro`                | 403    | Starting things, or linking, needs Plenipo Pro (on both sides, for a link)           |
  | `community_paused`         | 403    | 8 West paused this member's Community for now                                        |
  | `community_ended`          | 403    | 8 West ended this member's Community                                                 |
  | `terms_changed`            | 409    | The Community terms changed; the member must accept them again (§3)                  |
  | `not_found`                | 404    | No such thing, or not one this member may see. Also what a blocked person gets       |
  | `not_delivered`            | 403    | The other person can't get this (blocked, left the conversation, or no PC signed in) |
  | `waiting_for_accept`       | 409    | They haven't accepted the first message yet (§5)                                     |
  | `devices_changed`          | 409    | The sealed copies don't match the PCs signed in now. Fetch them again and seal again |
  | `not_accepting_objectives` | 403    | The other owner paused objectives on this link                                       |
  | `item_id_reused`           | 409    | That item ID was already used for a different item                                   |
  | `already_member`           | 409    | This account already joined                                                          |
  | `name_taken`               | 409    | Someone has that Community name, or it is held after someone left                    |
  | `name_not_allowed`         | 400    | The name breaks the rules in §3                                                      |
  | `name_change_too_soon`     | 409    | The name was changed in the last 30 days                                             |
  | `too_many_devices`         | 409    | Already 5 PCs signed in for this account                                             |
  | `waiting`                  | 400    | Sign-in: the person hasn't pressed **Allow** yet                                     |
  | `slow_down`                | 400    | Sign-in: asked sooner than `interval`; wait 5 more seconds each time                 |
  | `denied`                   | 400    | Sign-in: the person pressed **Don't allow**                                          |
  | `expired`                  | 400    | Sign-in: the code ran out (10 minutes)                                               |
  | `proof_failed`             | 400    | A reported item's proof doesn't check out (§11)                                      |
  | `already_thanked`          | 409    | Thanked this person in the last 7 days                                               |
  | `too_new_to_thank`         | 403    | Members can thank after 7 days in Community                                          |
  | `too_many`                 | 429    | A limit in §15. Wait `Retry-After` seconds                                           |
  | `gifs_unavailable`         | 503    | GIF search isn't available (§14)                                                     |
  | `unavailable`              | 503    | Community isn't open, or the service is busy. Nothing was changed                    |

- **The JSON Schemas** for every body are in [`schema/community.schema.json`](schema/community.schema.json),
  one `$defs` entry each, named in the sections below. Every file in [`examples/`](examples)
  passes the schema named in [`examples/index.json`](examples/index.json).

## 2. Signing in from a PC

Plenipo never sees a password. It signs in the way a TV does (ADR-162 §2):

1. **Start.** `POST /v1/community/sign-in/start`, no pass. Body `SignInStart`:

   | Field         | What                                                                                                    |
   | ------------- | ------------------------------------------------------------------------------------------------------- |
   | `device_name` | The PC's name as the person will see it, 1–60 characters, no control characters or text-direction marks |
   | `app_version` | Plenipo's version, like `1.20.0`                                                                        |
   | `signing_key` | This PC's new Ed25519 public key, 32 bytes, base64url                                                   |
   | `sealing_key` | This PC's new X25519 public key, 32 bytes, base64url                                                    |

   The PC makes both key pairs first and keeps the private halves in the Vault. Answer
   `SignInStarted`: `device_code` (secret; 43 characters), `user_code` (like `4KQ-7TD`: 6 of
   `BCDFGHJKLMNPQRSTVWXZ23456789`, with a dash in the middle), `verification_uri`
   (`https://account.getplenipo.com/community/connect`), `expires_in` (600), and `interval` (5).

2. **The person allows it**, in their own web browser: Plenipo shows "**Enter this code: 4KQ-7TD**"
   and **Open the sign-in page**. On the account site, signed in as usual, the person types the
   code, sees "**Plenipo on FRANKIE-DESKTOP wants to use Community as you**", and presses **Allow**
   or **Don't allow**. An account with 5 PCs already is asked to remove one first. Every time a PC
   starts using Community as an account, 8 West emails the account, so a code typed by mistake (or
   by trickery) is noticed at once.
3. **Finish.** Every `interval` seconds, `POST /v1/community/sign-in/token`, no pass. Body
   `SignInToken`: the `device_code`, and `proof`: this PC's Ed25519 signature, base64url, over the
   ASCII bytes of `plenipo-community-sign-in.v1.` followed by the `device_code`. Until the person
   answers: `400 waiting` (or `slow_down`). Then `denied` or `expired`, or `200` with
   `SignedIn`: the `pass` (43 characters; shown once, kept in the Vault, never logged), this PC's
   `device_id`, and `me` (§3).

The code works once. The pass lasts until the PC signs out (`POST /v1/community/sign-out`, answer
`204`), the person removes the PC on the account site (**Remove**), the member leaves Community,
or the account is deleted. Then every request with it is `401 unauthorized`.

## 3. You, your membership, and your profile

`GET /v1/community/me` answers `Me`: the account's name (`account.name`, for "**Signed in as Frank
Gonzalez**"), this PC's `device_id`, the account's `devices` (each `device_id`, `name`, `added_at`,
`last_seen_at`), the current Community terms version (`terms`), and `member`: `null` until the
account joins, otherwise:

| Field             | What                                                                               |
| ----------------- | ---------------------------------------------------------------------------------- |
| `member_id`       | `cm_…`                                                                             |
| `name`            | The Community name, without the `@`                                                |
| `joined_at`       | When                                                                               |
| `age_group`       | `adult` (18 or older) or `teen` (13 to 17)                                         |
| `standing`        | `ok`, `paused` (until `paused_until`), or `ended`                                  |
| `paused_until`    | Unix seconds, or `null`                                                            |
| `terms_accepted`  | The terms version this member accepted                                             |
| `can_start`       | Whether this member may start things: an adult, with Plenipo Pro, in good standing |
| `appear_offline`  | §4                                                                                 |
| `profile`         | `Profile`, below                                                                   |
| `has_picture`     | Whether a picture is shown                                                         |
| `picture_version` | Changes when the picture changes, or `null`                                        |
| `name_change_at`  | The earliest time the name may change again, or `null`                             |

**Joining:** `POST /v1/community/join`, body `Join`: `name`, `birth_month` (1–12), `birth_year`,
and `terms` (the version the person accepted on screen). Answer `Me`.

- **The age** (ADR-162 §4). Plenipo asks first, and says "**Community is for people 13 and
  older**" without sending anything if the answer is under 13. The service checks again and
  answers `too_young`, keeping nothing. A birthday counts only once its month has passed, so a
  person turns 13 (and 18) the month after their birth month. The service keeps the month and year
  so that a member who turns 18 becomes an adult member by themselves, and so it never asks again.
  Changing them later goes through 8 West by email.
- **The name:** 3 to 30 of `a-z`, `0-9`, and `-`, starting and ending with a letter or a number,
  unique, and not one that copies 8 West or Plenipo (`8west`, `plenipo`, `support`, `admin`, and
  the like: `name_not_allowed`). A name is held for 90 days after its member leaves.
  `PUT /v1/community/me/name`, body `{"name": "…"}`, changes it at most once every 30 days.
- **The terms:** when 8 West changes the Community terms, `terms` in `Me` changes and every request
  that sends or starts anything answers `terms_changed` until the member accepts the new ones:
  `PUT /v1/community/me/terms`, body `{"terms": "<version>"}`, answer `Me`.

**The profile** (ADR-163) is `Profile`. Every field is `null` (or `[]`) when the member hides it:

| Field            | What                                                                                                           |
| ---------------- | -------------------------------------------------------------------------------------------------------------- |
| `display_name`   | The name on the tile, 1–60 characters                                                                          |
| `status`         | `available`, `busy`, or `away`. Plenipo sends `busy` for Do not disturb                                        |
| `mood`           | `great`, `good`, `okay`, `tired`, `stressed`, `focused`, or `celebrating`                                      |
| `message`        | Up to 80 characters, one line                                                                                  |
| `company`        | Up to 80 characters                                                                                            |
| `business_kinds` | Up to 3 of the kinds in the schema (`construction`, `accounting`, …), no repeats                               |
| `business_line`  | What the business does, in the member's words, up to 80 characters                                             |
| `region`         | A country (ISO 3166-1 alpha-2, like `US`) or a US state (ISO 3166-2, like `US-CA`). Never a town or an address |

`PUT /v1/community/me/profile` with the whole `Profile` replaces it; answer `Me`. Texts are one
line, with no control characters.

**The picture:** `PUT /v1/community/me/picture`, body `{"png": "<base64url>"}`: a PNG of at most
256 × 256 pixels and 256 KB, not interlaced. The service checks that it is a real PNG (the chunks,
their checksums, and that the image data unpacks to exactly the size the header says), keeps only
the image itself (no text or other extra chunks), and answers `Me`. `DELETE
/v1/community/me/picture` removes it.

**Appear offline** (ADR-163 §5): `PUT /v1/community/me/presence`, body `{"appear_offline": true}`.
The member leaves the directory, **New this week**, and the leaderboard at once; people who know
them see **Offline**; and a look-up of their name answers only `request_only` (§4). `false` lists
them again. Messages, links, and collaborations keep working.

**Leaving Community** (ADR-167 §15): `DELETE /v1/community/me`, answer `204`. Every PC of the
member is signed out, and their profile, picture, listing, sealed items still waiting, links,
collaborations, blocks they made, points, badges, and thanks are deleted at once (ADR-168). The
name is held for 90 days. Joining again later keeps the same `member_id`, so blocks other people
made still apply. Deleting the 8 West account removes everything at once.

## 4. Finding people

A **card** (`Card`) is what one member may see of another: `member_id`, `name`, the profile fields
the member shows, `status` (also `offline`), `has_picture`, `picture_version`, `badges`, `points`
(all time), and `thanked_by` (how many different people thanked them).

- **The directory** (ADR-163 §4): `GET /v1/community/directory?q=&kind=&region=&cursor=`, answer
  `CardPage` (`cards`, 20 at a time, and `next`, a cursor or `null`). `q` (up to 60 characters)
  matches the start of a Community name, or any part of a display name, company, or business line,
  ignoring case; `kind` is one business kind; `region` is a country (which also matches its states)
  or a state. Listed: adult members, in good standing, not appearing offline. Most points first.
- **New this week:** `GET /v1/community/directory/new?cursor=`, the same, for members who joined in
  the last 7 days.
- **A name:** `GET /v1/community/people/by-name/{name}` answers the `Card`, or, for a member under
  18 or one appearing offline, `RequestOnly`: just `member_id`, `name`, and `"request_only": true`,
  so the PC can offer **Send a message request** and nothing more.
- **One member:** `GET /v1/community/people/{member_id}` answers their `Card` when this member may
  see it: anyone listed in the directory; and, for a member under 18 or appearing offline, only the
  people they talk with (§5), link with, or collaborate with. Adults never see the status or mood
  of a member under 18 (`null`).
- **A picture:** `GET /v1/community/people/{member_id}/picture` answers `image/png` under the same
  rule, with `ETag` set to the `picture_version`.
- **A member's PCs:** `GET /v1/community/people/{member_id}/devices` answers `Devices`: each PC's
  `device_id`, `signing_key`, and `sealing_key`. Any member may fetch them for anyone they could
  send to (not blocked either way), and for themselves (`people/{own member_id}/devices`).

The service limits look-ups (§15), so nobody can copy the directory whole. A blocked person gets
`not_found` for everything about the member who blocked them, the same as for nobody at all.

## 5. Conversations

Two members **talk** once one has accepted the other's first message (ADR-164 §7):

- A first `message` (§6) to someone this member doesn't talk with is a **request**. It needs
  `can_start` (an adult, with Pro). It is delivered marked `"request": true`, and until it is
  accepted the sender can send nothing more to that person (`waiting_for_accept`).
- `POST /v1/community/contacts/{member_id}/accept` accepts it (`204`). A reply also accepts it. The
  sender gets the notice `contact_accepted` (§6).
- `DELETE /v1/community/contacts/{member_id}` is **Leave this conversation**: the other person's
  next items answer `not_delivered`, until this member writes to them again.
- `GET /v1/community/contacts` answers `Contacts`: each `member_id`, `name`, `display_name`,
  `state` (`requested_by_me`, `requested_by_them`, `accepted`, `left_by_me`, or `left_by_them`),
  and `since`.

## 6. Sealed items: sending, picking up, and the stamp

Everything one member's PC says to another member's PCs is a **sealed item**: messages,
reactions, link notes, objectives, their states, answers, and collaboration invitations. 8 West
sees only the **envelope**.

**Sending:** `POST /v1/community/items`, body `ItemSend`:

| Field     | What                                                                                                                                            |
| --------- | ----------------------------------------------------------------------------------------------------------------------------------------------- |
| `item_id` | `ci_…`, made by the sending PC. Sending the same item again (after a lost answer) gives the same answer                                         |
| `to`      | The member it is for                                                                                                                            |
| `kind`    | `message`, `reaction`, `link_note`, `objective`, `objective_state`, `answer`, or `collab_note`                                                  |
| `ref`     | The link (`cl_…`) for `link_note`, `objective`, `objective_state`, and `answer`; the collaboration (`cc_…`) for `collab_note`; otherwise `null` |
| `tag`     | The report proof's commitment, 32 bytes, base64url (§7)                                                                                         |
| `copies`  | One `{device_id, sealed}` for each PC of the receiver, and for each **other** PC of the sender; nothing for the sending PC                      |

- The copies must name exactly the PCs signed in now; otherwise `devices_changed`, and the PC
  fetches both lists again (§4) and seals again. A receiver with no PC signed in answers
  `not_delivered`.
- What each kind needs: `message` follows §5; `reaction` needs the two to talk; `link_note` needs a
  link that is asked for or active; `objective` and `answer` need an active link that the receiver
  hasn't paused, and Pro on both sides; `objective_state` needs a link that hasn't ended;
  `collab_note` needs a collaboration that is invited or active, sent by its owner.
- **Answer** `ItemSent`: `item_id`, `accepted_at`, `stamp`, and `request` (whether it was a
  request). The sending PC keeps the stamp with its own copy.

**The stamp** proves 8 West took this envelope, at this time. It is
`<payload>.<signature>`, both base64url. The payload is JSON (`StampPayload`) with exactly: `v`
(1), `item_id`, `from`, `to`, `kind`, `ref`, `tag`, `at` (when it was accepted), and `signer` (the
name of the stamping key). The signature is Ed25519 over the ASCII bytes of
`plenipo-community-stamp.v1.` followed by the payload's base64url text. The stamping key is the
account service's own, never the license signing key (ADR-164 §6).
[`test-stamping-key.json`](test-stamping-key.json) is the test key, published on purpose.

**Picking up:** `GET /v1/community/items?wait=<0–25>` answers `Inbox`: up to 50 `InboxItem`s for
this PC, oldest first, and `more`. With `wait`, the service holds the request open up to that many
seconds until something arrives. An `InboxItem` is `item_id`, `kind`, `from`, `ref`, `sealed` (this
PC's copy), `tag`, `stamp`, `accepted_at`, `request`, and `notice` (`null`).

**Notices** are items the service itself makes, not sealed: `kind` is `notice`, `from`, `sealed`,
`tag`, and `stamp` are `null`, and `notice` is `Notice`:

| `notice.type`        | Fields                     | When                                                               |
| -------------------- | -------------------------- | ------------------------------------------------------------------ |
| `contact_accepted`   | `member_id`                | Someone accepted this member's first message                       |
| `link_requested`     | `link_id`, `member_id`     | Someone asked to link (their note comes as a `link_note`)          |
| `link_accepted`      | `link_id`                  | The other owner accepted                                           |
| `link_paused`        | `link_id`, `paused`        | The other owner paused or unpaused objectives                      |
| `link_ended`         | `link_id`                  | Either side unlinked, or blocked                                   |
| `collab_invited`     | `collab_id`, `member_id`   | An owner invited this member (details come as a `collab_note`)     |
| `collab_accepted`    | `collab_id`                | The invited person accepted                                        |
| `collab_ended`       | `collab_id`                | Removed, left, lapsed, or ended by a block                         |
| `standing_changed`   | `standing`, `paused_until` | 8 West warned, paused, or ended this member's Community            |
| `report_closed`      | `report_id`, `outcome`     | 8 West finished a report this member made: `action` or `no_action` |
| `my_devices_changed` | none                       | A PC of this member signed in or out                               |

**Done:** `POST /v1/community/items/ack`, body `{"item_ids": [...]}` (up to 100), answer `204`.
The service deletes this PC's copy; when every copy of an item is picked up, the item is deleted.
An item nobody picks up is deleted after 30 days (ADR-168).

## 7. What is inside a seal

The service never opens a seal. This section is for the PCs, and for the report check in §11.

- **The seal:** HPKE (RFC 9180), base mode, single shot, with DHKEM(X25519, HKDF-SHA256) (`0x0020`),
  HKDF-SHA256 (`0x0001`), and AES-256-GCM (`0x0002`), to the receiving PC's `sealing_key`. `info`
  is the ASCII bytes of `plenipo-community-seal.v1.`, then the `item_id`, `.`, and the receiving
  `device_id`, so a copy opens only as that item, on that PC. `aad` is empty. `sealed` is `enc`
  (32 bytes) followed by the ciphertext.
- **What is sealed** (`SealedContent`) is JSON with exactly:
  - `payload`: the item's inner JSON (`ItemPayload`), base64url, exactly as signed;
  - `sig`: the sending PC's Ed25519 signature over the ASCII bytes of
    `plenipo-community-item.v1.` followed by `payload` (the base64url text);
  - `fk`: the **report key**, 32 random bytes made for this item, base64url.
- **The tag** on the envelope is HMAC-SHA256 with the report key over the ASCII bytes of
  `plenipo-community-report.v1.` followed by `payload` (the base64url text). Every copy of one item
  carries the same `payload`, `fk`, and tag.
- **`ItemPayload`** is JSON with `v` (1), `item_id`, `kind`, `from`, `from_device`, `to`, `ref`,
  `sent_at`, and `body`, which depends on the kind:

  | Kind              | `body`                                                                                                                                                                                                                      |
  | ----------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
  | `message`         | `text` (up to 4,000 characters, or `null`), `gif` (`{library, id}` or `null`), `sticker` (`{set, name}` or `null`), `reply_to` (`ci_…` or `null`). At least one of `text`, `gif`, `sticker`; never both `gif` and `sticker` |
  | `reaction`        | `item` (the `ci_…` reacted to), `emoji` (one emoji, or `null` to take it back)                                                                                                                                              |
  | `link_note`       | `org_name` (up to 80 characters), `note` (up to 500, or `null`)                                                                                                                                                             |
  | `objective`       | `org_name`, `text` (up to 20,000 characters)                                                                                                                                                                                |
  | `objective_state` | `objective` (its `ci_…`), `state` (`approved`, `refused`, `stopped`, or `finished`), `why` (up to 200, or `null`)                                                                                                           |
  | `answer`          | `objective` (its `ci_…`), `text` (up to 20,000 characters)                                                                                                                                                                  |
  | `collab_note`     | `org_name`, `role` (`viewer`, `approver`, or `manager`), `part` (`whole` or a list of up to 50 `{kind, name}` of departments and projects), `owner_only` (the kinds of approvals only the owner answers, ADR-166 §3)        |

- **The receiving PC checks, in order,** and drops the item if any check fails: the seal opens with
  this PC's key and `info`; `SealedContent` and `ItemPayload` parse; `from_device` is one of
  `from`'s PCs and `sig` checks with its `signing_key`; the HMAC over `payload` with `fk` equals the
  envelope's `tag`; and `item_id`, `kind`, `from`, `to`, and `ref` in the payload equal the
  envelope's. It keeps `payload`, `sig`, `fk`, and the stamp, so the person can report the item.
- **Shown safely:** every text is shown as text, never as a web page; hidden control characters
  that could disguise words (like right-to-left overrides) are shown as visible marks (ADR-164 §4).

[`examples/vector-item.json`](examples/vector-item.json) is a worked example: an item's payload, its
report key and tag, its signature by a test PC key, and its stamp by the test stamping key.

## 8. Blocks

`PUT /v1/community/blocks/{member_id}` blocks (`204`); `DELETE` unblocks (`204`);
`GET /v1/community/blocks` answers `Blocks` (each `member_id`, `name`, `blocked_at`). A block, at
once (ADR-167 §1–4): deletes the sealed items still waiting between the two, both ways; ends their
conversation (a new one starts with a new request); ends every link and collaboration between them
(with `link_ended` and `collab_ended` notices); and, from then on, gives the blocked person
`not_delivered` or `not_found` for everything about the member who blocked them. They are not told
they were blocked. Unblocking brings none of it back.

## 9. Linked organizations

ADR-165. The service knows only that two members linked, the `co_` each chose for their
organization, and the link's state. Names, notes, objectives, and answers travel sealed (§6, §7).

- `POST /v1/community/links`, body `{"to": "cm_…", "org_ref": "co_…"}`, asks (`can_start` needed),
  answer `Link`. The other member gets `link_requested`; the asking PC then sends a `link_note`.
- `POST /v1/community/links/{link_id}/accept`, body `{"org_ref": "co_…"}`, accepts (the accepting
  member needs `can_start`), answer `Link`. A request not accepted in 14 days lapses.
- `PUT /v1/community/links/{link_id}/paused`, body `{"paused": true}`, is **Don't accept objectives
  for now** on this member's side; answer `Link`.
- `DELETE /v1/community/links/{link_id}` unlinks, from either side, at once (`204`). Objectives
  still waiting in the mailbox for the other side are deleted.
- `GET /v1/community/links` answers `Links`. A `Link` is `link_id`, `other` (`cm_…`), `asked_by`
  (`me` or `them`), `my_org_ref`, `their_org_ref` (`null` until accepted), `state` (`requested`,
  `active`, or `ended`), `paused_by_me`, `paused_by_them`, `created_at`, `accepted_at`, and
  `ended_at`.
- When either side's Pro ends, objectives and answers on its links answer `needs_pro`; nothing is
  deleted.

## 10. Collaborators

ADR-166. The service knows only who invited whom for which `co_`, and the state. The role, the
part, and the approvals only the owner answers travel sealed in a `collab_note`; the collaborator's
own work goes through Plenipo's relay, sealed, never through this service.

- `POST /v1/community/collaborations`, body `{"to": "cm_…", "org_ref": "co_…"}`, invites
  (`can_start` needed; at most 10 invited or active for one `co_`), answer `Collaboration`. The
  invited member gets `collab_invited`; the owner's PC then sends a `collab_note`. An invitation
  lapses after 7 days.
- `POST /v1/community/collaborations/{collab_id}/accept` accepts (any member, 13 or older), answer
  `Collaboration`.
- `DELETE /v1/community/collaborations/{collab_id}` is **Remove** for the owner and **Leave** for
  the collaborator, at once (`204`).
- `GET /v1/community/collaborations` answers `Collaborations`. A `Collaboration` is `collab_id`,
  `owner`, `collaborator`, `org_ref`, `state` (`invited`, `active`, or `ended`), `created_at`,
  `accepted_at`, `ended_at`, and `expires_at` (for an invitation).

## 11. Reports

`POST /v1/community/reports`, body `Report`, answer `ReportMade` (`report_id`). ADR-167.

| Field    | What                                                                                                                     |
| -------- | ------------------------------------------------------------------------------------------------------------------------ |
| `about`  | The member reported                                                                                                      |
| `reason` | `spam`, `harassment`, `scam`, `hate`, `sexual`, `under_13`, `young_person_risk`, `impersonation`, `cheating`, or `other` |
| `note`   | Up to 1,000 characters, or `null`                                                                                        |
| `what`   | `person`, `profile` (the service keeps a copy of the card and picture as they are now), or `items`                       |
| `items`  | For `items`: 1 to 20 `{stamp, payload, fk}` the reporting member received from `about`. Otherwise `[]`                   |

For each item the service checks: the stamp's signature (by a stamping key it has used); the
stamp's `from` is `about` and its `to` is the reporting member; the HMAC of `payload` with `fk`
equals the stamp's `tag`; and `item_id`, `kind`, `from`, `to`, and `ref` in the payload equal the
stamp's. If any item fails, the whole report is refused with `proof_failed` and `item` (the index),
so a report can never carry words nobody sent. The service keeps the payloads it checked, as
evidence, for the time in ADR-168. The signature inside the seal is not needed: the stamp and the
tag already tie the words to the sender. When 8 West is done, the reporting member gets the
`report_closed` notice.

## 12. Thanks, points, badges, and the leaderboard

ADR-169. The service works out points and badges from its own records; a PC never sends points.

- **Thanks:** `POST /v1/community/thanks`, body `Thanks`: `to`, `for` (`link_answer` or
  `collaborator`), and `ref` (the `cl_…` or `cc_…` it was for, between the two). Once for each
  person each 7 days (`already_thanked`); only members 7 days in Community (`too_new_to_thank`).
  Answer `204`.
- **Points** go to a member for: a first message they sent being accepted (2, once for each
  person); a thanks (3); a person they invited joining Community (5); a link lasting 7 days (10, to
  each side); a collaboration lasting 7 days (10, to each side); finishing **Getting started** (10,
  once: shown profile parts, a conversation accepted, a link, and a collaboration); each month in
  Community with no report against them upheld (5); and a person they invited buying Pro and keeping
  it past 14 days (25). At most 100 a week (from Monday, Pacific time), and at most 20 in 30 days
  from any one other person. 8 West can change the numbers, and can take points away (ADR-167 §9).
- **Badges:** `founding_member`, `helper`, `connector`, `good_neighbor`, `trusted`, and
  `top_helper` (ADR-169 §3), worked out each day.
- `GET /v1/community/me/points` answers `Points`: `total`, `week`, `week_started_at`,
  `place_week`, `place_all`, `badges`, `thanked_by`, `recent` (the last 20 changes: `points`,
  `reason`, `at`), and `free_months` (`{this_year, max}`; `null` for a member under 18).
- `GET /v1/community/leaderboard?period=week|all` answers `Leaderboard`: `period`, `since`, `top`
  (up to 50: `place`, `member_id`, `name`, `display_name`, `has_picture`, `picture_version`,
  `badges`, `points`), and `me` (this member's `place` and `points`, `place` `null` when not on the
  board). Members under 18, appearing offline, or not in good standing are left out.

## 13. Inviting by email

`POST /v1/community/invite-email`, body `{"email": "…"}`, answer `202` with `{}` every time, so
nobody learns whether an address has an account (ADR-163 §6). Needs `can_start`. 8 West emails the
address a link to join, from `hello@getplenipo.com`, naming the member who invited them. The same
address gets at most one invitation every 30 days, from anyone. The link carries the invitation, so
the service knows who invited a new member (for points and the free month, ADR-169 §6).

## 14. GIFs and stickers

- **GIFs** come from one GIF library, through the service, which holds the library's key and keeps
  no search words (ADR-164 §4). `GET /v1/community/gifs?q=&offset=` answers `Gifs`: `library`,
  `attribution` (words Plenipo must show), `results` (each `id`, `width`, `height`, and `preview`,
  an `https` address on the library's picture host), and `next_offset`. Ratings: PG-13 or milder,
  and G or PG for a member under 18. A message carries only `{library, id}`; the receiving PC builds
  the picture's address from the table below, and Guard allows only that host.

  | `library` | Picture address                                                                                                                       | Host |
  | --------- | ------------------------------------------------------------------------------------------------------------------------------------- | ---- |
  | none yet  | The owner chooses the library before part 24C; until then the service answers `gifs_unavailable` and Plenipo hides the **GIF** button | —    |

- **Stickers** are Plenipo's own sets, built into each release; a message carries `{set, name}`,
  and nothing is fetched.

## 15. Limits

The service enforces these; the numbers may change without a new version, and Plenipo shows the
`message` of a `too_many` answer.

| What                                       | Limit                                                                                                                     |
| ------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------- |
| PCs signed in, per account                 | 5                                                                                                                         |
| Sign-in starts, per internet address       | 10 an hour                                                                                                                |
| Finishing sign-in, per internet address    | 1,200 an hour                                                                                                             |
| Code tries on the account site             | 10 in 15 minutes per account                                                                                              |
| A message's text                           | 4,000 characters                                                                                                          |
| An objective's or an answer's text         | 20,000 characters                                                                                                         |
| One sealed copy (before base64url)         | 32 KiB for `message`, `reaction`, `link_note`, `objective_state`, and `collab_note`; 128 KiB for `objective` and `answer` |
| Sealed items from one member still waiting | 64 MiB in all (then `too_many` until they are picked up)                                                                  |
| Items sent, per member                     | 60 a minute                                                                                                               |
| New conversations, per member              | 20 a day                                                                                                                  |
| Objectives on one link, each way           | 20 a day (the receiving PC also refuses past 5 waiting)                                                                   |
| Link requests, per member                  | 20 a day                                                                                                                  |
| Collaboration invitations, per member      | 20 a day                                                                                                                  |
| Cards seen (directory and look-ups)        | 200 a day per member, 1,000 a day per internet address; name look-ups 60 an hour                                          |
| Picking up                                 | 1 waiting request per PC; 720 an hour                                                                                     |
| Reports, per member                        | 20 a day                                                                                                                  |
| Email invitations, per member              | 10 a day; one per address per 30 days                                                                                     |
| GIF searches, per member                   | 60 a minute                                                                                                               |
| A picture                                  | 256 × 256 pixels, 256 KB                                                                                                  |
| A request body                             | 1.5 MiB for `items`; 400 KiB for `picture` and `reports`; 16 KiB for the rest                                             |

## 16. The files in this folder

| File                                                           | What it is                                                                                   |
| -------------------------------------------------------------- | -------------------------------------------------------------------------------------------- |
| [`README.md`](README.md)                                       | This contract                                                                                |
| [`schema/community.schema.json`](schema/community.schema.json) | JSON Schemas for every body, one `$defs` entry each                                          |
| [`examples/index.json`](examples/index.json)                   | Which `$defs` entry each example passes                                                      |
| [`examples/`](examples)                                        | An example of each request and answer                                                        |
| [`examples/vector-item.json`](examples/vector-item.json)       | A worked item: payload, signature, report key, tag, and stamp, and a changed copy that fails |
| [`test-stamping-key.json`](test-stamping-key.json)             | The test stamping key (private half published on purpose; never trusted in production)       |
