import type { IconName } from "@plenipo/ui";

import { systemWords } from "../system/words";

/** The sections of Settings, one at a time (Phase 12). */
export type SettingsSection =
  | "aiTools"
  | "aiModels"
  | "spending"
  | "permissions"
  | "safety"
  | "organization"
  | "servers"
  | "connections"
  | "switches"
  | "devices"
  | "community"
  | "notifications"
  | "terminal"
  | "startAndClose"
  | "personalization"
  | "localPaths"
  | "diagnostics"
  | "license"
  | "updates"
  | "about";

/** Each section: its name on the list, and a line about it at its top (read when shown, in the
 * system's own words, ADR-155). */
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
    lead: "The AI tools on this computer, whether each is signed in, and your keys for paying per use. Plenipo uses their own sign-in and never sees your passwords.",
  },
  {
    id: "aiModels",
    label: "AI models",
    icon: "list",
    lead: "The models and effort your agents use: rules for the organization, departments, roles, and agents, each with a first choice and backups tried in order.",
  },
  {
    id: "spending",
    label: "Spending caps",
    icon: "spending",
    lead: "The most paid AI keys may spend each month: for the whole business, a department, or one position. Caps are up to you: without one, paid work has no dollar limit.",
  },
  {
    id: "permissions",
    label: "Permissions",
    icon: "shield",
    lead: "What workers may do on this computer, what always asks you first, and the secrets they may use.",
  },
  {
    id: "safety",
    label: "Safety",
    icon: "lock",
    lead: "How much Plenipo asks you before an agent saves files or runs programs, and what always asks you first.",
  },
  {
    id: "organization",
    label: "Organization",
    icon: "organization",
    lead: "This organization's name, departments, and projects, and your other organizations.",
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
    id: "devices",
    label: "Devices",
    icon: "phone",
    get lead() {
      return `Use Plenipo from your phone: add a phone, see your phones, and choose which approvals stay on ${systemWords().thisComputer}.`;
    },
  },
  {
    id: "community",
    label: "Community",
    icon: "workers",
    lead: "Your 8 West account in Plenipo: sign in, your Community name, and leaving. Nothing is sent until you turn Community on.",
  },
  {
    id: "notifications",
    label: "Notifications",
    icon: "bell",
    get lead() {
      const w = systemWords();
      return `Which ${w.notices} ${w.theSystem} shows you when something needs you.`;
    },
  },
  {
    id: "terminal",
    label: "Terminal",
    icon: "terminal",
    get lead() {
      return `The shell your terminal on ${systemWords().thisComputer} starts.`;
    },
  },
  {
    id: "startAndClose",
    label: "Start and close",
    icon: "play",
    get lead() {
      const when = systemWords().whenYouSignIn;
      return `Whether Plenipo starts ${when.charAt(0).toLowerCase()}${when.slice(1)}, and what closing its window does.`;
    },
  },
  {
    id: "personalization",
    label: "Personalization",
    icon: "user",
    lead: "What the app calls the ranks, where the panels sit, and how it looks.",
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
    id: "license",
    label: "License",
    icon: "key",
    get lead() {
      return `Free or Plenipo Pro on ${systemWords().thisComputer}, your license key, and the weekly check with 8 West.`;
    },
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
