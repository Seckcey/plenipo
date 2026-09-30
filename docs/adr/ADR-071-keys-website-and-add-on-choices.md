# ADR-071: HubSpot, Stripe, the website, and add-on tools — the owner's choices, and what the services' pages changed

- **Status:** Accepted (by the owner, 2026-09-30: every choice below as recommended — "Keys;
  Stripe agent key", "Refuse them", "Nobody; Read only", "Only the saved address", and "Money goes
  back")
- **Date:** 2026-09-30
- **Phase:** 20, part 20C (ADR-067, Phase 20 in three parts)
- **Amends:** [ADR-062 (one set of rules for every connection)](ADR-062-connections-one-set-of-rules.md)
  — publishing on a website, and money, never go on a **Send without asking to** list;
  [ADR-063 (signing in, and the Vault)](ADR-063-signing-in-to-a-connection.md) §7 — a key typed
  into a card is checked once, then kept only in the Vault;
  [ADR-064 (how each connection is built)](ADR-064-how-each-connection-is-built.md) §5–§7 —
  HubSpot's new web addresses, its notes' permission, Stripe's agent keys and approvals,
  WooCommerce refunds, and the website's address;
  [ADR-066 (add-on tools you set up)](ADR-066-add-on-tools.md) §1 — the programs refused, and who
  may use an add-on at the start

> **On screen** (ADR-010, plain words and rank names): **Key** (a box that hides what you type),
> **Save and check**, **Replace the key**, **Needs a new key**, **Test mode** / **Live mode: moves
> real money**, **Your site's address**, **Application Password**, **WooCommerce key**, **Posts
> and pages**, **Store**, **Add-on tools**, **Add a program**, **Look at its tools**, **Off /
> Reading / Changing**. This record keeps the code's words (API key, scope, REST, MCP,
> idempotency, redirect).

## In short

Before building part 20C, the approved design was checked against what parts 20A and 20B built and
against HubSpot's, Stripe's, WordPress's, and WooCommerce's own pages (2026-09-30). The owner
answered five questions, each as recommended:

1. **Keys typed into Settings** (choice 11): a HubSpot **service key** with only contacts,
   companies, and deals; a Stripe **restricted key tagged for an agent**, test mode first; a
   WordPress **Application Password** for a user made for Plenipo, and an optional **WooCommerce
   key**. Each goes only to the Vault.
2. **Add-on programs that download code each time they start are refused** (choice 12): `npx`,
   `uvx`, `bunx`, `pnpm dlx`, and the like. The owner installs a program first, then adds the
   installed copy.
3. **Nobody may use a new connection or add-on until the owner picks; each line starts at Read
   only** (choice 13), as Microsoft 365, Slack, and Google already work.
4. **The website connection reaches only the address saved on its card**: `https`, the usual
   port, a real domain name, that exact host. The password goes only there.
5. **An approved WooCommerce refund sends the money back** through the store's payment company
   (WooCommerce's own default), and Plenipo says so on the card.

Accepting this record means building part 20C with these choices and the fits in section 6.

## Context

The approved design (Phase 20 checklist, design §7–§8 and §11; ADR-064 §5–§7; ADR-066) was
compared with what parts 20A and 20B built:

- Guard's gate for Plenipo's own requests knows only **fixed hosts** per service
  (`outbound.rs`, `connection_hosts`). The owner's website is the one connection whose address is
  not fixed.
- Every connection so far **signs in in the owner's browser**, and "Reconnect to allow …" follows
  what the service granted at sign-in (`allowed_level`). A key has its permissions chosen when the
  owner creates it, in the service.
- Connection tools come from **fixed tables** found by name (`tools::find`). Add-on tools are named
  from the owner's programs, so they need their own path.
- `mcp.invoke` ("Use add-on tools") is in Guard's registry with no tools. Connections grant their
  permissions through their own **Who may use it** list, not a role's set (ADR-062 §3, one set of rules for every connection); ADR-066 §3 (add-on tools)
  gives each add-on the same list.
- The supervisor already runs a program Plenipo talks to over its standard input and output (the
  AI tools that speak ACP): an add-on program can be run the same way, as an approved program.

The services' own pages, read on 2026-09-30:

- **HubSpot.** Service keys are a **public beta** (since 2026-02-10), made at **Development → Keys
  → Service keys** (also **Settings → Integrations → Service Keys**) by a super admin or a user
  with developer tools access, who can give a key only permissions they have themselves. The key
  is sent as `Authorization: Bearer …`; HubSpot advises replacing it every six months ("Rotate and
  expire later" keeps the old one 7 days). There is no way to cancel a key from outside HubSpot.
  Since 2026-03-30 HubSpot's web interface uses **dated addresses** (`/crm/objects/2026-09/…`);
  the old `/crm/v3/…` ones stop being supported in September 2027. **Adding a note needs
  `crm.objects.contacts.write`**, even for a note on a company or a deal (there is no notes
  permission); a note is linked to a contact, company, or deal by association types 202, 190, and 214. Legacy private apps: none new from 2026-09-28 (new accounts) and 2026-10-26 (all); existing
  ones keep working until September 2027.
- **Stripe.** A restricted key starts `rk_test_` or `rk_live_`, is made at **Developers → API
  keys → Create restricted key** with **None**, **Read**, or **Write** per resource, and is shown
  once. **"Authorizing agent access to your account"** at creation tags it for an agent: it
  "works the same as other restricted API keys", and Stripe's approval rules apply to it — by
  default a **refund** (and a subscription cancelled) comes back `approval_required` and waits for
  the owner in Stripe's Dashboard, for up to 14 days. A missing permission is answered 403.
  `Idempotency-Key` (up to 255 characters, kept at least 24 hours) makes a repeated request return
  the first answer. Refunds need **Charges and Refunds: Write**. Every object says `livemode`.
  From 2026-10-31 Stripe's own MCP server takes only its sign-in or agent-tagged keys (Plenipo
  uses Stripe's regular web interface).
- **WordPress.** An Application Password (24 letters and digits, with or without spaces) is made
  at **Users → Profile → Application Passwords**, works only over `https` (or a site marked
  "local"), and acts with its user's **full** permissions; each can be revoked, and the REST
  interface can tell Plenipo which one it is using (`/wp/v2/users/me/application-passwords/introspect`).
  Security plugins or hosts may turn them off or strip the sign-in header. **Editor** is the
  smallest role that can publish both posts and pages.
- **WooCommerce.** A REST key (`ck_…`, `cs_…`) is made at **WooCommerce → Settings → Advanced →
  REST API → Add key** with **Read**, **Write**, or **Read/Write** (Write alone cannot read), and
  works only on the store's addresses (`/wc/v3/…`). WooCommerce also accepts the Application
  Password of a user who may manage the store (**Shop Manager**). A refund's `api_refund` is
  **true by default**: the payment company sends the money back. A customer note
  (`customer_note: true`) emails the customer.

## Decision

### 1. Keys, typed into each card (the owner's answer 1)

- **The card** for HubSpot, Stripe, and the website has boxes that hide what is typed, and **Save
  and check**. Plenipo makes **one reading call** with the key, through Guard's gate; only a key
  the service accepts is kept, in the Vault, read back to check it, and never shown or returned
  again. Nothing about it is recorded but that it was kept. **Replace the key** keeps the card
  connected when the new key is for the same account (HubSpot's six-month replacement);
  anything else needs **Disconnect** first.
- **HubSpot:** a service key, or the key of a legacy private app the owner already has (both are
  sent the same way). The checking call reads one contact; a key HubSpot refuses (401) is not kept,
  and a key without permission to read contacts (403) is kept with that said on the card.
- **Stripe:** only a **restricted** key (`rk_test_…` or `rk_live_…`). A full secret key
  (`sk_…`) or a publishable key (`pk_…`) is refused with the steps to make a restricted one. The
  card says **Test mode** or **Live mode: moves real money**, from the key and from Stripe's own
  answer. The steps say to tag the key for an agent.
- **The website:** its address, the WordPress user's name, and its Application Password; and,
  optionally, a WooCommerce key and its secret. The checking call reads who the password belongs
  to (`/wp/v2/users/me`), and the store's first order with the key the store's tools will use.
  When a WooCommerce key is kept, the store's tools use it (so a **Read** key keeps the store
  read-only at WooCommerce too); otherwise they use the Application Password.
- **Disconnect** erases every value the card kept from the Vault. The website's Application
  Password is also revoked at the site, where WordPress allows it. HubSpot and Stripe have no way
  to cancel a key from outside, so the card says where to delete it.
- **Needs a new key:** a key the service stops accepting (401) is erased and the card asks for a
  new one; its tools stop.

### 2. Downloading programs refused (the owner's answer 2)

An add-on's program is refused when it is a shell (`cmd`, `powershell`, `pwsh`, `bash`, `sh`,
`zsh`, `fish`, `wsl`, Windows script hosts) or a code downloader (`npx`, `pnpx`, `bunx`, `uvx`,
or `npm exec`, `npm x`, `pnpm dlx`, `yarn dlx`, `bun x`, `pipx run`, `uv tool run`). The refusal
says how to install the program first.

### 3. Who may use it, to start (the owner's answer 3)

Every new connection and add-on starts with an empty **Who may use it** list, and a role or agent
added to it starts at **Read only**. For an add-on, **Read only** is its **Reading** tools, and
**Read and write** adds its **Changing** tools, which ask every time.

### 4. The website's address (the owner's answer 4)

- **Kept on the card, not in the Vault** (it is not a secret): `https://` and a domain name with
  at least one dot, in small letters, and optionally the folder WordPress is installed in
  (`https://example.com/shop`). Refused: `http`, another port, a user name or password in the
  address, an IP address, `localhost`, and names that only work on a local network (`.local`,
  `.lan`, `.internal`, `.home.arpa`).
- **Guard's gate** lets the website connection reach **only that host**, over `https`, on the
  usual port, each redirect checked again. A redirect to any other host — even `www.` — is
  refused, and the card says which address to type instead. The Application Password and the
  WooCommerce key are added only for that host.
- **Changing the address needs Disconnect first**, so a password is never sent to a new address.

### 5. WooCommerce refunds send the money back (the owner's answer 5)

A refund is sent with `api_refund: true`, always stated, never left to the default. The approval
card says "WooCommerce asks <payment method> to send the money back to the customer". If the
payment company cannot refund by itself, WooCommerce refuses, and Plenipo says so and does nothing
else (never a refund that only marks the order).

### 6. What the design had to fit

1. **HubSpot's dated addresses:** Plenipo calls `https://api.hubapi.com/crm/objects/2026-09/…`,
   never `/crm/v3/…`.
2. **HubSpot's parts** are **Contacts**, **Companies**, and **Deals**, each **Off**, **Read
   only**, or **Full access**. Each has four tools — search, read (with its latest notes), create
   or change, and add a note — so a part that is off offers none of its tools. The key-making
   steps name `crm.objects.contacts.write` for notes on any record.
3. **Stripe's parts** are **Payments** (the balance, payments, and payouts; Full access adds
   **refunds**), **Customers** (Read only: Stripe's customers have no tool that changes them in
   this phase), and **Invoices** (invoices and subscriptions; Full access adds **drafting** an
   invoice and **finalizing and sending** one). Refunds and finalizing and sending are **Pay**:
   they always ask, whatever the switches and lists say.
4. **Stripe's approval rules:** a refund Stripe holds for the owner's approval (`approval_required`)
   is reported as "waiting for your approval in Stripe's Dashboard", recorded as such, and never
   sent again by Plenipo.
5. **Money is checked again, and never paid twice:** the card shows the amount, the currency, the
   customer, and test or live mode. Just before acting, Plenipo reads the payment or invoice again
   and stops if the amount, the currency, the customer, or the mode changed, or if a refund would
   be more than what is left. Every Stripe request that changes something carries an
   `Idempotency-Key` made when the call was planned, so a retry returns the first answer. A
   WooCommerce refund is never retried by Plenipo; if the answer is lost, Plenipo says to check the
   order before trying again. Plenipo pins Stripe's web interface version (`Stripe-Version`).
6. **The website's parts** are **Posts and pages** (read posts, pages, and their comments; Full
   access adds drafts, and publishing or changing what is published) and **Store** (orders, their
   notes, products, and customers; Full access adds private order notes, an order's status, notes
   the customer sees, and refunds).
7. **Who a send reaches** (ADR-062 §5): an order's status change and a customer note reach **the
   order's customer**, by the email address on the order, so the owner's list and switch can let
   them go ahead without asking. **Publishing, or changing what is published, reaches everyone who
   visits the site**: nothing on a list can stand for that, so it always asks. Money is never on a
   list.
8. **A key has no "Reconnect":** a part turned on or up for HubSpot, Stripe, or the website works
   at once, as far as the key allows; a call the key is not allowed makes the service refuse, and
   Plenipo says which permission to add in the service.
9. **Add-on tools** are offered by name (`addon_<add-on>_<tool>`, up to 50 characters, shortened
   with a check code when a program's name is longer), listed only while the add-on is on, the tool
   is **Reading** or **Changing**, and the worker's line allows it. `mcp.invoke` gets its tools and
   is granted through each add-on's own list, as `connections.read` and `connections.write` are
   (the registry stays at 18 permissions). An add-on's program starts on first use in a worker's
   step, is stopped when the step's tools close, and speaks MCP over its standard input and output
   (`initialize`, `tools/list`, `tools/call`), with a 30-second start and a 10-minute call. A tool
   whose description or input changed since it was marked is not called, and goes back to **Off**.
10. **New desktop commands** (the main window's alone): `save_connection_key`, `add_add_on`,
    `change_add_on`, `remove_add_on`, `check_add_on_tools`, and `set_add_on_tools`.

## Consequences

- The owner creates three kinds of keys, following click-by-click steps. Each key's own
  permissions, chosen in the service, are a second limit under Plenipo's parts.
- With an agent-tagged Stripe key, a refund needs two approvals: Plenipo's card, then Stripe's
  Dashboard. That is slower, and on purpose.
- A website that redirects between `example.com` and `www.example.com` must be saved under the
  address it ends at.
- An Application Password acts with its user's full permissions: the WordPress user's role is the
  real limit. The steps make a user just for Plenipo (**Editor**, or **Shop Manager** with a store).
- A money action can never go ahead without the owner, and a lost answer on a WooCommerce refund
  needs the owner to look at the order before trying again.
- HubSpot's service keys are a beta: HubSpot may change them. The stand-in tests hold the shape
  Plenipo expects, and the owner's first real try is listed in the acceptance report.

## Alternatives considered

- **The services' own hosted servers and their sign-ins** (choice 11's other). Not chosen.
- **A Stripe key not tagged for an agent.** Not chosen: Stripe's own approval for refunds is a
  second lock on money.
- **Allow downloading programs, with a warning** (choice 12's other). Not chosen.
- **New lines start at Read and write** (choice 13's other). Not chosen.
- **The website's `www.` twin, or local-network sites and other ports.** Not chosen: the password
  goes to one exact host.
- **A refund that only marks the order.** Not chosen: the customer would not get the money until
  the owner refunded again elsewhere.
- **One Application Password for everything, no WooCommerce key.** Kept as the default when no
  key is given; the optional WooCommerce key stays, because a **Read** key limits the store at
  WooCommerce itself.

## As built (v1.14.2)

Built as decided. The review before merging (acceptance report, section 6) changed these
details:

- **§1, a key without permission to read contacts:** HubSpot's key is kept, and the card lists
  only what was checked; a record's notes then say they need `crm.objects.contacts.read`
  (reading any record's notes needs it, so Companies and Deals at Read only ask for it too).
  Stripe's mode comes from the key (`rk_test_` or `rk_live_`), checked against Stripe's own
  answer when the key may read the balance.
- **§1, Replace the key:** only when Plenipo can tell it is the same account (HubSpot's number,
  Stripe's account, the website's user). A key that may not say which account it is needs
  **Disconnect** first; the card's parts and lists stay. A key the service stopped taking is
  replaced unless it is clearly another account. Replacing the website's password keeps (and
  checks) a WooCommerce key already kept.
- **§4, the website's address:** the domain in small letters, the folder as typed (a server may
  tell `/Blog` from `/blog`). The password is kept only for the address it was checked at, and
  Disconnect revokes it there.
- **§5, money:** a change whose answer is lost, or that the service fails with an error of its
  own (5xx), says "Maybe done: check before asking again", never "Not done". Stripe's changes are
  sent once more with the same idempotency key; no other service's change is sent twice, or
  followed to another page. An invoice paid, voided, or written off meanwhile is not sent. The
  refund card leaves out the payment's own description (the payment's words). Store refunds are
  only in currencies written with cents.
- **§6.7, who a send reaches:** an unpaid card order's status change (to processing, completed,
  or cancelled) may take or release the money held on the card: it is money, and always asks.
  The publish and change cards list what a post's markup holds that its words do not show
  (links' addresses, scripts, frames, forms, code run on a click).
- **§6.9, add-on tools:** workers get a tool's input shape without the program's words; a
  program's error words are fenced and never recorded; tools with unusual names or oversize
  inputs are left out; a running program whose program, arguments, or secrets changed is started
  afresh, in a private folder removed when it stops; more programs are refused (switches before a
  downloading subcommand, `env`, `busybox`, `conhost`, `node -e`, `python -c`, `deno npm:`,
  `go run …@…`, `dnx`).
