# Phase 22 — Implementation Checklist

**Status: Phase 22 is live** (Stripe live mode since 2026-10-02;
[acceptance report](phase-22-acceptance-report.md)). The owner's own key works; two test purchases
went through end to end. The owner chose to go live **before** the security review, which is still
to do.
The 8 West account service lives in its own private repository,
`Seckcey/plenipo-account` (draft pull request 1 there). None of its code is in this repository
([ADR-101](../adr/ADR-101-account-service-repository-name.md), the account service's own
repository). The one thing both share is the weekly check's contract,
`contracts/license-check/v1`, written here and copied there byte for byte. Stripe is in **live
mode** (the owner's word, 2026-10-02); the security review is still open. Plenipo is made by 8 West
Ventures, LLC.

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
- **Running in test mode since 2026-10-01** on its own server, at `account.getplenipo.com`. The
  owner has a complimentary Partner Unlimited key in Plenipo, checked in and "active". Two test
  purchases (one by a script with Stripe's test card, one by the owner through sign-up and Stripe
  Checkout) each got a key by email within seconds.
- **Live since 2026-10-02.** The owner said to go live without the security review. Live Stripe
  is set up, test data was removed after a fresh backup, and the service's log says it runs in
  live mode. The owner accepted sign-up and purchase without a live test purchase; the first real
  customer is the live check. AWS alarms and UptimeRobot watch it; nightly backups run, and a
  restore was checked. Still open: the security review and Stripe's custom email domain.

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
- [x] Buying Pro or a Partner plan with Stripe Checkout, monthly or yearly, at the prices in
      ADR-119. Prices are found by lookup key (for example `plenipo_pro_monthly`), never by Stripe's
      IDs. Each plan is its own Stripe product, because Stripe's customer portal allows one monthly
      and one yearly price per product (2026-10-01).
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
- [x] The privacy notice and the terms of sale (ADR-111). Approved by 8 West's attorney; the
      draft labels came off on 2026-10-01 (the owner's confirmation).
- [x] Hosting, backups, and monitoring decided (ADR-103). The server is made (a `t4g.micro` in AWS),
      with its deployment and backup scripts in the repository.

## Tests (the plan's list)

- [ ] **Buy monthly and yearly in Stripe's test mode; the key is issued, emailed, and accepted by
      Plenipo.** Monthly: done twice on the running service in test mode (2026-10-01, seen): Stripe's
      notices were verified, a Pro key was issued, and the key email arrived within seconds
      ([the email](evidence/phase-22/key-email-after-test-purchase.png),
      [Stripe's record](evidence/phase-22/stripe-sandbox-two-test-subscriptions.png)). Accepted by
      Plenipo: seen with the owner's complimentary key, which the app checked in with ("active");
      a bought key was not entered (it would have replaced the owner's). Yearly: not bought yet.
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
- [x] **AWS:** the server's instance role, the S3 backup bucket, and the settings in SSM Parameter
      Store (2026-09-30 and 2026-10-01; each checked, see the service's `docs/deploy.md`).
- [x] **AWS backups:** nightly, encrypted to a key pair whose private half the owner keeps printed;
      the first backup is in the bucket, locked for 35 days, and a restore check on Coastline matched
      the live database's counts (2026-10-01).
- [x] **AWS alarms:** server status check, memory over 80%, disk over 80%, and a failed signature,
      all emailing the owner (2026-10-02, seen in CloudWatch).
- [x] **Stripe sandbox:** Stripe Tax's head office address, the webhook endpoint and its signing
      secret, a restricted key for the service, and the customer portal with the privacy and terms
      links (2026-10-01).
- [x] **Stripe live mode:** branding (the owner), tax, the four plans and eight prices, the default
      customer portal, Billing's emails and retries, the notice endpoint, and the service's key
      (2026-10-02).
- [ ] **Stripe, still to do:** the custom email domain.
- [x] **Microsoft Entra:** the app registration that sends from hello@getplenipo.com, locked to that
      one mailbox (RBAC for Applications). Its secret is in SSM (2026-10-01).
- [x] **Cloudflare:** the tunnel to the server, Access for the admin page with an emailed code, and
      the one rate rule the free plan allows (2026-10-01).
- [x] **UptimeRobot:** the website and the account service's check address (the owner, 2026-10-02).
- [x] **An attorney:** the terms of sale and the privacy notice (approved; 2026-10-01).
- [x] **Live mode** (the owner's word, 2026-10-02, before the security review).
- [ ] **The security review.** Skipped for launch by the owner's choice; still to do.
