# ADR-106: Stripe sends every billing email; the account service sends the rest through Microsoft 365

- **Status:**
  - **Accepted** for billing email (the owner, 2026-09-30: "Can we have Stripe handle all billing
    emails instead of us?").
  - **Proposed** for the account service's own email through Microsoft 365. The builder first
    recommended Amazon SES; the owner asked, "We use Entra for all our other app emailing. Can we
    use that instead of AWS for consistency?" The builder recommends yes, and the owner confirms.
- **Date:** 2026-09-30
- **Phase:** 22
- **Part of:** [ADR-100 (Phase 11A and 22: what the check found, and the owner's answers)](ADR-100-phase-11a-22-owners-answers.md)

## In short

Stripe sends every email about money: receipts, renewal reminders, failed payments, cards about to
expire, invoices, refunds, and cancellations. It sends them from Plenipo's own domain, using Stripe's
custom email domain. The account service sends only what Stripe cannot: the license key, sign-in
links, password resets, email confirmations, and "your account was deleted". It sends those through
Microsoft Graph from a shared mailbox in 8 West's Microsoft 365, the way Milepost and 8 West ID
already send email.

## Context

- Stripe Billing can email customers:
  - a receipt after each payment;
  - a reminder before each renewal;
  - a notice when a card payment fails, with a link to fix it;
  - a warning a month before a card expires;
  - invoices, refunds, and cancellations.
- Each Stripe email can link to the customer portal. Stripe can send them from the seller's own
  domain once that domain is verified with DNS records, and replies go to the support address in
  Stripe's public business details.
- Stripe cannot send the license key, sign-in links, or password resets. Those belong to the account
  service.
- Milepost and 8 West ID send email with Microsoft Graph's `sendMail`. Each uses an Entra app with the
  Mail.Send permission, a client secret, and a sender mailbox. Milepost's notes recommend locking the
  app to that one mailbox.
- Exchange Online allows each mailbox 30 messages a minute and 10,000 recipients a day. That is far
  more than the account service will send.
- Stripe's test sandboxes do not send customer emails, except to addresses on the verified domain or
  to team members.

## Decision

1. **Stripe sends every billing email** (accepted):
   - receipts;
   - renewal reminders;
   - failed payments;
   - cards about to expire;
   - invoices, including the ones 8 West makes by hand;
   - refunds;
   - cancellations.

   Each is turned on in the Dashboard, with the 8 West logo and color, and each links to the
   customer portal. This keeps the plan's "Stripe's failed-payment emails". Stripe also retries failed
   payments by itself (Smart Retries).

2. **Stripe sends from Plenipo's domain** ([ADR-105](ADR-105-domain-and-web-address.md)), set up under
   Stripe's **Settings → Customer emails → custom email domain**:
   - DNS records: one TXT record and some CNAME records, which Stripe lists;
   - replies go to admin@8westventures.com, set as the support email in Stripe's public business
     details.
3. **The account service sends only account email:**
   - the license key;
   - sign-in links;
   - password resets;
   - email confirmations;
   - "your account was deleted".
4. **How the account service sends (proposed): Microsoft Graph `sendMail`**, from a shared mailbox on
   Plenipo's domain, for example `hello@<domain>`, named "Plenipo by 8 West". A shared mailbox needs no
   license. Replies go to admin@8westventures.com.
5. **An Entra app just for this service**, with only the Mail.Send permission, **locked to that one
   mailbox** with Exchange Online's RBAC for Applications. Without the lock, Mail.Send lets an app
   send as anyone in 8 West's Microsoft 365, so a stolen secret could send mail as the owner.
6. **Its secret:**
   - a client secret, as Milepost and 8 West ID use;
   - kept in AWS Systems Manager Parameter Store as an encrypted value that only the server's AWS
     identity can read;
   - never in either repository;
   - Entra secrets expire, after 24 months at most. The service warns the admin 30 days before.
7. **Email checks on Plenipo's domain:**
   - the domain is added to 8 West's Microsoft 365;
   - SPF: `v=spf1 include:spf.protection.outlook.com -all`. Stripe's mail passes through its own
     "mail from" address, so the domain's SPF record does not need to list Stripe;
   - DKIM: turned on in Microsoft Defender (two CNAME records), plus Stripe's own DKIM records;
   - DMARC: starts at `p=none`, with reports to admin@8westventures.com, and moves to `p=quarantine`
     after two clean weeks. It never uses strict SPF alignment, which Stripe does not support.
8. **Test copies never send real email.** The account service's test copies deliver to a mail catcher
   (Mailpit) on Coastline, where the tests read the key. Stripe's sandbox sends billing email only to
   addresses on the verified domain or to team members.

## Consequences

- Stripe writes, sends, and keeps records of every money email. The account service has fewer emails
  to get right.
- A customer gets email from two senders on the same domain: billing from Stripe, and account email
  from Microsoft 365.
- There is one more Entra app and one more secret to renew.
- Bounced account email lands in the shared mailbox, for a person to read.
- If Microsoft 365 is down, the account service keeps each email in a queue and tries again.

## Alternatives considered

- **The account service sends every email** (the builder's earlier proposal). Not chosen by the owner:
  Stripe already writes and sends billing email well.
- **Amazon SES for the account service's email.** It needs no stored secret, because the server's AWS
  identity sends, and it reports bounces to the program. Not chosen, for consistency with 8 West's
  other apps.
- **An email company such as Postmark.** Another vendor, another bill, and another secret.
