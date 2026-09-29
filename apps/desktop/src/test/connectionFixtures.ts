import type {
  ConnectionCard,
  ConnectionsPage,
  Part,
  PartCard,
  PartLevel,
  ServiceCard,
} from "@plenipo/types";

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
};

const PARTS: Part[] = ["mail", "calendar", "onedrive", "sharepoint", "teams"];

/** Microsoft 365's card as Settings shows it; `levels` and `card` change it. */
export function sampleCard(
  levels: Partial<Record<Part, PartLevel>> = {},
  card: Partial<ConnectionCard> = {},
  personal = false,
): ConnectionCard {
  const parts: Record<Part, PartLevel> = {
    mail: "readOnly",
    calendar: "readOnly",
    onedrive: "off",
    sharepoint: "off",
    teams: "off",
    ...levels,
  };
  return {
    connection: {
      id: "microsoft365",
      service: "microsoft365",
      parts,
      granted: [],
      access: [],
      sendList: [],
      state: "notConnected",
      ...(card.connection ?? {}),
    },
    hasApp: true,
    signingIn: false,
    parts: PARTS.map((p): PartCard => ({
      part: p,
      label: PART_WORDS[p][0],
      level: parts[p],
      available: !(personal && (p === "sharepoint" || p === "teams")),
      reads: PART_WORDS[p][1],
      changes: PART_WORDS[p][2],
      needsAdmin: p === "teams",
    })),
    reconnectFor: [],
    granted: [],
    ...card,
  };
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

const LATER: [ServiceCard["service"], string][] = [
  ["slack", "Slack"],
  ["google", "Google"],
  ["hubspot", "HubSpot"],
  ["stripe", "Stripe"],
  ["wordpress", "WordPress and WooCommerce"],
];

/** Settings → Connections with Microsoft 365's `card` (not connected by default). */
export function samplePage(
  card: ConnectionCard = sampleCard(),
  page: Partial<ConnectionsPage> = {},
): ConnectionsPage {
  return {
    services: [
      { service: "microsoft365", label: "Microsoft 365", built: true, connections: [card] },
      ...LATER.map(([service, label]) => ({ service, label, built: false, connections: [] })),
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
    ...page,
  };
}
