# ADR-063: Signing in to a connection in your own browser; its token kept only in the Vault

- **Status:** Accepted (by the owner, 2026-09-28), with the owner's choices in the
  [Phase 20 checklist](../phases/phase-20-checklist.md#owner-decisions-2026-09-28)
- **Date:** 2026-09-28
- **Phase:** 20
- **Carries out:** ROLLOUT_PLAN.md Phase 20, "**Sign-in** to each service in the owner's browser;
  the service's sign-in token is kept in the Vault; never in the Ledger, a prompt, or a log"
- **Amends:** ADR-013 (Guard and the capability broker) §12 — the Vault also keeps connections'
  sign-in tokens; Guard's gate for Plenipo's own requests (ADR-038, ADR-059) gains one purpose per
  service

> **On screen** (ADR-010, plain words and rank names): **Connect**, **Reconnect**,
> **Disconnect**, "**Finish signing in in your browser**", "**Signed in. You can close this tab
> and go back to Plenipo.**", and "**needs you to sign in again**". This record keeps the code's
> words (OAuth, PKCE, redirect, token, scope).

## In short

You press **Connect** on a connection's card. Plenipo opens the service's own sign-in page in
**your own web browser** — the one you use every day. You sign in there, on Microsoft's (or
Slack's, or Google's) own page, and approve what Plenipo asks for. Plenipo never sees your
password. The service then hands Plenipo a sign-in token, and Plenipo keeps it only in the Vault
(Windows Credential Manager) — never in the Ledger, a worker's prompt, a log, or a diagnostics
file. **Disconnect** removes it from the Vault. Accepting this record means building sign-in this
way for every connection that has a sign-in page.

## Context

The plan: sign in "in the owner's browser"; the token "in the Vault; never in the Ledger, a
prompt, or a log"; "a sign-in token never appears in the Ledger, a prompt, a log, or a diagnostics
file"; "disconnecting removes the token from the Vault". The owner's rules (2026-09-28): "Signing
in to each service happens in my own browser. Plenipo never sees my password"; nothing secret is
asked for in chat; the fewest permissions that work.

What the code has today (read at `0a53e1a`, v1.12.0):

- **No sign-in flow of this kind exists.** Plenipo never opens the owner's default browser (no
  opener, no shell plugin; ADR-013). Its own browser (ADR-020, ADR-028) is Edge or Chrome with
  Plenipo's own profile, driven by Plenipo over DevTools.
- **The Vault** (`crates/capabilities/src/vault.rs`) keeps values in Windows Credential Manager
  under `com.eightwest.plenipo`, in pieces when longer than 1,000 characters (up to 10,000), and
  Plenipo keeps only references. Server sign-ins are kept the same way (`server-<id>-…`), are
  removed with their server, and are fed to the redactor. Uninstalling with "delete my data"
  removes every value listed by `stored_ids`.
- **The redactor** hides stored values and known token shapes (`Bearer …`, JSON web tokens, Slack
  `xox…`) everywhere Plenipo records or shows text, and filters the log file and the diagnostics
  file.
- **Guard's gate for Plenipo's own requests** (`crates/guard/src/outbound.rs`) allows only fixed
  addresses per purpose, `https` only, with every redirect checked, and records refusals by host
  only. Its client does `GET` only.

## Decision

### 1. Your own browser, the standard way for desktop apps

- Sign-in uses the method the services publish for desktop apps: **the authorization code flow
  with PKCE** (a one-time secret made fresh for each sign-in, RFC 7636) and a **loopback
  redirect** (the browser comes back to a one-time address on this PC, `http://127.0.0.1:<port>`,
  RFC 8252). Where the service allows it (Microsoft 365), Plenipo holds **no client secret** at
  all.
- **Plenipo opens the address with Windows' own "open a web address"** (the default browser, as a
  link in an email would), never through a shell command, and only after Guard's gate has checked
  it: `https`, the service's own sign-in host from a fixed list, and nothing else.
- **Why not Plenipo's own browser:** Plenipo drives that browser and can read its pages. Your own
  browser is yours alone, and you may already be signed in to the service there. (A choice for the
  owner: the checklist, choice 3.)

### 2. What happens on Connect

1. The card calls `connect_connection(service)`. **Guard** checks it (`check_connection_action`):
   a known service, built into this copy, not already connecting, and an app ID for it present
   (ADR-065 §7). A refusal is recorded as `guard.connection_refused`.
2. The broker makes a fresh PKCE secret and a fresh `state` (32 random bytes each), and opens a
   **one-time listener** on `127.0.0.1` at a port Windows picks.
3. It builds the sign-in address from the service's fixed parts, the app ID, the permissions for
   the parts that are on (§3), the challenge, and the state; Guard's gate checks it; Windows opens
   it. The card says "**Finish signing in in your browser**" with **Cancel**.
4. The listener answers **one** request: `GET /callback` whose `state` matches. Anything else gets
   "not found" and is ignored. It shows "**Signed in. You can close this tab and go back to
   Plenipo.**" (or the service's refusal, in plain words), and closes. It never shows the code.
   It stops after 10 minutes, on **Cancel**, or when Plenipo closes.
5. The broker trades the code (with the PKCE secret) for tokens at the service's token address,
   through Guard's gate (the gate's client gains `POST` for this, with the same checks).
6. **The long-lived token** (the refresh token, or the service's long-lived token) goes to the Vault
   under `connection-<service>-token`. **The short-lived access token** stays in memory only, with
   its expiry. Both are added to the redactor at once.
7. Plenipo asks the service who is signed in (Microsoft 365: the name and address in the sign-in
   answer) and keeps that in the connection's settings, with the permissions actually granted.
   Records `connection.connected`.

### 3. The fewest permissions, asked for part by part

- Plenipo asks only for the permissions of the parts that are **on** (Microsoft's and Google's
  sign-in allow this: "incremental consent"). Turning a part on later shows **Reconnect to allow
  Teams**; turning one off stops its tools at once (the permission stays granted until the next
  sign-in, which asks for less).
- The exact permissions per service are in ADR-064 and ADR-065. The card lists them under "What
  Plenipo was allowed", in the service's words and in plain words.

### 4. Keeping the sign-in fresh

- Before a call, when the access token has less than 5 minutes left, Plenipo trades the long-lived
  token for a new one. When the service sends a new long-lived token (Microsoft does, each time),
  it replaces the old one in the Vault, then is read back to check it, before the old one is
  forgotten.
- **If the service refuses** (the owner changed their password, an admin removed the app, the
  token expired): the connection's state becomes **needs you to sign in again**; its tools are no
  longer offered to new steps, calls in running steps are refused with that reason, a notice tells
  the owner, and `connection.sign_in_needed` is recorded (service and reason, no token).

### 5. Disconnect

1. Its tools stop at once: new steps do not get them, and running steps' calls are refused.
2. Where the service has a way to cancel a token (Google, Slack, HubSpot), Plenipo cancels it
   there, through Guard's gate. Microsoft has none for one token: the card says how to remove
   Plenipo from the account entirely (**My Apps** → Plenipo → **Remove**).
3. The Vault entries (and their pieces) are erased, the access token forgotten, the redactor
   refreshed, and the account's name and granted permissions forgotten.
4. `connection.disconnected` is recorded. "Who may use it", the parts, and the "send without
   asking" list stay, so connecting again restores them.

A test checks the Vault has no entry for the connection afterwards (the plan's test).

### 6. Where the token can never be

- **Not in the Ledger:** Guard's settings keep no token; events carry no token, code, or state;
  sign-in addresses are recorded only as host, path, and field names (ADR-057).
- **Not in a prompt or a worker's result:** the token is added to the request inside the
  connection's code, and never passed to an AI tool, an add-on program, or a worker. Results are
  redacted, and the token is among the redacted values.
- **Not in a log or the diagnostics file:** both are filtered with the current tokens; Plenipo's
  messages about sign-in never include the address's query.
- **Not in an uninstall leftover:** `stored_ids` lists connection tokens, so "delete my data"
  removes them.
- **Tests** check each place with a recognizable fake token (§8), in the style of
  `settings_keep_sign_ins_in_the_vault_only` (Phase 11).

### 7. Nothing to paste in chat

- **Microsoft 365:** 8 West's app ID is public (every user of the app can see it). It goes in the
  code. There is no client secret.
- **Services that need a key or a secret the owner creates** (ADR-064): the owner types it into
  the connection's card in Settings, which sends it straight to the Vault, like server sign-ins
  today. It is never asked for in chat, never committed, and never shown again.

### 8. Tests without the real services

- A **stand-in for each service** (a small web server on `127.0.0.1`, in the tests and in copies of
  Plenipo built for the end-to-end tests with `PLENIPO_CONNECTIONS_STAND_IN`, as the AI tools'
  release lists are in Phase 19) answers the sign-in, token, and service addresses the connection
  uses, and records what it was sent.
- In those copies, "open in your browser" is replaced by a step that follows the address the way a
  browser would, so the whole sign-in runs end to end. **Released copies cannot do this**: the
  stand-in address is fixed when the copy is built, and is empty in the Release workflow.

## Consequences

- The owner signs in where they always do, and can see and remove Plenipo's access in the
  service's own account page.
- A one-time listener on `127.0.0.1` is new. It answers one request with the right `state`, for 10
  minutes at most, and only on this PC; another program on the PC could race it, but without the
  PKCE secret a stolen code cannot be traded for a token.
- Opening the owner's default browser is new, and only ever for a Guard-checked `https` sign-in
  address.
- Tokens live as long as the service allows (Microsoft: until they go unused for 90 days, or an
  admin or password change ends them). The card shows **needs you to sign in again** when they do.

## Alternatives considered

- **Plenipo's own browser** (the plan's "Phase 10 (browser, for sign-in)"). Not recommended:
  Plenipo can read and drive that browser, so it could in principle see the password page; and the
  owner is not signed in there. Offered as a choice.
- **The device-code flow** (the owner types a short code at microsoft.com/devicelogin). Rejected:
  Microsoft and many admins restrict it because it is a common phishing route, and it needs "public
  client flows" turned on.
- **An embedded sign-in window inside Plenipo.** Rejected: Plenipo would host the password page,
  and Microsoft and Google discourage embedded web views for sign-in.
- **Keep tokens in a file encrypted by Windows (DPAPI).** Not needed: the Vault already keeps long
  values in pieces, and one store is easier to audit and to remove on uninstall.

## As built (v1.13.0, part 20A: Microsoft 365)

Built as written, with these details:

- **The listener** listens on `127.0.0.1` and `[::1]` at one port (a port another program holds on
  either is never used; only a computer without IPv6 gets `127.0.0.1` alone). The redirect address
  is `http://localhost:<port>`. The browser is opened at a **one-time start page with a random
  key** (`/start/<key>`), which sends it on to Microsoft once and is "not found" after, so no other
  program on this computer can learn the sign-in's `state` or PKCE challenge from it.
- **Each sign-in, Cancel, and Disconnect takes a turn.** A sign-in or a renewal that Microsoft
  answers after a newer one of these keeps nothing: no sign-in in the Vault, nothing in memory.
- **Disconnect** marks the connection not connected and drops its access token first, so its tools
  stop even when Windows Credential Manager does not answer; that failure is then reported, and
  the value stays hidden in any text until it is removed.
- **A renewal asks Microsoft only for what it already granted** (asking for more fails the renewal
  and would cost the sign-in); a part turned on or up since the sign-in waits for Reconnect, and a
  part turned down works with what was granted.
- **Keeping a sign-in** is read back; if that fails, the previous one goes back (Microsoft does not
  cancel it when it is used), so the Vault never holds a mix of two.
- **Microsoft's "Need admin approval" page:** going back from it (AADSTS65004) also gives the link
  for the organization's admin.
- **Uninstalling with "delete my data"** removes every service's sign-in, even one kept before its
  connection was.
- **Copies built for the tests** use a stand-in browser that follows the sign-in on this computer;
  the Release workflow sets the stand-in empty and refuses the tests' app ID.
