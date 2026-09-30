import type {
  AddOn,
  ConnectionCard,
  ConnectionsPage,
  Part,
  PartCard,
  PartLevel,
  Service,
  ServiceCard,
} from "@plenipo/types";

/** Each part's name, what it reads, what Full access adds, and whether it has Full access. */
const PART_WORDS: Record<Part, [string, string, string]> = {
  mail: [
    "Mail",
    "Search and read your mail.",
    "Save drafts. Sending one asks you, unless everyone is on your Send without asking to list.",
  ],
  calendar: ["Calendar", "Read your calendar.", "Add events; inviting people asks you."],
  onedrive: [
    "OneDrive",
    "Find and read files in your OneDrive.",
    "Add new files; replacing a file asks you.",
  ],
  sharepoint: [
    "SharePoint",
    "Find and read files in SharePoint sites you can see.",
    "Add new files to sites you can edit; replacing a file asks you.",
  ],
  teams: [
    "Teams",
    "Read your chats and your teams' channels.",
    "Send chat messages, start chats, and post in channels; each asks you.",
  ],
  channels: [
    "Channels",
    "List and read the channels you are in, and their threads.",
    "Post and reply in threads, as you. Asks you, unless the channel is on your Send without asking to list.",
  ],
  directMessages: [
    "Direct messages",
    "Read your direct messages and group messages.",
    "Send in them, as you. Asks you, unless everyone in it is on your list.",
  ],
  search: ["Search", "Search messages, only in the parts that are on.", "Search only reads."],
  gmail: [
    "Gmail",
    "Search and read your mail.",
    "Save drafts. Sending one asks you, unless everyone is on your Send without asking to list.",
  ],
  drive: [
    "Drive",
    "Find and read your files: text files, Google Docs, and Word documents.",
    "Add new text files.",
  ],
  contacts: [
    "Contacts",
    "Search and read contacts, with their latest notes.",
    "Create or change a contact, and add a note to one.",
  ],
  companies: [
    "Companies",
    "Search and read companies, with their latest notes.",
    "Create or change a company, and add a note to one.",
  ],
  deals: [
    "Deals",
    "Search and read deals, with their latest notes.",
    "Create or change a deal, and add a note to one.",
  ],
  payments: [
    "Payments",
    "Read the balance, payments, and payouts.",
    "Refund a payment — always asks you, and Stripe asks again for an agent key.",
  ],
  customers: ["Customers", "Read customers.", "Customers only read."],
  invoices: [
    "Invoices",
    "Read invoices and subscriptions.",
    "Draft an invoice (it is not sent). Finalizing and sending one always asks you.",
  ],
  posts: [
    "Posts and pages",
    "Read posts and pages, and their comments.",
    "Write drafts. Publishing, and changing anything already published, always ask you.",
  ],
  store: [
    "Store",
    "Read orders and their notes, products, and customers.",
    "Add private order notes. An order's status and notes the customer sees ask you, unless the customer is on your list. Refunds always ask you.",
  ],
};

type Built = "microsoft365" | "slack" | "google" | "hubspot" | "stripe" | "wordpress";

const PARTS: Record<Built, Part[]> = {
  microsoft365: ["mail", "calendar", "onedrive", "sharepoint", "teams"],
  slack: ["channels", "directMessages", "search"],
  google: ["gmail", "calendar", "drive"],
  hubspot: ["contacts", "companies", "deals"],
  stripe: ["payments", "customers", "invoices"],
  wordpress: ["posts", "store"],
};

const STARTING: Record<Built, Partial<Record<Part, PartLevel>>> = {
  microsoft365: { mail: "readOnly", calendar: "readOnly" },
  slack: { channels: "readOnly" },
  google: { gmail: "readOnly", calendar: "readOnly" },
  hubspot: { contacts: "readOnly", companies: "readOnly", deals: "readOnly" },
  stripe: { payments: "readOnly", customers: "readOnly", invoices: "readOnly" },
  wordpress: { posts: "readOnly", store: "readOnly" },
};

const KEYED: readonly Built[] = ["hubspot", "stripe", "wordpress"];

function makeCard(
  service: Built,
  id: string,
  levels: Partial<Record<Part, PartLevel>>,
  card: Partial<ConnectionCard>,
  personal = false,
): ConnectionCard {
  const parts: Partial<Record<Part, PartLevel>> = {};
  for (const p of PARTS[service]) parts[p] = levels[p] ?? STARTING[service][p] ?? "off";
  return {
    connection: {
      id,
      service,
      parts,
      granted: [],
      access: [],
      sendList: [],
      state: "notConnected",
      ...(card.connection ?? {}),
    },
    hasApp: service !== "google",
    builtInApp: service !== "google" && !KEYED.includes(service),
    signingIn: false,
    parts: PARTS[service].map((p): PartCard => ({
      part: p,
      label: PART_WORDS[p][0],
      level: parts[p] ?? "off",
      available: !(personal && (p === "sharepoint" || p === "teams")),
      fullAccess: p !== "search" && p !== "customers",
      reads: PART_WORDS[p][1],
      changes: PART_WORDS[p][2],
      needsAdmin: p === "teams",
    })),
    reconnectFor: [],
    granted: [],
    usesKey: KEYED.includes(service),
    keyNeeds:
      service === "hubspot"
        ? ["crm.objects.contacts.read", "crm.objects.companies.read", "crm.objects.deals.read"]
        : service === "stripe"
          ? ["Balance: Read", "PaymentIntents: Read", "Payouts: Read", "Customers: Read"]
          : [],
    storeKeyKept: false,
    ...card,
  };
}

/** HubSpot's, Stripe's, or the website's card (not connected, unless `card` says so). */
export function keyedCard(
  service: "hubspot" | "stripe" | "wordpress",
  levels: Partial<Record<Part, PartLevel>> = {},
  card: Partial<ConnectionCard> = {},
): ConnectionCard {
  return makeCard(service, service, levels, card);
}

/** An add-on program, off, with its tools looked at (all Off) unless `extra` says otherwise. */
export function sampleAddOn(extra: Partial<AddOn> = {}): AddOn {
  return {
    id: "tickets",
    name: "Tickets",
    program: "C:\\Tools\\tickets-mcp.exe",
    args: ["--stdio"],
    secrets: [],
    on: false,
    tools: [
      {
        name: "lookup_order",
        alias: "addon_tickets_lookup_order",
        mark: "off",
        description: "Looks up an order by its number.",
        input: { type: "object" },
        readOnlyHint: true,
        changed: false,
      },
      {
        name: "create_ticket",
        alias: "addon_tickets_create_ticket",
        mark: "off",
        description: "Creates a support ticket.",
        input: { type: "object" },
        changed: false,
      },
    ],
    access: [],
    checkedAt: 1,
    addedAt: 1,
    ...extra,
  };
}

/** Microsoft 365's card as Settings shows it; `levels` and `card` change it. */
export function sampleCard(
  levels: Partial<Record<Part, PartLevel>> = {},
  card: Partial<ConnectionCard> = {},
  personal = false,
): ConnectionCard {
  return makeCard("microsoft365", "microsoft365", levels, card, personal);
}

/** A Slack workspace's card (`slack`, `slack-2`, …). */
export function slackCard(
  id = "slack",
  levels: Partial<Record<Part, PartLevel>> = {},
  card: Partial<ConnectionCard> = {},
): ConnectionCard {
  return makeCard("slack", id, levels, card);
}

/** Google's card (no app saved, unless `card` says so). */
export function googleCard(
  levels: Partial<Record<Part, PartLevel>> = {},
  card: Partial<ConnectionCard> = {},
): ConnectionCard {
  return makeCard("google", "google", levels, card);
}

/** A connected Microsoft 365 card. */
export function connectedCard(extra: Partial<ConnectionCard> = {}): ConnectionCard {
  const base = sampleCard({ mail: "fullAccess" });
  return {
    ...base,
    connection: {
      ...base.connection,
      state: "connected",
      accountKind: "work",
      account: {
        name: "Alex Rivera",
        address: "alex@8westit.com",
        organization: "8 West IT",
      },
      granted: ["Mail.ReadWrite", "Mail.Send", "Calendars.Read"],
      connectedAt: 1,
    },
    granted: [
      { name: "Mail.ReadWrite", words: "Read your mail and save drafts" },
      {
        name: "Mail.Send",
        words: "Send mail as you (asks you first, unless everyone is on your list)",
      },
      { name: "Calendars.Read", words: "Read your calendar" },
    ],
    ...extra,
  };
}

/** The app description a workspace pastes into Slack (a short stand-in). */
export const SAMPLE_MANIFEST = `{
  "oauth_config": {
    "redirect_urls": ["http://localhost:47211", "http://localhost:47212", "http://localhost:47213"],
    "pkce_enabled": true
  }
}`;

/**
 * Settings → Connections with Microsoft 365's `card` (not connected by default), Slack's cards,
 * and Google's card.
 */
export function samplePage(
  card: ConnectionCard = sampleCard(),
  page: Partial<ConnectionsPage> = {},
  others: {
    slack?: ConnectionCard[];
    google?: ConnectionCard;
    hubspot?: ConnectionCard;
    stripe?: ConnectionCard;
    wordpress?: ConnectionCard;
  } = {},
): ConnectionsPage {
  const built = (service: Service, label: string, connections: ConnectionCard[]): ServiceCard => ({
    service,
    label,
    built: true,
    many: service === "slack",
    connections,
  });
  return {
    services: [
      built("microsoft365", "Microsoft 365", [card]),
      built("slack", "Slack", others.slack ?? [slackCard()]),
      built("google", "Google", [others.google ?? googleCard()]),
      built("hubspot", "HubSpot", [others.hubspot ?? keyedCard("hubspot")]),
      built("stripe", "Stripe", [others.stripe ?? keyedCard("stripe")]),
      built("wordpress", "WordPress and WooCommerce", [others.wordpress ?? keyedCard("wordpress")]),
    ],
    people: [
      { kind: "role", id: "role-sup", name: "Supervisor", archived: false },
      { kind: "role", id: "role-writer", name: "Writer", archived: false },
      {
        kind: "agent",
        id: "pos-dev",
        name: "Backend Developer",
        role: "Senior Developer",
        archived: false,
      },
      { kind: "agent", id: "pos-old", name: "Old Scout", role: "Writer", archived: true },
    ],
    sendSwitchOn: false,
    vaultAvailable: true,
    vaultLabel: "Windows Credential Manager",
    slackManifest: SAMPLE_MANIFEST,
    addOns: [],
    secretNames: ["Notion key"],
    ...page,
  };
}
