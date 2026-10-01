# Phase 22 — Implementation Checklist

**Status: Phase 22 built, not live** (2026-09-30; [acceptance report](phase-22-acceptance-report.md)).
The 8 West account service lives in its own private repository,
`Seckcey/plenipo-account` (draft pull request 1 there). None of its code is in this repository
([ADR-101](../adr/ADR-101-account-service-repository-name.md), the account service's own
repository). The one thing both share is the weekly check's contract,
`contracts/license-check/v1`, written here and copied there byte for byte. Stripe is in **test mode
only**. A security review comes before anything goes live. Plenipo is made by 8 West Ventures, LLC.

Source: `ROLLOUT_PLAN.md`, Phase 22 — The 8 West Account Service: Users, Billing, Email, and
Licenses, and ADR-100 to ADR-118 (the owner's answers). The ones for this phase are ADR-101 to
ADR-109, ADR-111, and ADR-118.

Dates are Pacific time.

## In short, for the owner

- **Built.** A customer can:
  - make an account (a password, or a 15-minute sign-in link by email)
  - buy Pro monthly or yearly with Stripe Checkout
  - get a license key, shown on the account page and emailed
  - manage the subscription in Stripe's customer portal
  - delete the account
- **Stripe sends every billing email** (receipts, renewals, failed payments). The account service
  sends only account email (the key, sign-in links, password resets) from hello@getplenipo.com,
  through Microsoft 365.
- **The weekly check** answers Plenipo's request with a signed answer. The key is signed in AWS
  KMS, and the private key never leaves the vault.
- **8 West's admin page** sits behind Cloudflare Access, with a password and a passkey. It covers
  customers, subscriptions, keys (switch off and on), refunds, and a record of every action.
- **Not done yet: going live.** That waits for the secrets, the security review, and the attorney
  (see "Left for the owner").

## Deliverables

- [x] Its own repository and rules, named by the owner: `plenipo-account`, private
      (ADR-101, ADR-102).
- [x] Accounts:
  - sign up, with the email confirmed
  - sign in by password or by an emailed link
  - reset the password
  - delete my account
  - passwords kept as Argon2id hashes, and checked against leaked ones by a range lookup
    (ADR-118)
- [x] Buying Pro with Stripe Checkout, monthly ($9) or yearly ($99). Prices are found by lookup key
      (`plenipo_pro_monthly`, `plenipo_pro_yearly`), never by Stripe's IDs.
- [x] Stripe's customer portal: the card, invoices, switching plans, and cancelling at the end of
      the paid period.
- [x] Licenses, all driven by Stripe's notices:
  - a key issued when the first invoice is paid: stored, emailed, shown on the account page, and
    written to the subscription's metadata
  - renewals keep the same key
  - cancellation takes effect at the end of the paid period
  - failed payments keep Pro while Stripe retries
- [x] Sales tax by Stripe Tax (set in Stripe; see "Left for the owner"). A 7-year record of each
      paid invoice.
- [x] The weekly check (`POST /v1/check`):
  - it takes exactly the key's ID and the app version, and nothing else
  - it answers signed: active, cancelled, ended, or unknown
  - it logs only the key's ID
- [x] Email: account email through Microsoft Graph, from hello@getplenipo.com, with replies to
      admin@8westventures.com. Stripe sends the billing email (ADR-106). SPF, DKIM, and DMARC are set up
      for getplenipo.com (2026-09-30).
- [x] The admin page, for 8 West: customers, subscriptions, keys, refunds, and the audit record
      (ADR-107).
- [x] The privacy notice and the terms of sale, marked "Draft for attorney review" (ADR-111).
- [x] Hosting, backups, and monitoring decided (ADR-103). The server is made (a `t4g.micro` in AWS),
      with its deployment and backup scripts in the repository.

## Tests (the plan's list)

- [ ] **Buy monthly and yearly in Stripe's test mode; the key is issued, emailed, and accepted by
      Plenipo.** The service's side is tested offline ("issues one signed key when the first invoice
      is paid…"), and the app accepts the contract's keys. The run against real Stripe test mode, on a
      test copy on Coastline, waits for the owner's Stripe test key in that copy's settings file.
- [x] A notice not signed by Stripe is refused ("refuses a notice with no signature…", "refuses a
      forged signature and a changed body").
- [x] The same Stripe notice sent twice issues one key, not two ("the same notice sent twice
      issues one key and one email", with a UNIQUE rule in the database behind it).
- [x] A key signed by anything but the vault's key is refused by Plenipo
      (`a_key_signed_by_another_key_is_refused`, and the contract's `unknown-signer` key).
  - [x] The spare public key works after a key change
        (`the_key_in_use_and_the_spare_are_both_trusted`).
- [x] The weekly check answers active, cancelled (at the end of the paid period), ended, and
      unknown (check tests; the app's `each_state_is_accepted_when_signed_by_a_trusted_key`).
- [x] The check accepts only the key's ID and the app version ("refuses wrong or extra fields with
      400…", "refuses another content type with 415", "refuses a body over 1 KB with 400").
- [x] A failed payment does not cancel at once, and Pro ends only when Stripe stops retrying ("a
      failed payment sends nothing from us, cancels nothing, and keeps Pro active"; "active while Stripe
      retries a failed payment (past_due)"). Stripe emails the customer (ADR-106).
- [x] Delete my account removes personal data and keeps the tax records ("stops renewal, removes
      personal data at once, and keeps the key state and tax records").
- [x] Sign-in abuse is slowed; admin pages need 8 West's sign-in with a second factor (the
      rate-limit tests; "a password alone is not enough"; "refuses every admin request without a
      pass").

## Left for the owner

Values go only where each line says. Never in chat, and never in either repository.

- [x] **AWS KMS:** the two production signing keys and the test key (made 2026-09-30).
- [ ] **AWS:** the server's instance role, the S3 backup bucket, and the settings in SSM Parameter
      Store under `/plenipo-account/production/` (the list is in the acceptance report and in the
      service's `docs/secrets.md`).
- [ ] **Stripe (sandbox now, live before launch):**
  - Stripe Tax's head office address
  - the webhook endpoint and its signing secret
  - a restricted key for the service
  - the privacy and terms links on Checkout and the portal
  - the custom email domain
- [ ] **Microsoft Entra:** the app registration that sends from hello@getplenipo.com, locked to that
      one mailbox (RBAC for Applications). Its secret goes in SSM.
- [ ] **Cloudflare:** the tunnel to the server, and Access for the admin page.
- [ ] **UptimeRobot:** the check address, every 5 minutes.
- [ ] **An attorney:** the terms of sale and the privacy notice.
- [ ] **The security review, then live mode.**
