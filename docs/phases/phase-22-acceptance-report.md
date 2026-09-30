# Phase 22 — Acceptance Report

|              |                                                                                                                                                                                                                                                                |
| ------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Phase**    | 22 — The 8 West Account Service: Users, Billing, Email, and Licenses                                                                                                                                                                                           |
| **Where**    | The private repository `Seckcey/plenipo-account`, branch `claude/phase-22-account-service` (draft pull request 1). No code in this repository (ADR-101).                                                                                                       |
| **Verified** | 106 tests, all passing locally and on GitHub. They run offline, with an in-memory PostgreSQL and no network. Lint, format, and type checks are clean. The Docker image builds on GitHub.                                                                       |
| **Date**     | 2026-09-30 (Pacific time)                                                                                                                                                                                                                                      |
| **Result**   | Built, **not live**. Stripe is in test mode only. The run against real Stripe test mode, the owner's secrets and settings, the security review, and the attorney's review of the terms come before launch (sections 3 and 5). Plenipo by 8 West Ventures, LLC. |

## 1. Deliverables → result

The plan's list, one by one, is in the [checklist](phase-22-checklist.md#deliverables).

| Deliverable (ROLLOUT_PLAN.md)                     | Result               | Where (in `plenipo-account`)                                          |
| ------------------------------------------------- | -------------------- | --------------------------------------------------------------------- |
| Its own repository and rules                      | **Done**             | ADR-101 (its name and rules), ADR-102 (private)                       |
| Accounts: sign up, sign in, reset, delete         | **Done**             | `src/accounts/`, ADR-118 (customer accounts)                          |
| Buying Pro with Stripe Checkout                   | **Done** (test mode) | `src/billing/`, prices by lookup key                                  |
| Stripe's customer portal                          | **Done** (test mode) | `src/billing/`                                                        |
| Licenses driven by Stripe's notices               | **Done**             | `src/billing/`, `src/licensing/`                                      |
| Sales tax by Stripe Tax                           | **Set in Stripe**    | Stripe Tax's settings (section 5)                                     |
| The weekly check                                  | **Done**             | `src/licensing/`, `POST /v1/check`, the contract tests                |
| Email                                             | **Done**             | `src/email/`, ADR-106 (Stripe sends billing email)                    |
| An admin page                                     | **Done**             | `src/admin/`, ADR-107 (password and passkey behind Cloudflare Access) |
| Privacy notice and terms of sale, for an attorney | **Drafted**          | `legal/`, ADR-111                                                     |

## 2. Tests → evidence

The plan's list is in the [checklist](phase-22-checklist.md#tests-the-plans-list), with the test
names. One is not done yet: buying in real Stripe test mode, end to end, with the key accepted by
Plenipo (section 3).

## 3. Not done yet, and why

- **Buy → email → key → Pro, against real Stripe test mode.** This needs a test copy of the service
  on Coastline, with a Stripe sandbox restricted key in that copy's own settings file. The robot
  buys with Stripe's test payment method (never a typed card). The owner does one Checkout
  click-through by hand. The screenshots go in `docs/phases/evidence/phase-22/`.
- **"The spare public key works after a key change."** This waits for the production keys (the
  [Phase 11A report](phase-11a-acceptance-report.md), section 6).

## 4. Questions for the owner (from the build)

1. **Switching monthly ↔ yearly.** The key keeps saying its first plan; the weekly answer's
   paid-through date changes. Should a switch email a new key? (The builder's default: no.
   Settings → License shows the paid-through date from the weekly answer.)
2. **Subscriptions made by hand in the Dashboard.** The key goes to the Stripe customer's email.
   It links to an account only if the admin sets `plenipo_account_id` in the customer's metadata.
   A one-off invoice with no subscription gets no key.
3. **Still to pick:**
   - where the service's image is kept
   - the cloudflared version
   - the admin page's private address
4. **For the accountant:**
   - the tax code
   - the 7-year record
   - EU and UK VAT

## 5. Where each secret and setting goes

Values are never pasted into chat or put in either repository.

**AWS Systems Manager Parameter Store, `/plenipo-account/production/`** (the server reads it
through its instance role):

- **SecureString (secrets):**
  - `SESSION_SECRET`
  - `STRIPE_SECRET_KEY` (a restricted key)
  - `STRIPE_WEBHOOK_SECRET`
  - `GRAPH_CLIENT_SECRET`
- **Plain values:**
  - `PUBLIC_BASE_URL`
  - `LICENSE_SIGNER_NAME`
  - `GRAPH_TENANT_ID`
  - `GRAPH_CLIENT_ID`
  - `CF_ACCESS_TEAM_DOMAIN`
  - `CF_ACCESS_AUD`
  - `GRAPH_CLIENT_SECRET_EXPIRES_ON`
  - `ADMIN_PATH` (kept private)
- **At launch only:** `STRIPE_ALLOW_LIVE`, set by the owner.

**AWS Systems Manager Parameter Store, `/plenipo-account/host/`** (copied into the server's
root-only `.env` by `scripts/host-env-from-ssm.sh`):

- `APP_IMAGE`
- `POSTGRES_PASSWORD` (secret)
- `TUNNEL_TOKEN` (secret)
- `CLOUDFLARED_IMAGE`

**On the server, `/etc/plenipo-account/backup.env`:**

- `BACKUP_BUCKET`
- `BACKUP_AGE_RECIPIENT` (a public key)
- `COMPOSE_DIR`

The backup's private key and the admin recovery codes stay offline, with the owner.

**The Coastline test copy's own settings file:**

- the AWS keys of an IAM user that may sign with the test key only
- the Stripe sandbox restricted key
- `APP_ENV_FILE`, `APP_PORT`, `MAILPIT_PORT`

**GitHub:** no secrets.

The exact Stripe permissions are in the service's `docs/secrets.md`. There is one restricted key
for the service, and a separate one-time key for the set-up script.

## 6. Review

The security review of the service, with a second reviewer for each finding, is recorded here
when done. A full security review comes again before anything goes live.
