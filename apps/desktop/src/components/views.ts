import type { IconName } from "@plenipo/ui";

export type ViewId =
  | "home"
  | "organization"
  | "projects"
  | "workers"
  | "approvals"
  | "runtimes"
  | "activity"
  | "settings"
  | "diagnostics"
  | "gallery"
  | PageKind;

/** The pages of one thing (Phase 12): opened from other pages, not from the strip. */
export type PageKind = "department" | "project" | "worker" | "task";

export const PAGE_KINDS: readonly PageKind[] = ["department", "project", "worker", "task"];

/** Where you are: a section, or the page of one department, project, worker, or task. */
export interface Place {
  view: ViewId;
  /** The department, project, position, or task a page is about. */
  id: string | null;
}

export type Go = (place: Place) => void;

export const isPageKind = (view: ViewId): view is PageKind =>
  (PAGE_KINDS as readonly ViewId[]).includes(view);

/** The section on the strip a page belongs to (marked while the page is open). */
export const SECTION_OF: Record<PageKind, ViewId> = {
  department: "organization",
  project: "projects",
  worker: "workers",
  task: "activity",
};

/** The sections on the left strip, top to bottom (Settings and Diagnostics at the bottom). */
export const VIEWS: {
  id: ViewId;
  label: string;
  icon: IconName;
  tooltip: string;
  bottom?: boolean;
}[] = [
  {
    id: "home",
    label: "Home",
    icon: "home",
    tooltip: "Home: how your company is doing, and what needs you",
  },
  {
    id: "organization",
    label: "Organization",
    icon: "organization",
    tooltip: "Organization: your departments, projects, and who reports to whom",
  },
  { id: "projects", label: "Projects", icon: "projects", tooltip: "Projects and their work" },
  { id: "workers", label: "Workers", icon: "workers", tooltip: "Workers and their conversations" },
  {
    id: "approvals",
    label: "Approvals",
    icon: "approvals",
    tooltip: "Requests and lessons waiting for you",
  },
  {
    id: "runtimes",
    label: "AI tools",
    icon: "aiTools",
    tooltip: "AI tools and the programs they run",
  },
  {
    id: "activity",
    label: "Activity",
    icon: "activity",
    tooltip: "Everything that happened, in order",
  },
  { id: "settings", label: "Settings", icon: "settings", tooltip: "Settings", bottom: true },
  {
    id: "diagnostics",
    label: "Diagnostics",
    icon: "diagnostics",
    tooltip: "Diagnostics: technical details for troubleshooting",
    bottom: true,
  },
];

/** Every page's name in the top bar, including pages not on the strip. */
export const VIEW_TITLES: Record<ViewId, string> = {
  home: "Home",
  organization: "Organization",
  projects: "Projects",
  workers: "Workers",
  approvals: "Approvals",
  runtimes: "AI tools",
  activity: "Activity",
  settings: "Settings",
  diagnostics: "Diagnostics",
  gallery: "Gallery",
  department: "Department",
  project: "Project",
  worker: "Worker",
  task: "Task",
};

export const ALL_VIEWS = Object.keys(VIEW_TITLES) as ViewId[];
