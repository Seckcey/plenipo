# Plenipo Security Report

|                        |                                                                                                                                                                                                            |
| ---------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Repositories**       | [Seckcey/plenipo](https://github.com/Seckcey/plenipo) (public) at commit `42e9b42` and [Seckcey/plenipo-account](https://github.com/Seckcey/plenipo-account) (private) at commit `cf455e1`, both on `main` |
| **Date**               | October 2, 2026                                                                                                                                                                                            |
| **Kind**               | Read-only review. No code was changed. The only thing added is this file.                                                                                                                                  |
| **For**                | 8 West Ventures, LLC (Plenipo is made by 8 West Ventures, LLC)                                                                                                                                             |
| **Who should read it** | The owner (section 1) and whoever fixes things (sections 3 to 6, each finding is written so a junior developer can do it)                                                                                  |

## 1. In short (plain words)

Plenipo is in good shape. The hard parts (passwords, sessions, Stripe, license signing, the phone's end-to-end lock, the desktop updater, the sandboxing of AI tools) are built carefully and mostly match what `SECURITY.md` promises. No way was found for a stranger on the internet to get into an account, take money, forge a license, or control a PC.

The real risks are around the edges:

1. **Code gets onto 8 West's servers with nobody checking a signature.** The relay server, the phone page, and the website update themselves from GitHub every 15 minutes. They check a checksum that comes from the same place as the file, which only proves the download was not corrupted. The relay updater also runs the downloaded program as root. If anyone ever publishes a bad release (a stolen token, a bad dependency in the release job), they own the relay server and every paired phone within 15 minutes. The desktop installer does not have this problem; it is signed and the app checks the signature. The servers need the same treatment. (P-SRV-1)
2. **The `main` branch is the production switch in both repositories, but nothing protects it.** The account service deploys itself from `main` within 15 minutes of a green CI run. The public repo has a rule set, but it is switched off; the private repo has none. Turning the rules on takes ten minutes in the GitHub web interface. (G-1)
3. **Adding a phone can be hijacked by someone who sees your screen first.** The PC asks "Is this your phone?" but shows only a name the phone typed in, so a stranger who scans your code first looks exactly like your phone. Showing six check digits on both screens fixes it. (P-SRV-2)
4. **Three things in the account service need a fix soon:** one account can tie up the whole service by spamming the Buy button (A-CUST-1); a plan upgrade hands out the bigger license key before the upgrade invoice is paid (A-CUST-2); and the owner console's audit journal writes "succeeded" for actions that were refused (A-ADM-1).
5. **A few desktop and relay limits are missing:** the 30-day license grace period can be restarted by deleting one file (P-DESK-1); the default command list lets a worker run its own code without a card (documented, but the defaults contradict their own comment, P-GUARD-1); when a project folder is a sub-folder of a bigger repository, `git diff` shows changes from the whole repository (P-GUARD-2); a PowerShell script skips the blocked-commands list if an owner sets scripts to Allow (P-GUARD-4); and the relay can be pushed out of memory by a hostile relay, a leaked license file, or many addresses (P-SRV-4, 5, 6).

Dependencies: no secret was found in either repository or its history. One Rust advisory (`rsa`, a timing side channel, no fix published yet, low practical risk here) and four npm advisories in test-only packages. Details in section 6.

### Numbers

| Severity | plenipo-account | plenipo | GitHub and process (both) |
| -------- | --------------- | ------- | ------------------------- |
| Critical | 0               | 0       | 0                         |
| High     | 0               | 2       | 1                         |
| Medium   | 2               | 9       | 2                         |
| Low      | 7               | 9       | 1                         |
| Info     | 2               | 4       | 0                         |

### Do these first

1. **Sign the relay and phone-page release assets and verify the signature in both updaters; run the version check as the service user, not root.** (P-SRV-1, about a day of work.)
2. **Turn on a branch ruleset for `main` in both repositories, and enable private vulnerability reporting on the public one.** (G-1, G-2, ten minutes in GitHub Settings.)
3. **Show six check digits on both screens when pairing a phone, and tell the losing phone why.** (P-SRV-2, half a day.)
4. **Try-lock and rate-limit `/buy`, add a pool connect timeout, and require a paid invoice before a bigger key on a plan switch.** (A-CUST-1, A-CUST-2, half a day plus tests.)
5. **Confine `git diff`, `git log`, and `git status` to the project folder; enforce the key's paid-through date in the desktop license check; cap connections and sweep addresses in the relay.** (P-GUARD-2, P-DESK-1, P-SRV-4/5/6, a day.)

## 2. How to read this report

- **Severity** means how bad it would be if used, not how likely. _Critical_: a stranger takes over an account, money, a license, or a PC. _High_: the same, but needing a precondition such as a leaked token or a stranger seeing your screen. _Medium_: a real bug with limited reach, or a service outage. _Low_: hardening. _Info_: a note so the next reviewer does not redo the work.
- **Confidence** says how sure the reviewer is. _High_ means the code path was read end to end and the lines are quoted. Anything not confirmed by reading code is marked, or left out.
- Every finding has: **Where** (file and line numbers at the commits above), **What the code does now**, **Why it matters**, **How to fix** (the actual change), and **How to check** (a test or a manual step).
- Finding IDs: `A-` is the account service, `P-` is plenipo (desktop, servers, website), `G-` is GitHub and process, `D-` is dependencies.
- **How the review was done:** the whole of `plenipo-account` and the security-relevant parts of `plenipo` were read by hand (appendix B lists exactly what was and was not read); automated scans ran on top (`cargo audit`, `pnpm audit`, `npm audit`, `detect-secrets`, a git-history pattern sweep, GitHub API checks of rulesets, vulnerability reporting, and recent pull requests, and a live header check of the websites). Every finding from the automated and assisted passes was re-verified against the source by the author before it went in.

## 3. The account service (plenipo-account): findings

### 3.1 Customer accounts, billing, email, licensing

### A-CUST-1 [Medium] One confirmed account can stall the whole service by hammering `POST /buy`

- **Where:** `src/accounts/routes.ts:853-1049` (the `/buy` handler; the lock at `:917-919`; Stripe calls inside the transaction at `:969, 983, 1005-1006, 1014-1018, 1023`), `src/db/db.ts:27` (`new pg.Pool({ connectionString, max: 10 })`, no `connectionTimeoutMillis`) and `:36-44` (`pool.connect()` happens before `BEGIN`), `src/billing/stripe-api.ts:329-330` (`maxNetworkRetries: 2, timeout: 20_000`).
- **What the code does now:** `/buy` opens a transaction (which takes one of the 10 pooled connections), then waits on `pg_advisory_xact_lock('buy:<customer>')`, then makes three to five Stripe calls while still holding the connection. There is no rate limit on `/buy` or `/billing` (every `limiter.take` in `routes.ts` is on sign-up, sign-in, links, confirm-resend, or password checks; see lines 186-491, 815, 1155, 1216).
- **Why it matters:** ten parallel `/buy` posts from one signed-in account (a valid CSRF token is easy to get: it is in the page) hold all ten connections: one is talking to Stripe (up to 60 s with retries), nine are queued on the lock. Every other request that needs the database, including the session check that runs for every account page, the Stripe webhook, and the weekly license check, then waits on `pool.connect()` with no timeout. Repeating the burst keeps the service down. No data is exposed.
- **How to fix (do all three):**
  1. Fail fast instead of queueing. In `src/accounts/admin-support.ts` give the customer-facing callers a try-lock:
     ```ts
     const { rows } = await q.query<{ ok: boolean }>(
       "SELECT pg_try_advisory_xact_lock(hashtext($1)) AS ok",
       [`buy:${id}`],
     );
     if (!rows[0]?.ok)
       throw new CustomerProblem(
         "Another change to this account is still running. Wait a moment and try again.",
         409,
       );
     ```
     and use the same at `routes.ts:919`. Webhook and owner-console callers that must wait keep the blocking lock. Update the "two tabs" test at `test/accounts.test.ts:582-592` to expect a 409 on the second tab.
  2. Add a limit in `src/accounts/rate-limit.ts`: `buyPerAccount: { name: 'buy-account', max: 10, windowSeconds: 15 * 60 }`, and take it at the top of `/buy` and `/billing`, answering 429 with a plain "Too many tries, wait 15 minutes" page.
  3. `src/db/db.ts:27` → `new pg.Pool({ connectionString: databaseUrl, max: 10, connectionTimeoutMillis: 5_000 })` so a saturated pool gives a fast 500 instead of a hang.
- **How to check:** test in `test/accounts.test.ts`: make the fake Stripe's `listSubscriptions` wait on a promise; fire 12 parallel `/buy` posts; assert at least 11 answer 409/429 within a second and `GET /healthz` still answers; resolve the promise; assert the first `/buy` returns the Checkout URL.
- **Confidence:** High on the mechanism; Medium on how long real Stripe latency would hold the pool.

### A-CUST-2 [Medium] A plan switch to a bigger plan hands out the bigger license key before the upgrade invoice is paid

- **Where:** `src/billing/notices.ts:477-482`; `src/licensing/keys.ts:173-213` (`replaceKeyIfPlanChanged`); `src/licensing/state.ts:4` and `:97-101` (`past_due` counts as running and the weekly answer says `active`); portal settings in `scripts/stripe-setup.ts:239-255` (switching between all eight prices, `proration_behavior: 'always_invoice'`).
- **What the code does now:**
  ```ts
  if (isRunning(snapshot.status)) {
    // true for active, trialing, past_due
    if (paid) issued = await issueKeyIfMissing(tx, deps, snapshot, customerId);
    if (issued) await alertIfPaidTwice(tx, deps, snapshot);
    else issued = await replaceKeyIfPlanChanged(tx, deps, snapshot, customerId); // no `paid` check
  }
  ```
  The first key needs `paid`. A switch does not: `replaceKeyIfPlanChanged` only compares allowances, retires the old key, and signs a new one.
- **Why it matters:** buy Pro monthly ($19), get a Pro key. In Stripe's portal switch to Partner Unlimited ($299 a month); the portal invoices the difference at once. If that invoice does not settle (card declined after authorization, an asynchronous payment method, or the subscription was already `past_due`), Stripe still sends `customer.subscription.updated` with the new plan and status `past_due` or `active`. The service then emails a Partner Unlimited key and answers `active` to the weekly check until dunning cancels the subscription, which can take days to weeks. Fail-open on `past_due` was chosen for renewals of an already-paid plan; a never-paid bigger plan is a different case.
- **How to fix:** require proof of payment when the allowance grows. In `src/licensing/keys.ts` add a `proof: { paid: boolean }` parameter to `replaceKeyIfPlanChanged`; when the new plan covers more organizations than the current key and `proof.paid` is false, read the subscription's latest invoice from Stripe (`deps.stripe.getInvoice(snapshot.latestInvoiceId)`) and return `null` unless it belongs to this subscription and its status is `paid`. The `invoice.paid` notice for the proration invoice arrives with `paid = true` and runs the switch then, so notices in any order still end in one key. Callers: `notices.ts:481` passes `{ paid }`; `admin-operations.ts` (the owner's repair action, which already checks a paid invoice) passes `{ paid: true }`. Downgrades keep working as today.
- **How to check:** in `test/billing.test.ts` under "switching plans in the portal": set the fake subscription to `past_due`, switch it to `plenipo_partner_unlimited_monthly`, send `customer.subscription.updated`, and assert there is still one key and no `license_key` email; then set it `active`, add a paid invoice, send `invoice.paid`, and assert the Partner key exists. The existing "notices out of order" and downgrade tests must keep passing.
- **Confidence:** Medium. The code path is verified end to end. Whether Stripe's portal keeps the old price when a synchronous charge is declined outright was not verified; the `past_due`-then-switch and asynchronous-payment cases do not depend on it.

### A-CUST-3 [Low] `/account?m=` looks the message up on the object's prototype chain

- **Where:** `src/accounts/routes.ts:796`: `const message = req.query.m ? (ACCOUNT_MESSAGES[req.query.m] ?? null) : null;` with `ACCOUNT_MESSAGES` a plain object (`src/accounts/pages.ts:408`).
- **Why it matters:** `GET /account?m=constructor` renders a status box containing `function Object() { [native code] }`; `?m=__proto__` renders `[object Object]`. Output is HTML-escaped, so it is not XSS, only odd text on a signed-in page, and a trap if the lookup is ever reused for a redirect or raw render.
- **How to fix:** `const m = typeof req.query.m === 'string' ? req.query.m : ''; const message = Object.hasOwn(ACCOUNT_MESSAGES, m) ? ACCOUNT_MESSAGES[m] : null;`
- **How to check:** signed-in `GET /account?m=constructor` must not contain `native code`; `?m=confirmed` still shows the confirmation text.
- **Confidence:** High.

### A-CUST-4 [Low] Un-sent emails keep live one-time links in plain text in the database for up to about 22 hours

- **Where:** `src/email/outbox.ts:29-56` (`enqueueEmail` stores `text_body` and `html_body`), `:26-27` and `:105-137` (retry waits of 1, 5, 15, 60, 180, 360, 720 minutes before giving up; bodies are cleared only on success or after the last try). Link lifetimes: confirm 24 h, sign-in 15 min, reset 30 min (`src/accounts/sessions.ts:20-24`), email change 24 h, invitations 1 to 30 days.
- **Why it matters:** the service's stated property is that a copy of the database cannot be used to sign in, because tokens are stored only as SHA-256 hashes. While Microsoft Graph is down, every confirmation, email-change, and invitation link sits in clear text in `email_outbox` (and in any backup taken then) and stays valid. Someone with a database copy during an email outage could confirm attacker-made accounts, accept invitations (complimentary keys), or finish an email change. Precondition: database or backup read access plus an email outage.
- **How to fix:** add `give_up_at timestamptz` to `email_outbox`; in `enqueueEmail` set it to the link's own lifetime for link-bearing kinds (`confirm_email` 24 h, `sign_in_link` 15 min, `reset_password` 30 min, `customer_email_change` 24 h, invitations from the invitation's `expires_at`); in `processOutbox` select only rows whose `give_up_at` is null or in the future; in `cleanUp` (`src/worker.ts`) mark the rest failed and null out `to_address`, `text_body`, `html_body`. Alternative: encrypt the two body columns with AES-256-GCM under a key derived from `SESSION_SECRET`.
- **How to check:** in `test/email.test.ts`: queue a sign-in link, make the fake emailer fail, move the clock 16 minutes, run `processOutbox`, and assert the row is failed and the token appears nowhere in `email_outbox`.
- **Confidence:** High.

### A-CUST-5 [Info] Notes on customer accounts (no change strictly needed)

- The "restricted account cannot buy" check at `src/accounts/routes.ts:131` is a raw-URL string match (`['/buy', '/billing'].includes(req.url.split('?')[0])`). The two POST handlers re-check `restricted_at` under the lock (`:925-929`, `:1057-1064`), so there is no bypass today; move the check into `requireCustomer` so it does not depend on URL spelling.
- The service-wide cap of 200 sign-up emails an hour (`src/accounts/rate-limit.ts`) is a global pause switch: 200 valid sign-ups from 20 or more addresses pause all new accounts for the rest of the hour. This is the documented design, and the Cloudflare rule on `/signup` (5 in 10 minutes, then a 1-hour block) raises the cost. Keep that rule in place.

### 3.2 The owner console

### A-ADM-1 [Low] A refused owner action is written to the audit journal as "succeeded"

- **Where:** `src/accounts/admin-support.ts:159-170` (the `catch` block of `customerOperation`), used by `src/admin/customers.ts` and `src/admin/invitations.ts`. The journal is shown on `src/admin/console-routes.ts:365` and `src/admin/customers.ts:715`.
- **What the code does now:** when an owner action is refused or fails, the catch block writes an audit row with the real result only inside the JSON `detail` field (`detail.outcome: 'denied' | 'failed'`). The `outcome` column itself is left empty, and `audit()` in `src/admin/audit.ts` fills an empty outcome with `'succeeded'`.
- **Why it matters:** the Activity page shows "succeeded" for an action that was rolled back. Anyone reading the journal later (support, a dispute, a forensic review) sees a wrong record.
- **How to fix:** in the catch block, move the value to the column:
  ```ts
  } catch (error) {
    await audit(deps.db, deps.clock, {
      ...actor,
      action,
      target,
      outcome: error instanceof CustomerProblem ? 'denied' : 'needs_attention',
      detail: { operationId: input.id },
    });
    throw error;
  }
  ```
- **How to check:** in `test/admin-customers.test.ts`, after a change that returns 409, read `SELECT outcome FROM admin_audit WHERE action='customer edit'` and assert it is not `'succeeded'`.
- **Confidence:** High (traced the default in `audit.ts` and both page renderers).

### A-ADM-2 [Low] Posting a finished "reveal" or "export" again shows the secret again without a new audit row

- **Where:** `src/admin/licenses.ts:780-869` (reveal a signed license key) and `src/admin/customers.ts:902-930` (customer data export); the replay rule is in `src/admin/operations.ts:93-97`.
- **What the code does now:** an "action reference" (operation id) protects against double submission by replaying the first result. For reveal and export, the replay re-reads the key text or rebuilds the export file and returns it, but writes no audit row because `atomicOperation` treats a replay as "nothing new happened". The current tests lock this in (`test/admin-console.test.ts:270-287`, `test/admin-licenses.test.ts:551-554` expect exactly one audit row after two posts).
- **Why it matters:** pressing F5 on the result page (browser "resubmit form") shows the signed key again, and the journal under-counts how many times a secret was shown. `docs/owner-console.md` promises keys appear only on an explicitly confirmed reveal.
- **How to fix:** when `result.replayed` is true, write a second audit row (`license reveal repeated` / `customer data downloaded`), or refuse the replay with a 409 and a message to start a new reveal. Then update the two tests to expect the extra row (or the 409).
- **How to check:** post the same reveal form twice; the journal must show two rows.
- **Confidence:** High.

### A-ADM-3 [Low] The billing search does not escape `%` and `_` (the other two lists do)

- **Where:** `src/admin/billing.ts:181` and `:198-201`.
- **What the code does now:** `[query, \`%${query}%\`, ...]`goes straight into`ILIKE`. `src/admin/customers.ts:253`and`src/admin/invitations.ts:454`escape with`search.replace(/[\\%_]/g, '\\$&')`.
- **Why it matters:** this is not SQL injection (the value is a bound parameter). But `_` and `%` act as wildcards, so a search for `sub_1` matches `subX1…`, and `%` alone lists every subscription with a full table scan. Owner-only input, so impact is wrong results and load.
- **How to fix:** `const pattern = \`%${query.replace(/[\\%_]/g, '\\$&')}%\`;`and pass`pattern`as`$2`.
- **How to check:** create subscriptions `sub_a1` and `subXa1`; `GET <admin>/billing?q=sub_a1` must list only the first.
- **Confidence:** High.

### A-ADM-4 [Low] The audit journal is "append-only" by habit, not by the database

- **Where:** `migrations/005_owner_console.sql` (the `admin_audit` table), `compose.yaml:36` (one database role, `plenipo_account`, owns everything and also runs migrations).
- **What the code does now:** no trigger, `GRANT`, or `REVOKE` exists anywhere in `migrations/`, `src/`, or `docs/` (checked with grep). Only an `INSERT` path exists in code.
- **Why it matters:** the journal is the only evidence of refunds, key reveals, and owner sign-ins. Any bug, SQL injection elsewhere, or operator slip can `DELETE FROM admin_audit` and nothing stops it.
- **How to fix:** add `migrations/009_audit_append_only.sql`:
  ```sql
  CREATE OR REPLACE FUNCTION admin_audit_append_only() RETURNS trigger LANGUAGE plpgsql AS $$
  BEGIN RAISE EXCEPTION 'admin_audit is append-only'; END $$;
  CREATE TRIGGER admin_audit_no_change
    BEFORE UPDATE OR DELETE OR TRUNCATE ON admin_audit
    FOR EACH STATEMENT EXECUTE FUNCTION admin_audit_append_only();
  ```
  Stronger, later: a second database role for the running service with `INSERT, SELECT` only on `admin_audit`, while migrations keep running as the owner role.
- **How to check:** in `test/migrations.test.ts`: `await expect(h.db.query('DELETE FROM admin_audit')).rejects.toThrow('append-only')`.
- **Confidence:** High.

### A-ADM-5 [Low] A failed Stripe step is marked "needs attention" with no server log of why

- **Where:** `src/admin/operations.ts:247-268` (`externalOperation` catch block).
- **What the code does now:** `catch {` drops the error. The operation row gets a fixed sentence ("One or more steps may have completed…"). Nothing goes to the log.
- **Why it matters:** a refund or cancellation Stripe refuses (missing permission on the restricted key, card-network error) cannot be diagnosed from the server. The replay window for the action is 23 hours and can lapse before anyone knows the cause.
- **How to fix:** catch the error and log its class and code only (never the message, which can echo request data):
  ```ts
  } catch (error) {
    deps.log.warn(
      { operationId: input.id, action: input.action,
        error: error instanceof Error ? error.name : typeof error,
        code: (error as { code?: string; statusCode?: number }).code
          ?? (error as { statusCode?: number }).statusCode },
      'owner action needs attention',
    );
  ```
  (`deps.log` already exists on `Deps`; widen the function's `Pick<Deps, …>` to include `'log'`.)
- **How to check:** in `test/admin-owner.test.ts`, the "cancellation fails" case: the captured log must contain `owner action needs attention` and must not contain the fake Stripe error text.
- **Confidence:** High.

### A-ADM-6 [Info] Notes on the owner console (no change strictly needed)

- After a recovery-code sign-in, registering a new passkey makes the session fully "fresh" for 5 minutes (`src/admin/auth.ts:315-318`, exemption at `src/admin/routes.ts:134-147`). `docs/owner-console.md:19-20` says a recovery sign-in "does not unlock billing or support changes"; that is true only until the new passkey is registered. Either fix the sentence, or leave `verified_at` empty on that registration so the owner must pass `/reauth` with the new key first.
- `GET <admin>/billing/invoices/:id` (`src/admin/billing.ts:442-460`) calls Stripe with the raw path value before checking it belongs to a local subscription, and runs an `UPDATE refunds` on a GET. Harmless (owner-only, same-site cookie). Hardening: check `/^in_[A-Za-z0-9]{8,}$/` first.
- `scripts/stripe-setup.ts --live` reads the key from the environment or a hidden prompt (never the command line, good) but on a host where `STRIPE_SECRET_KEY` and `STRIPE_ALLOW_LIVE=yes` are both set it changes live products, prices, and the default customer-portal settings with no "are you sure". Hardening: require typing the `acct_…` id, and an explicit `--key-from-env` flag.
- Any identity that passes Cloudflare Access but is not the enrolled owner writes an `owner access refused` audit row per request (`src/admin/routes.ts:93-107`). Fine while the Access policy admits only the owner's email; rate-limit it if the policy ever widens.
- Paths with no automated test (correct by reading, but a change could break them unnoticed): a recovered session must get 428 on business posts but 200 on `/register/*`; `POST /sessions/revoke` and `/passkeys/rename`; the `safeReturn` open-redirect guard on `/reauth`; setup-code sign-in while passkeys exist; `GET <admin>/email/:id`; the billing search.

## 4. Plenipo (desktop app, its servers, and the website): findings

### 4.1 The servers: relay, phone page, website updaters, and the phone lock

### P-SRV-1 [High] The relay, the phone page, and the website are installed from GitHub Releases or `main` with no signature, and the relay updater runs the download as root

- **Where:**
  - `crates/relay/deploy/update-relay.sh`: runs as root every 15 minutes (`:62-64` requires root; `plenipo-relay-update.service` has no `User=`). `:87` asks GitHub for the latest release; `:122-128` downloads the binary and a `.sha256` from the same release and compares them; `:130` runs `"$staging/plenipo-relay" --version` as root before any sandbox; `:149-150` switches and restarts.
  - `apps/remote/deploy/update-page.sh:88-99, 121-137`: same pattern for the phone page zip (runs as the deploy user). The checks at `:132-137` (the page must name `wss://relay.getplenipo.com` and no test address) are satisfied by any page that keeps those strings.
  - `apps/website/deploy/auto-release.sh:128-131, 192-209`: builds and runs a Docker image from `main` whenever the CI and Website workflows passed at that commit (`:71-82`).
  - The Release workflow (`.github/workflows/release.yml:357, 397`) makes the `.sha256` with `sha256sum` in the same job. No signing step exists for these assets; `grep` for the updater key finds only the desktop installer path (`apps/desktop/src-tauri/src/update_host.rs`, `crates/capabilities/src/updates.rs:141-143`, which verify a minisign signature with a key built into the app).
- **What the code does now:** a checksum that travels with the file proves the download was not corrupted. It proves nothing about who made the release.
- **Why it matters:** anyone who can publish a GitHub release on `Seckcey/plenipo` (a stolen maintainer token, a compromised Actions run, a malicious dependency in the release job, a collaborator account) gets, within 15 minutes: (1) root on 8 West's shared server, because the relay updater executes the download as root; (2) a phone page of their choosing at `remote.getplenipo.com`, which every paired phone loads on next open. The page is the trust root for every phone: it holds the phone's Noise key and runs the passkey step, so a hostile page can sign in silently and approve anything on every paired PC. `docs/adr/ADR-146` (where the phone page lives) says the page is trusted "the same way you trust Plenipo's installer", and the installer _is_ signed (ADR-038); the page and relay are not.
- **How to fix:**
  1. Sign both assets in the Release workflow with the updater key already held in the `release` Environment (minisign format; `pnpm tauri signer sign` or `minisign -S`), producing `plenipo-relay-<v>-linux-x86_64.sig` and `plenipo-phone-page-<v>.zip.sig`, uploaded with the release.
  2. In both scripts, install the public key once by hand (`/etc/plenipo-relay/relay.pub`, `/srv/8west/apps/plenipo-phone-page/page.pub`, root-owned) and verify before anything else:
     ```sh
     curl -fsSL --retry 3 -o "$staging/$program.sig" "$DOWNLOADS/$tag/$program.sig" || fail "no signature for $program"
     minisign -Vm "$staging/$program" -x "$staging/$program.sig" -p /etc/plenipo-relay/relay.pub > /dev/null \
       || fail "$program is not signed with 8 West's updater key"
     ```
     Keep the sha256 check too; it is no longer the gate. (A stronger alternative or addition: `actions/attest-build-provenance` in the Release workflow and `gh attestation verify <file> --repo Seckcey/plenipo` in the scripts.)
  3. Never run the download as root. In `update-relay.sh` replace line 130 with `says="$(runuser -u plenipo-relay -- "$staging/plenipo-relay" --version 2>/dev/null || true)"` and `chown root:plenipo-relay "$staging" && chmod 0750 "$staging"` first.
  4. Refuse downgrades: both scripts keep `VERSION` in `state/current.env`; refuse a smaller version unless `--version` was given by the operator.
  5. For the website: the fix is G-1 (protect `main`), plus the digest pinning in P-WEB-2.
  6. Write a short ADR ("the page and the relay are signed like the installer") amending ADR-146 §2 and ADR-149 §7.
- **How to check:** point `REPO_API`/`DOWNLOADS` at a local stand-in that serves (a) a correctly signed asset → installed; (b) a matching sha256 with no or a wrong signature → `STOPPED: … not signed`; (c) a lower version → refused. Wrap the relay binary in a stub that records `id -u` and assert it is never 0.
- **Confidence:** High for the missing signature and the root execution (lines quoted). The blast radius through the page follows from ADR-146's own trust statement.

### P-SRV-2 [High] Pairing a phone can be taken over by whoever scans the code first; the "Is this your phone?" question shows only a name the phone chose

- **Where:** `crates/remote/src/service.rs:1258-1266` (`pairing_meeting`, third handshake message): the first phone to finish the Noise handshake with the right code becomes the only candidate; any later phone with the same correct code is closed with no message (`self.close(&mut st, conn)`). `service.rs:446-450` (`PairingView::Asking { name, browser, since }`) shows exactly what the phone sent in `PairHello` (`protocol.rs:26-31`), cleaned only for length and control characters. The owner's own phone, if it loses the race, shows "That didn't work: the code may be wrong or used" (`apps/remote/src/pages/pairing-words.ts:16`). Design claim: `docs/adr/ADR-141-pairing-a-phone.md:55-57` says the question "stops someone who photographed your screen from adding their phone first".
- **Why it matters:** the owner clicks **Add a phone**; the code and QR are on screen for up to 10 minutes. Someone who can see the screen (a screen share, a video call, a photo, screenshot malware) scans first, naming their phone "iPhone". The owner's own scan a moment later fails with a generic error. The PC now asks "Is this your phone? iPhone · Safari on iPhone · just now", the owner, who just scanned, clicks **Add**, and the stranger's phone completes the passkey step and becomes a paired device with the full ADR-145 list: approve, stop, send objectives. Nothing on the PC lets the owner tell the two phones apart, so the ADR's claim does not hold. Precondition: seeing the screen during the window and scanning first. The cryptography is sound; this is a user-verification gap.
- **How to fix:**
  1. Show a short check string from the handshake on both screens and tell the owner to compare them. Both sides already have the handshake hash (`snow`: `hs.get_handshake_hash()`; page: `Handshake.handshakeHash`, `apps/remote/src/lock/noise.ts:270-272`). On the PC, in `pairing_meeting` before line 1245:
     ```rust
     // Six digits both screens show, from the meeting's own hash: a stranger's meeting makes different ones.
     let check = {
         let h = hs.get_handshake_hash();
         let n = u32::from_be_bytes([h[0], h[1], h[2], h[3]]) % 1_000_000;
         format!("{n:06}")
     };
     ```
     Carry `check` in the candidate and in `PairingView::Asking`; show it in `DevicesSettings.tsx` next to the name ("The phone shows the same six digits: 482 913"). On the page, compute the same from `hs.handshakeHash` in `Pairing.start` (`apps/remote/src/line/phone.ts:271-277`) and show it under "Your PC is asking: Is this your phone?" (`Pair.tsx:221-226`). In XXpsk3 the hash after message 3 includes the code and both static keys, so a stranger's meeting cannot show the owner's digits.
  2. Tell the losing phone why: at `service.rs:1260` send `PairStep::Refused { message: "Another phone already used this code. If that was not you, click Cancel on your PC and start again." }` before closing.
  3. Amend ADR-141 §6: the owner compares the six digits; a name alone proves nothing.
- **How to check:** in `crates/remote/tests/remote.rs`, start pairing, run two `NetPhone::start_pairing` with the same code; assert the PC's `PairingView::Asking.check` equals the first phone's digits, differs from the second phone's, and the second phone hears `PairStep::Refused`. By hand: pair with two phones at once and read both screens.
- **Confidence:** High (candidate selection, silent close, and the self-reported name are all in the quoted lines).

### P-SRV-3 [Medium] The 30-minute phone sign-in idle limit is extended by reads the PC itself triggers

- **Where:** `crates/remote/src/service.rs:1527-1529`: `if signed && !from_notice { live.last_request = now; }` (`again` is in scope at `:1473` but unused). `apps/remote/src/session-context.ts:93-96`: on every `changed` event from the PC the page re-reads with `again: true`. The PC emits `changed` for task state, agent results, approvals, and more. Promise: ADR-142 (the phone proves it is you), "30 minutes after your last tap, 12 hours at most".
- **Why it matters:** while workers run, the PC pushes `changed` every few seconds, the open page re-reads automatically, and each re-read resets the idle clock. The 30-minute window becomes 12 hours exactly when approvals arrive. A phone left unlocked on a desk can approve all day.
- **How to fix:** `if signed && !from_notice && !again { live.last_request = now; }` and note in the doc comment on `PhoneSays::Ask.again` (`protocol.rs:346-352`) that automatic re-reads do not extend the sign-in.
- **How to check:** `crates/remote/tests/remote.rs`: sign in, move the test clock 29 minutes, send an ask with `again: true`, move 2 more minutes, `tick()`, assert the phone hears `Event::SignedOut { why: Idle }`.
- **Confidence:** High.

### P-SRV-4 [Medium] A hostile relay can grow the PC's connection table without limit

- **Where:** `crates/remote/src/service.rs:984-997` (`Joined` from the relay), `:1056-1113` (`phone_joined`), insert at `:1093-1100`. No count of `st.conns` exists anywhere (grep for `conns.len()` finds nothing); entries leave only on `Left`, `Close`, a failed meeting, or `relay_down`.
- **Why it matters:** ADR-143 treats the relay as untrusted, and the relay knows every paired phone's ID from the passes. It can send `{"t":"joined","conn":"cN","phone":"<known id>"}` with a fresh `conn` forever and never send `left`. Each one allocates a Noise handshake state plus an assembler (a kilobyte or two). At network speed that is hundreds of MB a minute until Windows kills the Plenipo process and the owner's workers stop. These joins never fail, so the failed-meeting brake never trips.
- **How to fix:** cap per device and in all. Add `const CONNS_PER_DEVICE: usize = 4;` and `const MAX_CONNS: usize = 96;` and, before the insert in `phone_joined` (and in `mailbox_joined`):
  ```rust
  let mine = st.conns.values().filter(|c| c.kind == Kind::Phone(id.clone())).count();
  if mine >= CONNS_PER_DEVICE || st.conns.len() >= MAX_CONNS {
      drop(st);
      self.send(ToRelay::Close { conn: conn.to_owned() });
      self.count_failed_meeting(); // a relay that keeps doing this trips ADR-143 §8's stop
      return;
  }
  ```
- **How to check:** in `crates/remote/tests/remote.rs`, feed `RelayToPc::Joined` 1,000 times with fresh `conn` names and assert the PC sent `Close` for all but 4 and that `view().meetings_stopped_until` is set.
- **Confidence:** High.

### P-SRV-5 [Medium] One leaked weekly answer lets anyone register unlimited PC identities at the relay, and the per-connection outgoing queue is bounded by count, not bytes

- **Where:** `crates/relay/src/pc.rs:82-90`: the hello is accepted when the Ed25519 proof checks and the weekly answer verifies; nothing ties the answer to the relay key or limits how many keys one answer may bring. The answer is a plain file on every Pro PC (`apps/desktop/src-tauri/src/license_host.rs:36`, `license.json`) and the relay accepts one up to 30 days old. `crates/relay/src/http.rs:103` and `crates/relay/src/limits.rs:58`: each connection's outgoing queue is `mpsc::channel::<Message>(256)`, and a message can be up to 96 KB, so a phone that never reads can hold about 24 MB before the writer kills it. `limits.rs:45-49`: 512 connections in all, 40 phones per PC; `plenipo-relay.service`: `MemoryMax=128M`, `Restart=always`.
- **Why it matters:** with one Pro `license.json` (a backup, a support bundle, a second-hand PC), a handful of addresses, a few fake PC keys, and a few phone connections that never read, an attacker can queue more than 128 MB; systemd kills and restarts the relay, every real PC and phone drops, and the attacker repeats. The same answer also gives a non-customer free use of the relay, which ADR-143 §3 set out to prevent.
- **How to fix:** (1) bound the outgoing queue by bytes: keep a per-link `AtomicUsize` of queued bytes in `hub.rs`; in `Link::send_message`, if `queued + m.len() > 1_048_576` then kill the connection instead of queuing; subtract after each write; lower `outgoing_queue` to 32. (2) Limit PCs per license without keeping the key ID: after `verify`, hash the key ID with a random per-process salt, keep a `HashMap<[u8;32], usize>` of live PCs per hash in `State`, and refuse above 3 (`Limits::pcs_per_license: 3`). (3) Count PC connections per address (`Limits::pcs_per_address: 4`). (4) Longer term, in the account service, let the weekly answer carry the PC's relay-key fingerprint so the relay can bind answer to key.
- **How to check:** `crates/relay/tests/hardening.rs`: (a) a phone that never reads while the PC sends 2 MB: the phone connection is closed and process memory stays flat; (b) four PC keys with the same test answer: the fourth hello is refused `too_many_tries`.
- **Confidence:** High for the mechanics; Medium for how easy it is in practice.

### P-SRV-6 [Medium] The relay's per-address table is never swept

- **Where:** `crates/relay/src/hub.rs:162-211`. `forget_idle_address` runs only from `enter` (on a refusal) and `leave`, and only for that same address, and only when `new.count(now) == 0`. A connection that opens and closes within its first minute leaves `new.count == 1`, so its entry stays; nothing looks at it again unless the same address returns. The comment at `:202` ("takes no memory") promises the opposite.
- **Why it matters:** each entry is about 100 bytes. An attacker with many source addresses (IPv6 is keyed by /64 and providers hand out millions of /64s; or a modest IPv4 pool) makes one connection per address, closes it, and moves on. Around 1.3 million entries reach `MemoryMax=128M`, the relay is killed and restarted, and all customers drop. No credentials needed.
- **How to fix:** sweep on a timer. In `Relay::start` (`lib.rs`), spawn a task that every 60 s calls a new `hub.sweep_addresses()`:
  ```rust
  pub fn sweep_addresses(&self) {
      let now = self.now();
      let mut st = lock(&self.state);
      st.addresses.retain(|_, u| u.open > 0 || u.new.count(now) > 0 || u.tries.count(now) > 0);
  }
  ```
  Add a hard cap (`Limits::addresses_remembered: 100_000`): when reached, sweep inline and, if still full, answer `503 full`.
- **How to check:** unit test in `hub.rs` with the test clock: `enter`/`leave` for 10,000 distinct addresses, move the clock 61 s, call `sweep_addresses()`, assert the table is empty. Add `Stats::addresses` so the hardening tests can read the count.
- **Confidence:** High.

### P-SRV-7 [Low] Relay and pairing hardening items

- **No `Origin` check on the WebSocket upgrade** (`crates/relay/src/http.rs:60-72, 171-179`). Any website the owner visits can open 40 sockets to `wss://relay.getplenipo.com/plenipo/v1/phone` and send junk; each is refused `bad_pass` and counted against the visitor's address, so the owner's own phone and PC behind the same home address get `429` for the rest of the minute. A nuisance, not a break-in. Fix: after computing the role, read the `Origin` header; for the phone path accept only `https://remote.getplenipo.com` (or `http://localhost:*`/`http://127.0.0.1:*` under the `test-hooks` feature) or no `Origin` at all; for the PC path refuse any `Origin` (the PC is not a browser); make the allowed origin a `Config` field. Test in `hardening.rs`: upgrade with `Origin: https://evil.example` → 403.
- **Online/offline oracle for a PC** (`crates/relay/src/hub.rs:374-387`): the reply differs (`pc_offline` vs `bad_pass`) before the pass signature is checked, so an unsigned payload naming a PC's fingerprint tells whether that PC is connected. A removed phone still knows the fingerprint. Fix: keep a bounded memory of recently seen PC keys (fingerprint → key, last seen; cap 10,000, forget after 24 h), check the pass against it when the PC is absent, and only then answer `pc_offline`; otherwise `bad_pass`.
- **Pairing uses `Noise_XXpsk3`** (`crates/remote/src/noise.rs:22`, `apps/remote/src/lock/noise.ts:204-205`): the pairing code is mixed in only at the end of message 3, after the phone's long-term public key was sent, so a relay that answers message 1 itself can learn the phone's static public key (not the name, and it cannot finish the meeting). That key is the phone's identity in every later meeting, so the relay could link one phone across pairings. Fix: switch both sides to `Noise_XXpsk2_25519_AESGCM_SHA256` (`[["e"], ["e","ee","s","es","psk"], ["s","se"]]`), regenerate `contracts/phone-relay/v1/noise-vectors.json` (`PLENIPO_WRITE_CONTRACT=1 cargo test -p plenipo-remote contract`), re-run `apps/remote/src/lock/lock.test.ts`, and note it in ADR-143 §5.
- **`cloudflare` client-address mode trusts `CF-Connecting-IP` from any direct connection** (`crates/relay/src/http.rs:207-214`). The README has the operator start with a DNS-only record (origin IP public) and later switch to the orange cloud with `PLENIPO_RELAY_CLIENT_ADDRESS=cloudflare`; anyone who reaches the proxy directly can then send a random `CF-Connecting-IP` per connection and dodge every per-address limit. Fix: trust the header only when the proxy's peer (the last `X-Forwarded-For` hop) is in Cloudflare's published ranges (a const table refreshed with releases), else fall back to `forwarded()`; and tell the operator to firewall 80/443 to Cloudflare's ranges when switching.

### P-SRV-8 [Info] Phone notes (by design, recorded so the accepted risk is understood)

- Passkey registration uses `attestation: "none"` (`apps/remote/src/lock/passkey.ts:518`), so the PC cannot tell a platform passkey from a key the page made itself. Standard for passkeys and fine against the stated threats; it is why P-SRV-1 (page signing) is the real guarantee. Add a sentence to ADR-142 §2.
- No per-phone request rate limit on the PC (`crates/remote/src/service.rs:1578-1583`); only the page can produce sealed asks, so this is robustness. Optional: a token bucket per sign-in (60 asks per 10 s).
- A signed-in phone reads the exact command line of each approval and project folder paths (`remote_host.rs:370-401`, `Approvals.tsx:66`) and the AI's answers. This is what ADR-145 promises; the accepted "unlocked phone within the sign-in window" risk includes reading, not only approving.

### 4.2 The desktop app and licensing

### P-DESK-1 [Medium] Pro can be kept without a successful check by deleting one file every 30 days; the key's own paid-through date is never enforced

- **Where:** `crates/licensing/src/license.rs:136-162` (`License::load`), `crates/licensing/src/state.rs:83-95` (`Record::entered`) and `:164-223` (`Record::status`), `apps/desktop/src-tauri/src/license_host.rs:154-156` and `:435-437` (`read_record_bytes` → `unwrap_or_default()`).
- **What the code does now:** the 30-day "no check" grace (ADR-022, the 30 days with no successful check) is anchored in a plain JSON file, `license.json`, next to the Ledger. If the file is missing or unreadable, `Record::default()` is used; its `key_id` is empty, so `License::load` calls `Record::entered(...)`, which sets `entered_at = now` and `answer = None`. `Record::status` then returns Pro with reason "not checked yet" for another 30 days. `status()` copies the key's `paid_through` for display (lines 193-196) but never compares it with `now`.
- **Why it matters:** an owner whose subscription ended can (1) block `CHECK_ADDRESS` in the hosts file or firewall so every weekly check fails, (2) every 30 days quit Plenipo and delete `%LOCALAPPDATA%\com.eightwest.plenipo\license.json` (the key itself stays in Credential Manager), (3) start Plenipo and get Pro for 30 more days. An "ended" answer already received is forgotten the same way. The signatures are sound, so this is not forgery, and it needs the owner's own PC, so it is Medium, not Critical.
- **How to fix:**
  1. Make the key's paid-through date a ceiling in `Record::status` (after the `match a.state` block, before the grace check):
     ```rust
     // A key is good until its own paid-through date plus the grace, unless a signed answer
     // from 8 West says the paid period reaches further (a renewal). A record deleted or
     // edited by hand cannot move this ceiling: only 8 West's signed answer can.
     let ceiling = [
         Some(key.payload().paid_through),
         answer.as_ref().and_then(|a| a.paid_through),
         answer.as_ref().and_then(|a| a.ends_at),
     ]
     .into_iter()
     .flatten()
     .max()
     .unwrap_or(key.payload().paid_through);
     if status.edition == Edition::Pro && now > ceiling.saturating_add(GRACE) {
         status.edition = Edition::Free;
         status.reason = Reason::NoCheck;
     }
     ```
     For this to work on renewals, the account service must fill `paid_through` on every `active`/`cancelled` answer (the contract already has the field; `plenipo-account/src/licensing/check-route.ts` passes `state.paidThrough`, so check `state.ts` there sets it from Stripe's `current_period_end`).
  2. Optional, closes the "delete the file" trick itself: keep a copy of `entered_at` and the newest signed answer in the Vault (Credential Manager) next to the key, and on start use whichever of file or Vault is newer.
  3. Say in ADR-022 and in Settings → License that Pro needs a successful check at least every "paid period + 30 days".
- **How to check:** add a test to `crates/licensing/src/state.rs`: a record freshly "entered" at a time past the test key's `paid_through + GRACE + 1` must be Free; the same record with a signed answer whose `paid_through` is later must be Pro. For step 2, in `license_host.rs` tests: enter a key, get an "active" answer, move the clock 31 days with the service answering 503, delete the record file, build a new `LicenseHost` on the same store, and assert the edition is Free.
- **Confidence:** High. I traced `read_record_bytes` → `Record::default()` → `License::load` → `Record::entered` → `status()` by hand.

### P-DESK-2 [Low] "Open in another program" still hands a few script types to Windows

- **Where:** `crates/capabilities/src/broker/owner_files.rs:46-204` (the `RUNS` list), `:206-212` (`runs`), `:847-868`; `apps/desktop/src-tauri/src/files_commands.rs:195-212` (`open_file_outside`).
- **What the code does now:** before starting `explorer.exe <file>`, the name's extension is checked against `RUNS`, a long list of things that run when double-clicked (exe, ps1, py, bat, lnk, Office macro types, installers…).
- **Why it matters:** these run-on-double-click types are missing (confirmed with grep: zero matches): `ahk`, `ahk2` (AutoHotkey), `rb`, `rbw` (Ruby), `php`, `lua`, `tcl`, `tk`, `r`, `jl` (Julia), `nu` (Nushell), `groovy`. A worker can write such a file into the project and name it something tempting; if the owner picks "Open in another program" and has that interpreter installed, the file runs. Impact depends on what is installed, so Low.
- **How to fix:** add those strings to `RUNS` (keep the list sorted). Consider also treating `html`, `htm`, `xhtml`, `svg`, `mht`, `mhtml` as "open inside Plenipo only", because they open in the owner's browser on a `file://` page where script runs.
- **How to check:** extend the `runs()` test with `assert!(runs("evil.ahk")); assert!(runs("evil.RB"));` and a negative `assert!(!runs("notes.md"))`.
- **Confidence:** High that the extensions are absent; Medium on impact.

### P-DESK-3 [Low] AI-tool updates run each vendor's own updater with no check of what it downloads, and SECURITY.md reads as if every update is signed

- **Where:** `crates/capabilities/src/ai_tools.rs:1373-1376` and `:1558-1584`; adapters `crates/runtime/src/agent/claude_code.rs:175-177` (`claude update`), `codex.rs:231-238`, `kimi.rs:178-180` (`kimi upgrade --yes`), `grok.rs:227-229`, `copilot.rs:337-339`; Guard check `crates/guard/src/ai_tools.rs:83-122`.
- **What the code does now:** "Update" runs the vendor's command through the supervisor with a cleared environment and no shell. Guard only checks the tool is not busy; there is no approval card, checksum, or signature. With "update by themselves" switched on (off by default), the daily look starts these on its own. Plenipo's own updater, by contrast, verifies a minisign signature bound to the version.
- **Why it matters:** this is a reasonable design (Plenipo cannot sign Anthropic's or OpenAI's binaries), but SECURITY.md's line "Plenipo installs an update only when the owner chooses Install now, and only an installer signed with 8 West's updater key" reads as if it covers AI tools too. The newest-version look itself is safe: read-only GETs to fixed npm/GitHub addresses through Guard, and the test stand-in address is accepted only when it starts with `http://127.0.0.1:` (an empty value from the Release workflow is ignored).
- **How to fix:** (1) add to SECURITY.md under updates: "AI tools (Claude Code, Codex, Kimi, Grok, Copilot) are updated by their makers' own update commands; Plenipo starts them only when you press Update, or daily when you switch 'update by themselves' on, and checks the tool answers afterwards, but it cannot check what the maker's updater downloads." (2) keep the switch off by default (it is). (3) record the exact command and before/after versions in the `ai_tool.update_started` event so a bad update can be traced.
- **How to check:** doc review; extend the update tests in `ai_tools.rs` to assert the event payload has `command`.
- **Confidence:** High.

### P-DESK-4 [Info] Desktop notes (no change strictly needed)

- **Ledger JSON export is not run through the secret filter** (`crates/ledger/src/maintenance.rs:208-248`, `export_json`), unlike the diagnostics zip (`apps/desktop/src-tauri/src/diagnostics.rs:90-102, 150-168`). Secrets are redacted when events are written, so this is defence in depth: give `export_json` an optional text filter and pass the broker's filter from `commands::export_ledger`.
- **Tool-server admission falls open** when Windows cannot say which process owns a connection (`crates/capabilities/src/broker.rs:1527-1562`: `Holders::Unavailable` → admitted with a notice and a Ledger event). Same-user attackers are out of scope in SECURITY.md; a Guard option "refuse tools when the connecting program cannot be checked" would let cautious owners fail closed.
- **`allowDowngrades: true`** in `apps/desktop/src-tauri/tauri.conf.json` affects only a manual installer run. The updater only offers a newer version and the signature is bound to the version, so it cannot be made to downgrade. Mention in the install docs.
- **One webview XSS would equal full compromise** (`write_terminal`, `save_file`, `install_update` are callable from the main window). No HTML sink was found in `apps/desktop/src` (grep for `dangerouslySetInnerHTML`, `innerHTML`, `insertAdjacentHTML`, `document.write`, `eval(`, `new Function`), `window.open` is used once for `about:blank`, and the CSP allows scripts from `'self'` only. Add the ESLint rules `react/no-danger` and `no-eval` to `pnpm check` so it stays that way.
- **WordPress connection host** could be pointed at a private address by DNS rebinding; the host is the owner's own choice, Guard already refuses IPs, localhost, ports, and user-info; no change.

### 4.3 Guard and the worker's tools

### P-GUARD-1 [Medium] The default approved-commands list lets a worker run code it wrote itself with no approval card, and the list contradicts its own comment

- **Where:** `crates/guard/src/defaults.rs:230-286`. The comment at `:230-233` says "Script runners (`npm run`, `make test`) run a project's own scripts with the owner's account, so they are not approved for every project: the owner approves them where the project is trusted (ADR-034)." The `approved` list that follows includes `cargo test *`, `cargo build *`, `npm test *`, `pnpm test *`, `pnpm build *`, `pnpm check *`, `yarn test *`, `yarn build *`, `jest *`, `vitest *`, `pytest *`, `./gradlew test *`, `./gradlew build *`, `make lint *`, `make check *`. A `./gradlew` program resolves to a file inside the project and runs as `Origin::Project` (`crates/capabilities/src/broker.rs:3374-3417`, `program_path`); `program_key` strips `.bat`/`.cmd`/`.exe`/`.com` (`crates/guard/src/commands.rs:12-29`), so a worker-written `gradlew.bat` matches the rule `./gradlew test *`. An approved-list match is allowed with no card (`crates/guard/src/engine.rs`).
- **What this means:** every one of those commands runs whatever the project says: `npm test` runs the `test` script in `package.json`, `cargo test` compiles and runs `build.rs` and the tests, `make lint` runs the `Makefile`, and `./gradlew` runs a file the worker can create. A Developer worker that can change project files can therefore run any program it likes, with the owner's account, by writing it into one of these hooks and calling the approved command. `SECURITY.md` says this openly ("treat a project's scripts as code you trust, and approve script runners only for projects you trust"), so this is a documented trade-off, not a hidden hole. The defaults do not match that advice, though: the comment excludes `npm run` and `make test` but the list approves `npm test`, `make lint`, `make check`, and a project-local `./gradlew`, which are the same thing.
- **How to fix:** (1) Make the default list match the comment: move `npm test *`, `pnpm test *`, `pnpm build *`, `pnpm check *`, `yarn test *`, `yarn build *`, `yarn lint *`, `make lint *`, `make check *`, `./gradlew test *`, `./gradlew build *` from `approved` to `ask`, so the first use in each project shows a card; keep tool invocations that do not run project hooks approved where they truly do not (note that `cargo test`/`cargo build` run `build.rs` and proc-macros, so they belong with the script runners too). (2) Never approve an `Origin::Project` program from the default list: in `engine.rs`, when the resolved origin is `Origin::Project`, treat an `approved` match as `ask` unless the owner's own rule names `./<path>` explicitly. (3) In the "Hire a Developer" flow, show the SECURITY.md sentence once ("Build and test commands run this project's own scripts with your account; approve them only for projects you trust"). (4) Say in `docs/faq.md` which default commands run project code.
- **How to check:** in `crates/guard/src/engine.rs` tests: a `CommandLine` of `./gradlew test` resolved as `Origin::Project` yields `Verdict::Ask` with the defaults; `cargo fmt --check` still yields `Allow`.
- **Confidence:** High on the list contents and resolution (read `defaults.rs`, `commands.rs`, `broker.rs:3374-3417`); the severity reflects that the behaviour is documented.

### P-GUARD-2 [Medium] `git diff`, `git log`, and `git status` with no path cover the whole repository, not only the project folder

- **Where:** `crates/capabilities/src/broker.rs:3833-3866` (the `GitStatus`, `GitDiff`, and `GitLog` arms add `-- <path>` only when the worker gave a path) and `:3959` (`programs::git_args(w.root(), &op)` → `git -C <project folder> …` with no pathspec). `crates/capabilities/src/broker/git_tools.rs:69-76` (`prefix()` via `rev-parse --show-prefix`, `in_folder()`) and its test at `:607-640` show that a project folder may be a sub-folder of its repository.
- **What the code does now:** with no path argument, git runs from the project folder but, as git does, reports on the whole work tree. When the project folder is a sub-folder of a larger repository, `git diff` prints the contents of changed files above the project folder, and `git status` and `git log` print their names and commit lines. Only blocked-list files above the folder are excluded (`git_tools.rs:106-112`).
- **Why it matters:** the first promise in `SECURITY.md` is "A worker stays inside its project's folder. It cannot read or change files outside it." A worker whose project is `repo/apps/site` can read every uncommitted change anywhere in `repo/` with one `git diff`. Precondition: the project folder is a sub-folder of a repository with changes elsewhere, which is common in monorepos.
- **How to fix:** in the three arms, when the worker gave no path, confine git to the folder:
  ```rust
  // git -C <root> makes "." the project folder, whatever the repository's top is.
  if !op.iter().any(|a| a == "--") {
      op.extend(["--".to_owned(), ".".to_owned()]);
  }
  ```
  Apply for `GitDiff` with `path == None`, `GitLog` with `path == None`, and `GitStatus`; `GitLook::Diff { path: None }` should pass `"."` to `look_at` too, so the exclusion list matches what is shown.
- **How to check:** extend `a_folder_inside_the_repository_sees_its_own_paths` in `git_tools.rs:610`: write `a.txt` at the repository top, run `diff -- .` from `sub`, and assert `a.txt` is absent; add a broker test that `prepare(GitDiff { staged: false, path: None })` yields an op ending in `["--", "."]`.
- **Confidence:** High (lines read; `git_args` adds no pathspec).

### P-GUARD-3 [Medium] The project-folder check happens once at resolve time; the later read, write, rename, or delete does not check again

- **Where:** `crates/guard/src/paths.rs:206-233` (follow links of the deepest existing part, then `real.starts_with(&self.root)`); `crates/capabilities/src/files.rs:121, 178, 396, 407, 433, 447, 457, 464` (`fs::read`/`write`/`rename`/`remove_*` on `file.abs` with no root available and no re-check); `crates/capabilities/src/broker.rs:2644-2686` (carry-out hands the earlier `Resolved` to `files::*`), with an approval card possibly waiting in between (`broker.rs:2092-2107` then `:2174-2258`).
- **What the code does now:** `Workspace::resolve` canonicalises the path and checks it is inside the folder. The result (`Resolved.abs`) is used later as a plain path. Nothing re-canonicalises or opens with `O_NOFOLLOW` at the moment of the I/O.
- **Why it matters:** if a component of the path is replaced by a link between resolve and use (for example while an approval card waits), the write or delete lands wherever the link points. Making links needs a capability the worker usually lacks on Windows (symbolic links need developer mode or admin; junctions do not), so this is a hardening gap rather than an open door, but it sits under the product's first promise.
- **How to fix:** give `files::*` the root and re-check right before I/O:
  ```rust
  fn still_inside(file: &Resolved, root: &Path) -> Result<(), String> {
      let anchor = if file.exists { file.abs.as_path() } else { file.abs.parent().unwrap_or(&file.abs) };
      let real = dunce::canonicalize(anchor).map_err(|e| io(file.shown(), &e))?;
      if real.starts_with(root) { Ok(()) } else { Err(format!("{} now leads outside the project folder", file.shown())) }
  }
  ```
  Call it at the top of `read`, `read_text`, `write`, `edit`, `move_path`, and `delete` (carry-out already has `workspace`, so pass `workspace.root()`), and on Unix open the final component with `OpenOptions` plus `custom_flags(libc::O_NOFOLLOW)`.
- **How to check:** a Unix test in `files.rs`: resolve `docs/new.txt`, then remove `docs` and replace it with a symlink to a folder outside the root, then assert `write(&f, "x")` returns an error and the outside folder has no `new.txt`.
- **Confidence:** High on the code; Medium on how a worker would exploit it on Windows.

### P-GUARD-4 [Medium] A PowerShell script is a shell the owner's command lists never see

- **Where:** `crates/capabilities/src/broker.rs:3602-3630` (`Action::Script` → `powershell -Command -` with the script on stdin; `p.script = Some(script)`, `p.command` stays `None`); `crates/guard/src/engine.rs:623-637, 753-767, 788-804` (the blocked, always-ask, and approved lists are consulted only `if let Some(cmd) = request.command`); `crates/guard/src/sensitive.rs:352-354` (`script()` runs the word classifier only; the "deletes outside the project folder" check at `:342-347` is in `command()` and never runs for scripts); `crates/guard/src/defaults.rs:46, 164` (`PowershellExec` defaults to **Ask** in both built-in permission sets).
- **What the code does now:** by default every script asks, which is right. If an owner sets `PowershellExec` to **Allow** for a role, a script runs whatever programs it names with only the best-effort word scan standing in the way: the blocked-commands list (for example `rm *`) does not apply inside it, and the "outside the project folder" risk is never raised.
- **Why it matters:** "A worker cannot do anything its permission set does not allow" stops being true for that role the moment the switch is on, and the Settings page does not say so.
- **How to fix:** treat a script as never on the approved list. In `engine.rs`, before the approved-list check:
  ```rust
  if request.script.is_some() && request.command.is_none() {
      return decision(Verdict::Ask, Layer::Rule,
          format!("{} needs your approval: a PowerShell script is not on your approved commands list.", capitalized(request.summary)),
          risk, None, checks);
  }
  ```
  and in `sensitive.rs::script()`, also scan the script's words for `DESTRUCTIVE` programs followed by an argument that `escapes()` the folder. Say on the permission screen that **Allow** for scripts means "any program, no card".
- **How to check:** `engine.rs` test `inherent_risk_and_scripts`: with the role's `PowershellExec` level set to Allow and the script `Get-ChildItem`, assert `Verdict::Ask`; `sensitive.rs` test `scripts_are_checked_too`: `script("Remove-Item C:\\Users\\me\\other -Recurse")` → `Some(OutsideWorkspace)`.
- **Confidence:** High.

### P-GUARD-5 [Low] An absolute path skips the name checks that a relative path gets

- **Where:** `crates/guard/src/paths.rs:187-196` (absolute input: only `is_absolute()` is checked) versus `:197-202` (relative input: `check_name` on every component) and `:234-237` (`check_name` again, but only for components still to be created). `check_name` (`:77-106`) refuses `:` (drive or alternate data stream), trailing dots or spaces, device names, and `<>"|?*`.
- **What the code does now:** for an absolute path, the existing components never go through `check_name`. Confinement itself still holds (`:225-233` compares the canonical real path with the root), and the blocked-list name comes from the canonical path.
- **Why it matters:** the module header promises that alternate-stream names and names Windows silently changes are refused. Whether Windows' canonicalisation turns an existing `name:stream` into the plain file name was not verified from this Linux session; the fix costs nothing either way.
- **How to fix:** after `normalize` (`:203`), run `check_name` over every `Component::Normal` of the normalised path; the later loop at `:235-237` then becomes redundant.
- **How to check:** in `paths.rs` test `traversal_is_refused`: `w.resolve(<root>/src/main.rs:stream)` and `w.resolve(<root>/src/main.rs.)` must both be errors; under `cfg(windows)`, create the stream first so the existing-component case is exercised.
- **Confidence:** High on the code path.

### P-GUARD-6 [Low] `git status` and `git log` still name blocked files

- **Where:** `crates/capabilities/src/broker.rs:3833-3836` (`git status --short --branch`, no files list) and `:3853-3866` (`git log … -- <path>`: the path is resolved but not pushed into `files`, so Guard's blocked-file check at `crates/guard/src/engine.rs:564-575` does not run for it). `git_tools.rs:15` documents it: "`git status` and `git log` show names and commit lines, never contents, and stay as they are."
- **Why it matters:** names only, never contents. But the promise is "A folder listing leaves them out", and the file-tool listing does hide them (`files.rs:60-86`); `git status` shows `?? .env`, and `git log -- .env` lists the commits that touched it.
- **How to fix:** in the `GitLog` arm, push the resolved path into `files` so Guard refuses a blocked path as it does for `diff`; for `GitStatus`, append the same `:(top,literal,exclude)<blocked>` pathspecs that `git diff` uses (`git status` accepts pathspecs), or filter the `--porcelain -z` lines through `blocked_by` before returning them.
- **How to check:** `git_tools.rs` test `a_diff_leaves_blocked_files_out_and_counts_them`: add a `status --short -- :(top,literal,exclude).env.local` run and assert `.env.local` is absent; broker test: `prepare(GitLog { path: Some(".env") })` must put `.env` in `prepared.files`.
- **Confidence:** High.

### P-GUARD-7 [Low] "Is this program inside the project?" is decided on an un-canonicalised PATH entry

- **Where:** `crates/capabilities/src/broker.rs:3413-3416` (`Ok(p) if p.starts_with(ws.root()) => Origin::Project`), with `p` from `crates/capabilities/src/programs.rs:125-134` (`dir.join(n)` from PATH, never canonicalised) and `ws.root()` canonical (`crates/guard/src/paths.rs:144`).
- **Why it matters:** ADR-048 (secrets reach only the programs they are for) says a program inside the project folder gets no stored secret. The comparison is lexical, so a PATH or search-folder entry that reaches the project folder through a link, a short 8.3 name, or different letter case is classed as installed, and `secrets_for` (`broker.rs:634-658`) hands that file the secrets bound to its name. Precondition: an owner-configured search folder or PATH entry that points into the project through a link.
- **How to fix:** `Ok(p) if dunce::canonicalize(p).is_ok_and(|real| real.starts_with(ws.root())) => Origin::Project`.
- **How to check:** `broker.rs` test `a_program_in_the_project_folder_is_not_the_installed_one` (`:4177`): under `cfg(unix)`, symlink the project's `scripts` folder from outside, search through the link, and assert the origin is `Origin::Project`.
- **Confidence:** High.

### P-GUARD-8 [Low] The typed-text filter is a list, so some invisible Unicode format characters get through

- **Where:** `crates/capabilities/src/desktop.rs:137-157` (`refuse_hidden_characters`: `is_control()` except tab and line breaks, plus an explicit list of code points).
- **Why it matters:** the rule is "Text a worker types holds only visible characters, tabs, and line breaks." Unicode format characters outside the list are accepted, for example U+1D173 to U+1D17A (musical format controls) and U+1BCA0 to U+1BCA3 (shorthand format controls). The key filter itself matches the documented list.
- **How to fix:** add `'\u{1BCA0}'..='\u{1BCA3}' | '\u{1D173}'..='\u{1D17A}' | '\u{13430}'..='\u{1343F}'` to the match, or use the `unicode-general-category` crate and refuse `Format | Control` except `\t`, `\n`, `\r`.
- **How to check:** `desktop.rs` test `typed_text_has_no_hidden_characters` (`:681`): add `"\u{1D173}"` and `"\u{1BCA0}"` to the refused inputs.
- **Confidence:** High.

### 4.4 The marketing website and the release workflow

### P-WEB-1 [Low] The marketing website sends no `Strict-Transport-Security` header

- **Where:** `apps/website/nginx.conf:41-46` (the `add_header` block). Checked live: `curl -sI https://getplenipo.com/` returns CSP, X-Frame-Options, nosniff, Referrer-Policy, Permissions-Policy, and no `strict-transport-security`. Cloudflare is not adding one either.
- **Why it matters:** without HSTS, a browser sent to `http://getplenipo.com` (typed without https, an old link, a captive portal) makes one plain-HTTP request before the redirect. On a hostile network that one request can be answered with a fake page whose "Download" button points at a bad installer. This page is how customers get the installer.
- **How to fix:** either add one line after line 46 of `apps/website/nginx.conf`:
  ```nginx
  add_header Strict-Transport-Security "max-age=31536000; includeSubDomains" always;
  ```
  or turn it on in Cloudflare: **SSL/TLS → Edge Certificates → HTTP Strict Transport Security → Enable**, max-age 12 months, apply to subdomains. `includeSubDomains` is safe because `www.`, `account.`, `relay.`, and `remote.` are all HTTPS-only. Do not add `preload` yet. Then add a check to `.github/workflows/website.yml` after line 47: `curl -sI http://127.0.0.1:14380/ | grep -qi '^strict-transport-security: max-age=31536000'`.
- **How to check:** `curl -sI https://getplenipo.com/ | grep -i strict-transport` prints the header.
- **Confidence:** High (absent in the config and absent live).

### P-WEB-2 [Low] The website's Docker base images are pinned by tag, not by digest, and are built on the production host

- **Where:** `apps/website/Dockerfile:1` (`FROM node:22-alpine AS build`) and `:21` (`FROM nginx:1.30.5-alpine`); `apps/website/deploy/auto-release.sh:206-209` builds this on the server every 15 minutes when something changed.
- **Why it matters:** a tag can move. Two builds of the same commit can differ, and a mistaken or hostile upstream push reaches production with no review. The account repo already pins every runtime image by digest.
- **How to fix:** `FROM node:22-alpine@sha256:<digest> AS build` and `FROM nginx:1.30.5-alpine@sha256:<digest>` (get digests with `docker buildx imagetools inspect node:22-alpine`). Add a `docker` entry for `/apps/website` to `.github/dependabot.yml` so the digests are bumped weekly.
- **How to check:** `grep -c '@sha256:' apps/website/Dockerfile` prints 2 and the Website workflow still passes.
- **Confidence:** High.

### P-WEB-3 [Info] Release workflow: `contents: write` is granted to both jobs, and release safety depends on GitHub settings that live outside the repo

- **Where:** `.github/workflows/release.yml:36-37` (top-level `permissions: contents: write`), `:94` (`environment: release` / `dry-run`), `:96-103`.
- **What the code does now:** both the `relay` job (Linux build) and the `windows` job inherit `contents: write`; only `windows` needs it (`gh release create`).
- **Why it matters:** least privilege. More importantly, the workflow's safety assumes four settings this review could not see: (1) the Environment `release` has the owner as a required reviewer and limits deployment branches/tags to `main` and `v*`; (2) a tag ruleset protects `v*` tags; (3) `main` requires pull requests and passing checks; (4) the repository variable `PLENIPO_UPDATER_PUBLIC_KEY` and the Environment secrets are the real ones (the workflow only checks they are non-empty). `docs/development/code-signing.md` and ADR-052 (release signing behind the owner's approval) describe exactly these settings, so the intent is documented; confirm them once in the GitHub UI and write the date in `code-signing.md`.
- **How to fix:** delete lines 36-37; add `permissions: contents: write` under the `windows` job and `permissions: contents: read` under `relay`.
- **Confidence:** High on the workflow text; settings unverified by design.

### P-WEB-4 [Info] The account repo's publish job: what it assumes

- **Where:** `plenipo-account/.github/workflows/ci.yml:76-154`.
- **What it does:** only on a push to `main`, after the check and docker jobs pass, logs in to GHCR with the job's own token, builds for arm64, pushes `ghcr.io/seckcey/plenipo-account:sha-<commit>`, and records the digest in an artifact. No stored secret.
- **What it assumes:** that `main` is protected (anyone who can push to `main` can publish a production image that the checked updater may deploy within 15 minutes), and that the GHCR package stays private. See the process findings.

## 5. GitHub settings and process (both repositories)

### G-1 [High] `main` is the production deployment gate in both repos, and no branch ruleset enforces how code gets onto it

- **What was checked (GitHub API, 2026-10-02):**
  - `Seckcey/plenipo-account`: `GET /repos/Seckcey/plenipo-account/rulesets` → `[]` (no rulesets). The classic branch-protection API answered 403 (this session's token has no admin scope), so a classic rule may or may not exist.
  - `Seckcey/plenipo`: one ruleset named `plenipo` exists (rules: no deletion, no force-push on the default branch) but its `enforcement` is `disabled`.
  - The last five merged pull requests in each repo were opened and merged by the owner with zero reviews. That is fine for a one-person company; the point is that nothing _forces_ a pull request or green CI before a commit lands on `main`.
- **Why it matters:** three auto-deployers treat `main` (or a release built from it) as "approved":
  - `plenipo-account/scripts/checked_update.py` deploys the newest `main` commit with a green CI run to the live account service within 15 minutes (`scripts/plenipo-account-update.timer`), unless the commit touched the "compatibility" files.
  - `plenipo/apps/website/deploy/auto-release.sh` builds and runs the website from `main` after CI passes.
  - `plenipo/.github/workflows/release-notes.yml` has `contents: write` and runs on every push to `main`.
    A leaked token with push rights, a compromised collaborator account, or a mistaken `git push origin main` goes live with no second look.
- **How to fix (GitHub web UI, both repos):** **Settings → Rules → Rulesets → New branch ruleset.** Name `main`; target "Default branch"; enforcement **Active**; rules: _Restrict deletions_, _Block force pushes_, _Require a pull request before merging_ (0 required approvals is fine for a solo owner; keep _Dismiss stale approvals_ off), _Require status checks to pass_ (plenipo: `Docs (format, links)`, `Frontend…`, `Rust…`, `E2E…`, `Windows…`; plenipo-account: `Lint, types, tests`, `Docker image builds`), and _Require branches to be up to date_. Keep the bypass list empty (the owner can still use "Bypass" from the UI when truly needed, and GitHub records it). In plenipo, either enable the existing `plenipo` ruleset after adding the rules above, or delete it and create the new one.
- **How to check:** `gh api repos/Seckcey/<repo>/rulesets` shows `"enforcement": "active"`; a direct `git push origin main` of a test branch is refused.
- **Confidence:** High on what the API returned; the classic protection on `main` could not be read.

### G-2 [Medium] Private vulnerability reporting is off on the public repo, and SECURITY.md says so

- **What was checked:** `GET /repos/Seckcey/plenipo/private-vulnerability-reporting` → `{"enabled": false}`. `SECURITY.md` ("Current intake status, September 27, 2026") tells reporters there is no private channel.
- **Why it matters:** a researcher who finds a real hole in a product that runs AI agents with file, git, browser, and SSH access has no safe place to send it, so it either goes public or goes nowhere.
- **How to fix:** **Settings → Code security → Private vulnerability reporting → Enable** (free for public repos). Then update `SECURITY.md`: replace the "intake status" paragraph with "Use **Report a vulnerability** on the Security tab. We answer within 5 business days." Optionally add `security@8westventures.com` as a second channel.
- **How to check:** the Security tab shows a green "Report a vulnerability" button when signed out.
- **Confidence:** High.

### G-3 [Medium] `plenipo-account` has no Dependabot (or Renovate); `plenipo`'s Dependabot misses the website lockfile and Docker images

- **Where:** `plenipo-account/.github/` holds only `workflows/ci.yml`. `plenipo/.github/dependabot.yml` covers `github-actions`, `npm` at `/`, and `cargo` at `/`; the website is built with `npm ci` from `apps/website/package-lock.json` (a separate lockfile Dependabot's root `npm` entry does not update), and no `docker` ecosystem is listed.
- **Why it matters:** the live payment service gets no automated proposals for security patches at all. For the website, a root Dependabot bump of a website package makes `npm ci` fail in the Website workflow until someone regenerates `apps/website/package-lock.json` by hand.
- **How to fix:** create `plenipo-account/.github/dependabot.yml` with `github-actions` (`/`), `npm` (`/`, weekly, ignore major), and `docker` (`/`). In `plenipo/.github/dependabot.yml` add `npm` for `/apps/website` (same grouping as the root entry) and `docker` for `/apps/website`. See P-WEB-2.
- **How to check:** **Insights → Dependency graph → Dependabot** lists each directory; the next weekly run opens pull requests.
- **Confidence:** High.

### G-4 [Low] `plenipo-account`'s Dockerfile base image is not pinned by digest

- **Where:** `Dockerfile:8` and `:19` (`FROM node:22-bookworm-slim`), while every runtime image in `compose.yaml` is required to carry `@sha256:` (and CI tests that).
- **How to fix:** `FROM node:22-bookworm-slim@sha256:<digest>` on both lines; the `docker` Dependabot entry from G-3 keeps it current.
- **Confidence:** High.

## 6. Dependencies and secret scans

### D-1 Rust crates (`cargo audit`, 760 crates in `plenipo/Cargo.lock`)

| Advisory                                                                              | Crate                    | How it gets in                                                                                                            | What it means                                                                                                                                                                                                                       | What to do                                                                                                                                                                                                         |
| ------------------------------------------------------------------------------------- | ------------------------ | ------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| RUSTSEC-2023-0071 (medium, "Marvin" timing side channel; **no fixed version exists**) | `rsa` 0.10.0-rc.18       | `russh` 0.63.3 ← `plenipo-capabilities` (the SSH tool; `crates/capabilities/Cargo.toml:72` enables russh's `rsa` feature) | `crates/capabilities/src/ssh.rs:266-299` uses RSA only to sign with the owner's own client key when that key is RSA. An attacker would need to be the SSH server and time thousands of the client's signatures. Low practical risk. | Prefer Ed25519 server keys in the docs for "Servers". Watch `rsa` 0.10 for a release that closes the advisory and let Dependabot pick it up. If RSA client keys are not needed, drop the `rsa` feature from russh. |
| RUSTSEC-2024-0370 (unmaintained)                                                      | `proc-macro-error` 1.0.4 | `glib-macros` ← GTK ← Tauri's tray icon (Linux only)                                                                      | Build-time only, and only on Linux.                                                                                                                                                                                                 | Nothing now; upstream Tauri.                                                                                                                                                                                       |
| RUSTSEC-2025-0075/0080/0081/0098/0100 (unmaintained)                                  | `unic-*` 0.9.0           | `urlpattern` ← `tauri-utils`                                                                                              | Upstream Tauri.                                                                                                                                                                                                                     | Nothing now; track Tauri releases.                                                                                                                                                                                 |
| RUSTSEC-2024-0429 (unsound iterator)                                                  | `glib` 0.18.5            | GTK ← Tauri (Linux only)                                                                                                  | Not in the Windows build.                                                                                                                                                                                                           | Revisit when Phase 23 (Mac and Linux) starts.                                                                                                                                                                      |
| yanked                                                                                | `yoke-derive` 0.8.3      | `url` → `idna` → ICU crates                                                                                               | A yanked crate still builds from the lockfile but will not resolve fresh.                                                                                                                                                           | `cargo update -p yoke-derive` and commit the lockfile.                                                                                                                                                             |

### D-2 npm packages (`pnpm audit`)

- **plenipo-account:** no known vulnerabilities (fastify 5.12.5, @fastify/cookie 11.1.2, @fastify/formbody 9.0.0, @node-rs/argon2 2.2.1, @simplewebauthn/server 14.0.3, jose 6.2.12, marked 18.0.14, nodemailer 10.0.13, pg 8.23.1, pino 10.3.1, stripe 22.6.2 pinned exactly, @aws-sdk/client-kms 3.1144.0).
- **plenipo:** 3 high + 1 low, all in development-only packages that are never shipped to customers:
  - `extract-zip` ≤ 2.0.1 (GHSA-jmr9-qjv8-65gv, GHSA-7pqw-9j4j-h8q3; **no patched version exists**) and `basic-ftp` 5.3.1 (GHSA-c475-qrg2-pj4r, fixed in ≥ 6.2.1), both under `tests/e2e > webdriverio > @wdio/utils > @puppeteer/browsers`. `tests/e2e/package.json` has only `devDependencies`; the installer is built from `apps/desktop`. The code runs only when webdriverio has to download a browser, which does not happen on GitHub runners (they set `CHROMEWEBDRIVER`).
    Fix: in the root `package.json` add `"pnpm": { "overrides": { "basic-ftp": ">=6.2.1", "@puppeteer/browsers": "^3.2.3" } }`, run `pnpm install`, then `pnpm e2e`. `@puppeteer/browsers` 3.x no longer depends on `extract-zip`; if the override breaks the phone tests, keep only the `basic-ftp` line and record the two extract-zip advisories as accepted (`"auditConfig": { "ignoreGhsas": [...] }`) with a one-line note in `docs/development/setup.md`.
  - `esbuild` 0.27.7 (GHSA-g7r4-m6w7-qqqr, low: the dev server on Windows can read arbitrary files) under `apps/website` devDependencies, used only by `scripts/build.mjs` inside the Docker build stage. The range `^0.27.0` can never reach the fix. Fix: `apps/website/package.json:19` → `"esbuild": "^0.28.1"`, then `cd apps/website && npm install --package-lock-only` and, at the root, `pnpm install`; commit all three files.
- **Lockfile hygiene (both repos):** every entry resolves to the npm registry with an integrity hash; no `git+`, `http://`, or tarball dependencies; `--frozen-lockfile` / `npm ci` everywhere. pnpm 10 blocks dependency install scripts by default (plenipo lists none; plenipo-account allow-lists only `esbuild`).

### D-3 Secrets in the repositories

- **detect-secrets** (all files, both repos, excluding lockfiles): 252 hits in plenipo, 46 in plenipo-account. Every hit was read: test fixtures used to prove redaction works (`ghp_abcdefghijklmnopqrstuvwxyz0123456789`, `GOCSPX-ipc-test-…`, `AKIAIOSFODNN7EXAMPLE`), alphabets and constants, SHA-256 hashes of brand assets, published Noise/WebAuthn/web-push test vectors, the **public** license keys in `crates/licensing/src/trust.rs`, the contract's **published test** signing key (`contracts/license-check/v1/test-signing-key.json`, trusted only by test builds; see "What is sound"), and CI placeholders such as `POSTGRES_PASSWORD=ci-only-not-a-secret`.
- **Git history** (plenipo: all 326 commits; plenipo-account: deepened to its full 51 commits): pattern sweep for AWS keys, Stripe live keys, webhook secrets, GitHub/Slack/Google tokens, JWTs, private-key blocks, and `password=`/`secret=`/`token=` literals. Every hit is a placeholder or a refusal-test string (`rk_live_PLENIPO-TEST-…`, `sk_live_51IpcSecretKey`). No `.env`, `*.pem`, `*.key`, `*.p12`, `id_rsa`, `.netrc`, or token-bearing `.npmrc` was ever committed to either repo.
- **Personal data:** only example domains in docs and tests. The private account repo's release receipts (`docs/releases/2026-10-01-account-branding*.md`) name the AWS account ID and an EC2 instance ID. Those are not credentials and are acceptable in a private repo; do not copy those lines into the public repo.
- **Result:** no live secret, token, or private key in either repository or its history.

## 7. What is sound (checked and found good)

These are things the review looked at closely and found implemented as promised. They are listed so nobody re-does the work, and so the fixes above are read in proportion: the foundations are solid.

**Account service (plenipo-account)**

- **Settings and secrets:** every setting comes from AWS SSM with the server's own role; nothing secret is in the repo, the image, or a file on the server. A live Stripe key is refused unless the owner set `STRIPE_ALLOW_LIVE=yes`; a full `sk_` key is refused in production (restricted `rk_` keys only). The KMS signing key is checked against the public key Plenipo trusts at start-up and the service refuses to start on a mismatch (`src/config.ts`, `src/licensing/signer.ts:345-363`).
- **Headers:** CSP `default-src 'none'` with scripts and styles from `'self'` only, `frame-ancestors 'none'`, `base-uri 'none'`, HSTS two years with subdomains, nosniff, `Referrer-Policy: same-origin` (so one-time links never leak in a Referer), COOP/CORP, `Cache-Control: no-store` by default (`src/web/security.ts`).
- **Sessions and cookies:** `__Host-` cookies (Secure, HttpOnly, Path=/, no Domain), 256-bit random tokens stored only as SHA-256, 30-day maximum and 7-day idle for customers, 8 hours and 30 minutes idle for the owner, a fresh token on every sign-in, all sessions ended on reset, confirm, and email change (`src/accounts/sessions.ts`, `src/admin/auth.ts`).
- **CSRF:** every one of the 17 customer POST routes and all admin POSTs go through `csrfOk` (a keyed hash of a `__Host-` cookie plus an `Origin` check) via a scope-wide `preHandler` hook; unknown content types are refused before any handler.
- **Passwords:** Argon2id at the OWASP floor (19 MiB, 2 passes), 12 to 256 characters, Have I Been Pwned k-anonymity range check (only 5 hex characters leave the server; fails open on outage by documented design), constant-time dummy verification so sign-in timing does not reveal whether an email exists.
- **Rate limits** on sign-up, sign-in (per IP, per email, per email+IP with give-back on success), links, confirm-resend, password checks, the weekly check (per key, per IP, and a service-wide budget for unknown keys), keyed by a keyed hash so the table never holds emails or addresses; IPv6 counted per /64.
- **One-time links:** purpose-bound, single-use by an atomic `UPDATE … WHERE used_at IS NULL RETURNING`, consumed under the customer row lock; confirming an account needs the password as well as the link; unconfirmed accounts cannot sign in with a password, buy, or accept invitations and are deleted after 24 hours.
- **Open redirects:** every `next`/`_return` value goes through `localPath` (a same-site path only).
- **Stripe:** webhook signature checked over the raw bytes; one row per event ID inside the same transaction; the subscription is re-read from Stripe for every notice; customer linkage checked three ways with a unique index; Checkout and portal sessions bound to the signed-in account's own Stripe customer; prices found by lookup key under a lock and re-verified against Stripe; `client_reference_id` written only by this service.
- **License keys and answers:** Ed25519 in AWS KMS, each signature verified against the public key before use; exactly the contract's format; 130-bit key IDs; one current key per subscription by a partial unique index; the weekly check takes exactly `{key_id, app_version}` as JSON at most 1 KB, answers from a daily cache keyed by the state, and logs only the key ID.
- **SQL:** every query parameterised; only constant fragments are interpolated. **HTML:** every interpolation escaped by the `html` tag; `raw()` only on repository Markdown and icons. **Email:** sent through Microsoft Graph's JSON API (no header injection), recipients validated, link bodies wiped after sending.
- **Owner console:** Cloudflare Access JWT (RS256, issuer and audience checked against the team's published keys) on every request, fails closed when no owner is enrolled, one bound owner identity; password then passkey (WebAuthn with user verification required, per-session single-use challenges, counter checks); 80-bit single-use recovery codes and setup codes; a five-minute passkey freshness rule on every business POST; action references (UUID, advisory-locked, bound to admin + action + target + request hash); refunds limited to the exact PaymentIntent's refundable amount; catalog prices held against the attorney-approved baseline before any Stripe write; signed key text shown only on an explicit reveal with `no-store`.
- **Server and deployment:** the app container is read-only, no capabilities, `no-new-privileges`, 512 MB cap, runs as `node`; PostgreSQL on an internal network with no route out; only the app container may reach the instance metadata service (`scripts/block-imds.sh`), IMDSv2 required; no inbound ports (Cloudflare Tunnel only); nightly `pg_dump` encrypted with `age` to a public key whose private half stays offline, into a bucket with Object Lock; the checked updater deploys only an image whose receipt matches a successful `main` CI run, holds any change to start-up, config, schema, compose, Dockerfile, workflows, or scripts for operator review, runs the candidate image with no network, read-only, no capabilities to read its metadata, takes a backup first, verifies through the public route, and rolls back on failure.

**Desktop app (plenipo)**

- **Updater (ADR-038):** manifest and installer fetched only from `github.com/Seckcey/plenipo/releases` over HTTPS with Guard checking each redirect; size caps; version must be newer and not a pre-release; a minisign signature with the key built into the app, bound to the announced version, checked before anything is written; the installer is started by path with fixed flags. The Release workflow signs the installer and the app as 8 West Ventures, LLC with a timestamp and checks both inside the installer.
- **License keys:** Ed25519 `verify_strict` over a context string plus the exact key text, `deny_unknown_fields`, every field validated after the signature; the key text never appears in `Debug`, events, or logs; replayed answers never move time backwards; the test signing key is trusted only under `cfg(test)` or the `test-keys` feature, which the desktop crate enables only in `[dev-dependencies]` with `resolver = "2"`, so a release build cannot carry it; CI and the relay updater grep the binary for the test-hooks string.
- **Child processes:** every program Plenipo starts gets `env_clear()` plus a fixed baseline (`PATH`, `SystemRoot`, `TEMP`, `USERPROFILE`, …) plus the adapter's declared variables; the owner's environment, API keys, and tokens are not inherited; stdout/stderr are read with line caps; no shell is involved.
- **ADR-051 (AI tools without their own file access):** Claude Code runs with `--tools "" --strict-mcp-config` and only Plenipo's MCP server; Codex with `--sandbox read-only` and its shell and image tools off; Grok with a profile that disallows every own tool; the worker's working directory is Plenipo's own empty per-session folder, so a `.claude/settings.json`, `.mcp.json`, or `.codex/config.toml` a worker writes into the project is never read.
- **Tauri surface:** no fs, shell, http, process, opener, or dialog plugins; every command typed and granted only to the main and organisation windows; pop-out windows get no commands and `on_new_window` denies any URL other than `about:blank`; file commands take a root ID plus relative path resolved inside a known folder with `..`, `\0`, `.git`, programs/scripts (by extension), and commas refused; `open_browser` accepts only `http`/`https` without user-info and opens Plenipo's own browser over CDP; backups, restore, export, and diagnostics never take a path from the page.
- **Browser:** `--remote-debugging-pipe` (never a port), its own profile, extensions and sync off, downloads refused over CDP before any tab, `file:` navigation refused, the network gate holds non-read requests for approval.
- **OAuth connections:** fresh PKCE verifier and `state` per sign-in, a loopback listener on an OS-chosen port with a one-time start page, only the matching `state` accepted, token exchange over TLS through Guard, tokens in Credential Manager, revocation on disconnect, bearer tokens added only for the service's API hosts and dropped on redirects; no token in any log line.
- **Secrets at rest and in logs:** Windows Credential Manager for keys, tokens, and the license key; the log writer and the diagnostics zip pass every line through the Redactor (Anthropic, OpenAI, Stripe, GitHub, AWS, Google, Slack, xAI, npm key patterns, JWTs, bearer tokens, URL passwords, `KEY=value` lines) plus the owner's stored values; the paid-key helper reads its key from stdin, never the command line.
- **Front end:** zero `dangerouslySetInnerHTML`, `innerHTML`, `insertAdjacentHTML`, `document.write`, `eval`, `new Function`, `postMessage` listeners, or `fetch` in 248 non-test files; no markdown-to-HTML library; AI output, file contents, diffs, and page text render as text nodes, CodeMirror, or xterm (no web-links addon); `scripts/csp.test.mjs` fails the build on raw HTML or inline styles; the CSP allows scripts from `'self'` only.
- **Ledger:** SQL built only from compile-time column constants with bound values; data folder owner-only; restore names validated and the file checked as SQLite before use.

**Phone relay (Phase 14)**

- **The lock:** pairing with `Noise_XXpsk3_25519_AESGCM_SHA256` keyed by HKDF of the 80-bit code; every meeting after with `Noise_KK` and both static keys pre-shared plus a prologue naming both IDs; AES-GCM with strictly increasing counter nonces per direction (a copied, changed, reordered, or cross-meeting message fails); per-sign-in request-ID replay set; both implementations pinned to shared test vectors.
- **Sign-in:** a 32-byte challenge per meeting, single-use, two-minute life; full WebAuthn checks (type, exact origin, `rpIdHash`, UP and UV flags, signature with low-S normalisation, counter).
- **What a phone may ask:** one fixed enum with `deny_unknown_fields` on every wire type; Guard decides in the documented order (switched on → Pro → paired and not paused → signed in → kept-on-PC); "kept on PC" applies to Approve and Refuse, including from a notice; carry-out uses the same core functions and validators as the window.
- **Removal:** Remove closes the phone's connections, forgets its keys, and tells the relay to drop its pass for 90 days; switching phone access off or Pro ending ends every sign-in; a restart signs everyone out.
- **Relay:** PC proof is Ed25519 over a fresh nonce; the weekly answer is verified against the two production keys only; passes are signed with expiry; a PC can act only on its own phones; strict JSON with unknown fields refused; 96 KB message caps; no `unwrap` or indexing on untrusted bytes; 10-second first-message deadline; idle and ping timers; per-address, per-PC, and per-minute budgets; listens only on loopback or a private address; an off-file switch; a tight systemd sandbox; logs carry only codes and addresses.
- **Phone page:** CSP with `script-src 'self'`, `connect-src` the relay only, `frame-ancestors 'none'`, HSTS, COOP/CORP; no XSS sinks; the phone's X25519 key is non-extractable in IndexedDB; the service worker caches nothing and validates open targets.
- **Notices:** RFC 8291 `aes128gcm` matching the RFC's vector, fresh salt and key per notice, exact-host allow-list for push services checked at sign-up and at every send, no redirects followed, and only the PC's own notice words leave the PC.

**Marketing website and CI**

- Headers at origin and unchanged through Cloudflare: strict CSP, `X-Frame-Options: DENY`, nosniff, Referrer-Policy, Permissions-Policy, `server_tokens off`; no inline or third-party scripts; the redirect server sends old hosts to a hard-coded `https://getplenipo.com$request_uri`, so an attacker-controlled `Host` cannot make an open redirect; static files served with `try_files` and dotfiles refused; the container runs as UID 10001, read-only, no capabilities, loopback-only port; the release-notes renderer escapes everything and allows only http(s) links, with tests for `<script>`, `onerror`, `javascript:`, and `data:`.
- All 42 `uses:` lines across both repos' workflows are pinned to a full commit SHA; no `pull_request_target` or `workflow_run`; every PR-triggered workflow has `contents: read`; every `github.event.*` value reaches a script through `env:`; the only cross-job artifact is checksum-verified; the Release workflow never enables test keys and forces every stand-in URL empty; `release-notes.yml` can only edit titles and notes.
- `latest.json` is built only from a fixed repository constant, a validated version, and a signature that must be non-empty; the NSIS hooks run only the installed `plenipo.exe` from `$INSTDIR` with quoted paths and no downloads.

## 8. Appendix A: how to run the automated checks yourself

From a clone of each repository (Node 22, pnpm 10, Rust stable):

```sh
# Rust crate advisories (plenipo)
cargo install cargo-audit --locked
cargo audit

# npm advisories (either repo)
pnpm install --frozen-lockfile
pnpm audit

# Website lockfile (plenipo)
cd apps/website && npm audit --package-lock-only

# Secrets in the working tree (either repo)
pip install detect-secrets
detect-secrets scan --all-files --exclude-files 'node_modules/|target/|pnpm-lock.yaml|package-lock.json|Cargo.lock|\.git/|dist/'

# Secrets in history (either repo)
git log -p --all | grep -nE "AKIA[0-9A-Z]{16}|sk_live_|rk_live_|whsec_|ghp_[A-Za-z0-9]{36}|github_pat_|xox[baprs]-|BEGIN (RSA |EC |OPENSSH |)PRIVATE KEY|AIza[0-9A-Za-z_-]{35}"

# GitHub settings (needs the gh CLI signed in as the owner)
gh api repos/Seckcey/plenipo/rulesets
gh api repos/Seckcey/plenipo-account/rulesets
gh api repos/Seckcey/plenipo/private-vulnerability-reporting
gh api repos/Seckcey/plenipo/branches/main/protection
gh api repos/Seckcey/plenipo-account/branches/main/protection
```

## 9. Appendix B: what was read, and what was not

**plenipo-account** (190 files, about 19,300 lines of source, scripts, migrations, and tests): every file under `src/`, `scripts/`, `migrations/`, `public/admin.js`, `.github/`, `Dockerfile`, both compose files, `.env.example`, `.gitignore`, `.dockerignore`, `README.md`, `CLAUDE.md`, `docs/secrets.md`, `docs/deploy.md`, `docs/checked-updates.md`, `docs/owner-console*.md`, `docs/admin-stripe-permissions.md`, the license-check contract, and the admin, invitation, accounts, billing, and email tests (titles of every test; bodies of the gate, freshness, sign-in, reveal, customer-review, buy, and switch tests). Not read line by line: the bodies of `test/admin-billing-display`, `admin-billing-eligibility`, `admin-billing-recovery`, `admin-catalog-recovery`, and parts of the remaining admin test files; `legal/*.md`; the monetization research notes.

**plenipo** (about 2,370 tracked files; about 214,000 lines of Rust and TypeScript): read in full or in their security-relevant parts: `SECURITY.md`, `CLAUDE.md`, all workflows and `dependabot.yml`, `tauri.conf.json`, the capability files, `build.rs`, the NSIS hooks, every deploy folder (`crates/relay/deploy`, `apps/remote/deploy`, `apps/website/deploy`), the whole of `crates/relay`, `crates/relay-contract`, `crates/remote` (except the tail of `contract.rs`), `apps/remote` (non-test files), `contracts/phone-relay`, `crates/licensing` (`trust.rs`, `key.rs`, `codec.rs`, `answer.rs`, `license.rs`, `state.rs`), `crates/capabilities/src/updates.rs`, `ai_tools.rs`, `license_check.rs`, `ssh.rs`, `vault.rs`, `programs.rs`, the file-command and owner-file paths in `broker/owner_files.rs`, `browser/mod.rs` launch and gate parts, `connections/signin.rs`, `http.rs`, the OAuth parts of `connections/mod.rs`, `paid/mod.rs`, `guard/src/outbound.rs`, `websites.rs`, `ai_tools.rs`, `redact.rs`, `remote.rs`, `defaults.rs`, `commands.rs`, `crates/runtime/src/policy.rs`, `supervisor.rs`, and each agent adapter's launch code; `apps/desktop/src-tauri/src/lib.rs` (non-test part), `update_host.rs`, `upkeep_commands.rs`, `files_commands.rs`, `license_host.rs`, `remote_host.rs`, `workspace_windows.rs`, `diagnostics.rs`, `logs.rs`, and the relevant parts of `commands.rs`; `apps/website` entirely; `scripts/*.mjs`; `scripts/windows/installer-tests.ps1`; the whole desktop and UI front end by sink grep (248 files) plus targeted reads; ADR-022, ADR-034, ADR-038, ADR-048, ADR-051, ADR-052, ADR-091, ADR-140 to ADR-149, and the Phase 14 documents.

Not reached, or grep-only (no findings are claimed from them): the browser's `cdp.rs`, `classify.rs`, `page.js`, and most of `tab.rs` (the overlay sign and the control re-check were not reviewed); the per-service connection modules (`google.rs`, `microsoft365.rs`, `slack.rs`, `hubspot.rs`, `stripe.rs`, `keyed.rs`, `text.rs`, most of `wordpress.rs`); most of `paid/helper.rs`; `runtime/agent/{acp,antigravity,direct,paid,brief,preview,memory_store,dto}.rs` and `ollama/`; the Ledger crate beyond its SQL grep, backups, and export; `licensing/{entitlements,words}.rs` bodies; `guard/src/{dto,connections,paid,registry,service,add_ons}.rs` bodies; `capabilities/src/{watch (body),screens,server,control,github,mcp}.rs`, `broker/{connecting (body),live,watching}.rs`; the desktop `recovery.rs`, `tray.rs`, `notices.rs`, `window_watch.rs`, `tool_holds.rs`, `indicator.rs`, `org_host.rs`, `org_commands.rs`, `owner_commands.rs`, `canvas_commands.rs`, `spending_commands.rs`, `workspace_commands.rs`, `ledger_host.rs`, `backup_host.rs`, `agent_host.rs`, `runtime_host.rs`; `crates/core`, `crates/router`, `crates/workforce`, `crates/liaison` (grepped only for `Command::new`, file writes, dynamic SQL, and `unwrap` on untrusted input); the React component tree line by line; the e2e specs; most of `docs/`. A second pass over these areas is listed under "What next".
