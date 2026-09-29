import type { IconName } from "@plenipo/ui";

/** The sections of Settings, one at a time (Phase 12). */
export type SettingsSection =
  | "aiTools"
  | "aiModels"
  | "permissions"
  | "organization"
  | "servers"
  | "connections"
  | "switches"
  | "notifications"
  | "terminal"
  | "startAndClose"
  | "personalization"
  | "localPaths"
  | "diagnostics"
  | "updates"
  | "about";

/** Each section: its name on the list, and a line about it at its top. */
export const SETTINGS_SECTIONS: readonly {
  id: SettingsSection;
  label: string;
  icon: IconName;
  lead: string;
}[] = [
  {
    id: "aiTools",
    label: "AI tools",
    icon: "aiTools",
    lead: "The AI tools on this computer, and whether each is signed in. Plenipo uses their own sign-in and never asks for passwords.",
  },
  {
    id: "aiModels",
    label: "AI models",
    icon: "list",
    lead: "The models and effort your agents use: rules for the organization, departments, roles, and agents, each with a first choice and backups tried in order.",
  },
  {
    id: "permissions",
    label: "Permissions",
    icon: "shield",
    lead: "What workers may do on this computer, what always asks you first, and the secrets they may use.",
  },
  {
    id: "organization",
    label: "Organization",
    icon: "organization",
    lead: "Your departments and projects. Add or change them on the Organization map.",
  },
  {
    id: "servers",
    label: "Servers",
    icon: "server",
    lead: "The servers workers may reach, how Plenipo signs in, and the rules for each.",
  },
  {
    id: "connections",
    label: "Connections",
    icon: "link",
    lead: "Your business accounts workers may use, like Microsoft 365. You sign in on the service's own page in your browser; Plenipo never sees your password.",
  },
  {
    id: "switches",
    label: "Switches",
    icon: "settings",
    lead: "Turn whole features on or off.",
  },
  {
    id: "notifications",
    label: "Notifications",
    icon: "bell",
    lead: "Which pop-up notices Windows shows you when something needs you.",
  },
  {
    id: "terminal",
    label: "Terminal",
    icon: "terminal",
    lead: "The shell your terminal on this PC starts.",
  },
  {
    id: "startAndClose",
    label: "Start and close",
    icon: "play",
    lead: "Whether Plenipo starts with Windows, and what closing its window does.",
  },
  {
    id: "personalization",
    label: "Personalization",
    icon: "user",
    lead: "What the app calls the ranks, and how it looks.",
  },
  {
    id: "localPaths",
    label: "Local paths",
    icon: "file",
    lead: "Where Plenipo keeps its files on this computer.",
  },
  {
    id: "diagnostics",
    label: "Diagnostics",
    icon: "diagnostics",
    lead: "How Plenipo is doing, and the technical details for troubleshooting.",
  },
  {
    id: "updates",
    label: "Updates",
    icon: "refresh",
    lead: "New versions of Plenipo: checked once a day, installed only when you say so.",
  },
  {
    id: "about",
    label: "About Plenipo",
    icon: "info",
    lead: "The version you have, and how Plenipo keeps you in charge.",
  },
];

export const SETTINGS_SECTION_KEY = "plenipo.settings.section";

export function isSettingsSection(v: unknown): v is SettingsSection {
  return SETTINGS_SECTIONS.some((s) => s.id === v);
}
