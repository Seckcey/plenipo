# ADR-103: Hosting, backups, and monitoring for the account service

- **Status:** Accepted (by the owner, 2026-09-30, as recommended), except the server's size
  (decision 2). That part is **Proposed** until the owner answers.
- **Date:** 2026-09-30
- **Phase:** 22
- **Part of:** [ADR-100 (Phase 11A and 22: what the check found, and the owner's answers)](ADR-100-phase-11a-22-owners-answers.md)

## In short

The account service runs on AWS, on a small server of its own, with Cloudflare in front. Test copies
run on Coastline. Every night, a locked copy of the database goes to AWS storage, and every key ID is
also saved on its Stripe subscription. An outside checker warns the owner if the service goes down.

## Context

- 8 West's other production apps run on AWS EC2. Plenipo's website runs on Coastline behind a
  Cloudflare Tunnel.
- The owner's standing rule: test copies and previews run on Coastline, each in its own folder with
  its own test database, and are stopped after use.
- The service is small. Each Pro copy checks in once a week, so even 10,000 customers is about one
  check a minute. The largest part is the database and the program's own memory, not the traffic.
- An outage never takes Pro away from a paying customer: Plenipo keeps Pro for 30 days between
  successful checks ([ADR-022](ADR-022-subscription-and-license-check.md)). While the service is
  down, though, nobody can buy Pro or open their account page. Stripe keeps retrying its notices for
  up to three days, so no payment is lost.

## Decision

1. **AWS, on a server of its own.** One EC2 server runs only the account service, never the Milepost
   or Logbook server. It uses Ubuntu's long-term release on an ARM (Graviton) type. Docker Compose
   runs the service and PostgreSQL. Images are built by the repository's checks, never on the
   server.
2. **Size (Proposed).** Start on `t4g.micro`: 2 processors, 1 GB of memory, about $6 a month.
   - Add 2 GB of swap (disk used as spare memory).
   - Set a memory alarm at 80%.
   - Move to `t4g.small` (2 GB, about $12 a month) if the alarm fires. Changing the size takes a few
     minutes of downtime, and Pro stays on the whole time.
   - The whole bill, with the disk, the address, backups, and the vault's keys, is about $15 to $25 a
     month.
3. **Cloudflare in front,** through a Cloudflare Tunnel like the website's. The server has no open web
   port. Administrators reach it only through AWS Session Manager, or SSH from 8 West's addresses.
4. **Test copies on Coastline.** Each has its own folder and Compose project, a test database, the
   test signing key ([ADR-104](ADR-104-signing-key-in-aws-kms.md)), Stripe's test mode, and a test
   address through Coastline's tunnel, so Stripe's test notices reach it.
5. **Backups:**
   - every night, an encrypted copy of the database to S3, kept 35 days. The server can add copies
     but cannot delete them;
   - every day, a snapshot of the server's disk (AWS Backup), kept 7 days;
   - every key ID is also written on its Stripe subscription, so the list of keys can be rebuilt from
     Stripe;
   - once a month, a copy is restored on Coastline to prove it works, and the result is recorded.
6. **Monitoring:**
   - an outside checker (UptimeRobot, free) opens the service's health page every 5 minutes, and
     emails the owner and alerts their phone through its app when the page fails;
   - AWS alarms go off for signing errors from the vault, and for memory or disk above 80%;
   - Stripe emails 8 West when it cannot deliver a notice;
   - the service emails admin@8westventures.com about its own errors, never with personal data or
     secrets.
7. **Kept up to date:** the server installs security updates by itself. The service gets a security
   review before launch, as the plan says.

## Consequences

- The server's own AWS identity is how it reaches the signing vault (ADR-104) and its stored secrets
  (ADR-106), so no vault password sits on the server.
- A problem on this server never takes down Milepost or Logbook, and the reverse.
- The owner creates the AWS pieces. The builder writes the exact steps in the account service's
  repository.

## Alternatives considered

- **Coastline for production.** Not chosen. Coastline is the test host, by the owner's rule.
- **Milepost's or Logbook's server.** Not chosen. A problem on one would take down the other, and
  payments deserve a server of their own.
- **AWS's managed database (RDS).** Not chosen for launch: about $12 or more a month extra. The
  service can move to it later without other changes.
- **Serverless (Lambda).** Not chosen: more moving parts for a one-person team.
