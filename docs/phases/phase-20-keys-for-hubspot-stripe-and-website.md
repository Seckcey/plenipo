# Making keys for HubSpot, Stripe, and your website

Written 2026-09-30 (Pacific time) for Phase 20, part 20C
([ADR-071 (HubSpot, Stripe, the website, and add-on tools: the owner's choices)](../adr/ADR-071-keys-website-and-add-on-choices.md)).
You do these steps yourself, in your own browser. **Nothing here goes into a chat.**

- Each key goes **only** into its card in Plenipo: **Settings → Connections →** HubSpot, Stripe,
  or WordPress and WooCommerce. The box hides what you type. Plenipo checks the key once, keeps it
  only in the Vault (Windows Credential Manager), and never shows it again.
- Never paste a key into a chat, an email, a document, or GitHub.
- **Disconnect** removes the key from the Vault. HubSpot and Stripe cannot cancel a key from
  outside, so delete it in the service too (the card says where). Your website's Application
  Password is revoked at your site by Plenipo.

HubSpot, Stripe, and WordPress rename buttons often. Where a button has a newer and an older name,
both are given. Sources are at the end.

## Which steps you need

| You want workers to…                                                        | Do                   |
| --------------------------------------------------------------------------- | -------------------- |
| Read and update HubSpot contacts, companies, and deals                      | Part A (HubSpot)     |
| Read Stripe payments, customers, and invoices; draft invoices; refunds      | Part B (Stripe)      |
| Read and draft posts and pages on your WordPress site; read store orders    | Part C (WordPress)   |
| Keep the store read-only at WooCommerce itself, or use a separate store key | Part D (WooCommerce) |

**Before any of them:** in Plenipo, turn on the parts you want on the card first (each **Off**,
**Read only**, or **Full access**). HubSpot's and Stripe's cards then list the exact permissions
the key needs, under **Its key**. Give the key only those.

## Part A — HubSpot: a service key (about 5 minutes)

HubSpot's **service keys** are a **public beta** (HubSpot may change them). A key from a legacy
**private app** you already have works the same way. HubSpot stops new private apps on
2026-10-26, so make a service key.

**Before you start:** sign in to HubSpot as a **super admin**, or a user with developer tools
access. You can give a key only permissions you have yourself.

1. In HubSpot, open **Development → Keys → Service keys** (also under **Settings → Integrations →
   Service Keys**).
2. Click **Create service key**. Name it **Plenipo**.
3. Under scopes, tick **only** what Plenipo's HubSpot card lists. For each part:

   | Part in Plenipo | Read only                                                             | Full access adds                                                        |
   | --------------- | --------------------------------------------------------------------- | ----------------------------------------------------------------------- |
   | Contacts        | `crm.objects.contacts.read`                                           | `crm.objects.contacts.write`                                            |
   | Companies       | `crm.objects.companies.read`, and `crm.objects.contacts.read` (notes) | `crm.objects.companies.write`, and `crm.objects.contacts.write` (notes) |
   | Deals           | `crm.objects.deals.read`, and `crm.objects.contacts.read` (notes)     | `crm.objects.deals.write`, and `crm.objects.contacts.write` (notes)     |

   HubSpot has no notes permission of its own: reading any record's notes needs
   `crm.objects.contacts.read`, and adding a note needs `crm.objects.contacts.write`.

4. Click **Create**. Copy the key (it starts `pat-`).
5. In Plenipo: **Settings → Connections → HubSpot → Service key**. Paste it and click **Save and
   check**. The card says **Connected to HubSpot** (with your account's number, when the key
   may read it).
6. **Every six months** (HubSpot's advice): in HubSpot, **Rotate and expire later** on the key
   (the old one keeps working 7 days). On the card, press **Disconnect**, then paste the new key
   and **Save and check**: the card's parts and lists stay. (Plenipo cannot tell by itself that
   a new key is for the same HubSpot account, so it does not swap keys while connected.)

**When you disconnect:** delete the key in HubSpot too (**Development → Keys → Service keys →**
the key **→ Delete**).

## Part B — Stripe: a restricted key tagged for an agent (about 10 minutes)

**Start in test mode.** A test-mode key (it starts `rk_test_`) moves no real money. Try
everything there first. A live key (`rk_live_`) moves real money.

Plenipo takes **only a restricted key**. It refuses your full secret key (`sk_…`) and a
publishable key (`pk_…`).

1. Sign in to the **Stripe Dashboard**. Turn on **Test mode** (or open a **sandbox**), at the top.
2. Open **Developers → API keys**, and click **Create restricted key**.
3. When Stripe asks how you will use the key, choose **Authorizing agent access to your account**
   (this tags it for an agent). With that tag, Stripe also holds each refund for your approval in
   its Dashboard, after Plenipo's card, for up to 14 days.
4. Name it **Plenipo**.
5. Set **every** permission to **None**, then set only what Plenipo's Stripe card lists:

   | Part in Plenipo | Read only                                            | Full access adds                                                                                              |
   | --------------- | ---------------------------------------------------- | ------------------------------------------------------------------------------------------------------------- |
   | Payments        | Balance: Read · PaymentIntents: Read · Payouts: Read | Charges and Refunds: Write (refunds; each always asks you), and Customers: Read (the card names the customer) |
   | Customers       | Customers: Read                                      | (nothing: customers only read)                                                                                |
   | Invoices        | Invoices: Read · Subscriptions: Read                 | Invoices: Write (drafts; finalizing and sending always ask you), and Customers: Read                          |

6. Click **Create key**. Stripe shows it **once**: copy it now.
7. In Plenipo: **Settings → Connections → Stripe → Restricted key**. Paste it and click **Save and
   check**. The card says your business's name and **Test mode**.
8. **Going live, later:** switch the Dashboard to live mode and make a second key the same way
   (steps 2–6). On the card, **Disconnect**, then paste the live key. The card then shows **Live
   mode: this key moves real money**.

What Plenipo never does with Stripe: charge a card, send a payout, or make a payment link. Every
refund, and every invoice finalized and sent, waits for you on an approval card that shows the
amount, the currency, the customer, and test or live mode.

**When you disconnect:** delete the key in Stripe too (**Developers → API keys →** the key **→
Delete**, or **Roll key**).

## Part C — Your WordPress site: an Application Password (about 10 minutes)

An Application Password can do **everything its WordPress user can**. So first make a WordPress
user **just for Plenipo**, with the smallest role that works:

- **Editor**: reads, drafts, and publishes posts and pages, and reads comments. No store.
- **Shop Manager** (with WooCommerce): all that, and the store's orders, notes, and refunds.
  **Choose this if you want the store at all**, with or without a WooCommerce key (Part D).
- Never **Administrator**.

Your site must use **https**.

1. In your WordPress admin, open **Users → Add New User** (older: **Users → Add New**).
2. **Username:** `plenipo`. **Email:** an address you control (for example
   `plenipo@yourdomain.com`). **Role:** **Editor**, or **Shop Manager** for the store. Click
   **Add New User**.
3. Open **Users → All Users →** `plenipo` **→ Edit**. Scroll to **Application Passwords**.
4. **New Application Password Name:** **Plenipo**. Click **Add New Application Password**.
5. WordPress shows the password **once** (24 letters and digits, in groups of four). Copy it now.
6. In Plenipo: **Settings → Connections → WordPress and WooCommerce**:
   - **Your site's address:** as your browser shows it once the site has loaded, with `https://`
     (for example `https://www.example.com`, or `https://example.com/shop` if WordPress is in a
     folder).
   - **WordPress user name:** `plenipo`.
   - **Application Password:** paste it (spaces or not).
7. Click **Save and check**. The card says **Connected to** the user's name **at** your site, and
   its WordPress role.

**If it does not connect:**

- "Your site sent Plenipo to another address": your site sends visitors elsewhere (often from
  `example.com` to `www.example.com`). Open your site in your browser, wait until it loads, and
  type the address your browser then shows.
- "did not accept that user name and Application Password": check both. Some security plugins, and
  some hosts, turn Application Passwords off or strip the sign-in from requests. Allow them for the
  `plenipo` user, or ask your host.

**When you disconnect:** Plenipo revokes the Application Password at your site. You can check it
is gone under **Users →** `plenipo` **→ Application Passwords**.

## Part D — Optional: a WooCommerce key (about 5 minutes)

The store needs a **Shop Manager** either way:

- **Without a key**, the store's tools use the Application Password, so `plenipo` must be a Shop
  Manager.
- **With a key**, WooCommerce lets the key do **only what its WordPress user may**. A key made for
  an Editor is accepted but cannot see a single order. So make the key for a Shop Manager.

With a **Read** key, the store stays read-only at WooCommerce itself, whatever Plenipo's parts say.

1. If `plenipo` is an **Editor**, make it a Shop Manager first: **Users →** `plenipo` **→ Edit →
   Role: Shop Manager → Update User**.
2. In your WordPress admin, open **WooCommerce → Settings → Advanced → REST API**, and click **Add
   key** (or **Create an API key**).
3. **Description:** **Plenipo**. **User:** `plenipo`. **Permissions:** **Read** (read-only), or
   **Read/Write** (to let workers add notes, change an order's status, or refund, each as the card
   allows). Never **Write** alone: it cannot read.
4. Click **Generate API key**. Copy the **Consumer key** (`ck_…`) and the **Consumer secret**
   (`cs_…`). The secret is shown **once**.
5. In Plenipo, on the **WordPress and WooCommerce** card:
   - **Site already connected:** in **Add a WooCommerce key**, paste both and click **Add a
     WooCommerce key**. Your Application Password is not needed again.
   - **Not connected yet:** open **WooCommerce key (optional)** under the password, paste both, and
     click **Save and check**.

**If the key is refused**, the card says why:

- "the WordPress user it belongs to may not see the store's orders": make that user a Shop Manager
  (step 1), or make the key for a Shop Manager, then try again.
- "WooCommerce knows that key, but not with that secret": copy the secret again. If it is lost,
  revoke the key and make a new one (the secret is shown once).
- "WooCommerce does not know that key": check you copied the whole key, and that it is still
  listed under **REST API** (not revoked).
- "It is Write only": edit the key's **Permissions** to **Read** or **Read/Write**.
- "WooCommerce did not see the key", or "refused the request before WooCommerce answered": a host or
  security plugin is in the way. Ask your host to let the `Authorization` header through to
  WordPress.

**To replace the key later:** use **Replace the WooCommerce key** on the card, the same way.

**When you disconnect:** Plenipo removes the key from the Vault. Revoke it in WooCommerce too
(**WooCommerce → Settings → Advanced → REST API →** the key **→ Revoke**).

## Safe to share, and secret

| Value                                            | Safe to share? | Where it goes               |
| ------------------------------------------------ | -------------- | --------------------------- |
| HubSpot service key (`pat-…`)                    | **Secret**     | The HubSpot card only       |
| Stripe restricted key (`rk_test_…`, `rk_live_…`) | **Secret**     | The Stripe card only        |
| Stripe secret key (`sk_…`)                       | **Secret**     | Nowhere: Plenipo refuses it |
| Your site's address                              | Yes            | The website card            |
| WordPress user name                              | Yes            | The website card            |
| Application Password                             | **Secret**     | The website card only       |
| WooCommerce consumer key and secret              | **Secret**     | The website card only       |

## Risks to know about

- **HubSpot's service keys are a beta.** HubSpot may change how they work. If the card says
  **Needs a new key**, make a new one (Part A).
- **Stripe live keys move real money.** Plenipo always asks before any refund or invoice, and an
  agent-tagged key makes Stripe ask again. Stripe's terms say what an AI agent does through your
  account binds you.
- **An Application Password has its user's full permissions.** The WordPress user's role is the
  real limit: keep it Editor or Shop Manager, never Administrator.
- **Client data goes to the AI company.** What a worker reads (a contact, an order, a payment)
  becomes part of its conversation with the AI tool. Check each AI tool's plan does not train on
  your data, and your agreements with clients.
- **Selling Plenipo Pro with these connections** may need its terms reviewed by a lawyer.

## Sources (checked 2026-09-30)

HubSpot: [service keys](https://developers.hubspot.com/changelog/service-keys) ·
[legacy private apps ending](https://developers.hubspot.com/changelog/legacy-private-app-creation-sunset) ·
[scopes](https://developers.hubspot.com/docs/apps/developer-platform/build-apps/authentication/scopes)

Stripe: [API keys](https://docs.stripe.com/keys) ·
[restricted keys](https://docs.stripe.com/keys/restricted-api-keys) ·
[idempotent requests](https://docs.stripe.com/api/idempotent_requests) ·
[Services Agreement](https://stripe.com/legal/ssa)

WordPress and WooCommerce:
[Application Passwords](https://make.wordpress.org/core/2020/11/05/application-passwords-integration-guide/) ·
[roles and capabilities](https://wordpress.org/documentation/article/roles-and-capabilities/) ·
[WooCommerce REST API](https://woocommerce.com/document/woocommerce-rest-api/)
