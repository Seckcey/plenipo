import type {
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
};

const PARTS: Record<"microsoft365" | "slack" | "google", Part[]> = {
  microsoft365: ["mail", "calendar", "onedrive", "sharepoint", "teams"],
  slack: ["channels", "directMessages", "search"],
  google: ["gmail", "calendar", "drive"],
};

const STARTING: Record<"microsoft365" | "slack" | "google", Partial<Record<Part, PartLevel>>> = {
  microsoft365: { mail: "readOnly", calendar: "readOnly" },
  slack: { channels: "readOnly" },
  google: { gmail: "readOnly", calendar: "readOnly" },
};

function makeCard(
  service: "microsoft365" | "slack" | "google",
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
    builtInApp: service !== "google",
    signingIn: false,
    parts: PARTS[service].map((p): PartCard => ({
      part: p,
      label: PART_WORDS[p][0],
      level: parts[p] ?? "off",
      available: !(personal && (p === "sharepoint" || p === "teams")),
      fullAccess: p !== "search",
      reads: PART_WORDS[p][1],
      changes: PART_WORDS[p][2],
      needsAdmin: p === "teams",
    })),
    reconnectFor: [],
    granted: [],
    ...card,
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

const LATER: [Service, string][] = [
  ["hubspot", "HubSpot"],
  ["stripe", "Stripe"],
  ["wordpress", "WordPress and WooCommerce"],
];

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
  others: { slack?: ConnectionCard[]; google?: ConnectionCard } = {},
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
      ...LATER.map(([service, label]) => ({
        service,
        label,
        built: false,
        many: false,
        connections: [],
      })),
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
    ...page,
  };
}
