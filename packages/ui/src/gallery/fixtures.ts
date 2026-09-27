/** Sample data for the Gallery: realistic, and the same on every run (seeded). */

import { bucketActivity, DAY_MS, type ActivityKind, type ActivitySeries } from "../activity";
import type { EntityCardProps } from "../cards";
import type { TimelineEvent } from "../detail";
import type { IconName } from "../icons";
import type { Status } from "../status";
import type { MapLink, MapNode } from "../topology";

/** A small seeded random generator (mulberry32), so the Gallery looks the same every time. */
export function seeded(seed: number) {
  let s = seed >>> 0;
  return () => {
    s = (s + 0x6d2b79f5) >>> 0;
    let t = s;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

const pick = <T>(rand: () => number, list: readonly T[]): T =>
  list[Math.floor(rand() * list.length)] as T;

/** A day of activity: bursts of work, a few problems, a few approvals. */
export function sampleActivity(seed: number, now: number, busy = 1): ActivitySeries {
  const rand = seeded(seed);
  const from = now - DAY_MS;
  const items: { at: number; kind: ActivityKind }[] = [];
  const bursts = Math.round(3 + rand() * 5 * busy);
  for (let b = 0; b < bursts; b++) {
    const center = from + rand() * DAY_MS;
    const size = Math.round(4 + rand() * 30 * busy);
    for (let i = 0; i < size; i++) {
      const at = center + (rand() - 0.5) * 3_600_000;
      const roll = rand();
      items.push({ at, kind: roll < 0.03 ? "problem" : roll < 0.06 ? "waiting" : "event" });
    }
  }
  return bucketActivity(items, from, now);
}

export const STATUS_WORDS: Record<Status, string> = {
  ok: "Working",
  warn: "Blocked",
  error: "Failed",
  offline: "Idle",
  pending: "Waiting for you",
};

const RESOURCE_SETS: { icon: IconName; label: string }[][] = [
  [
    { icon: "file", label: "Read and change files" },
    { icon: "branch", label: "Use Git" },
    { icon: "terminal", label: "Run programs" },
  ],
  [
    { icon: "globe", label: "Use websites" },
    { icon: "file", label: "Read files" },
  ],
  [
    { icon: "server", label: "Connect to servers" },
    { icon: "terminal", label: "Run programs" },
    { icon: "key", label: "Use secrets" },
  ],
  [],
];

export function sampleCards(now: number): EntityCardProps[] {
  return [
    {
      title: "Development",
      status: "ok",
      statusLabel: "Working",
      subtype: "Department · 3 projects",
      activity: sampleActivity(1, now, 1.4),
      owner: { icon: "user", label: "Manager · Claude Code" },
      resources: RESOURCE_SETS[0] ?? [],
    },
    {
      title: "Website",
      status: "pending",
      statusLabel: "Waiting for you",
      subtype: "Project · Development",
      activity: sampleActivity(2, now),
      owner: { icon: "user", label: "Supervisor · Codex" },
      resources: RESOURCE_SETS[1] ?? [],
    },
    {
      title: "Shop server",
      status: "error",
      statusLabel: "Failed",
      subtype: "Project · Operations",
      activity: sampleActivity(3, now, 0.6),
      owner: { icon: "server", label: "Operations Engineer · Claude Code" },
      resources: RESOURCE_SETS[2] ?? [],
    },
    {
      title: "Research",
      status: "offline",
      statusLabel: "Idle",
      subtype: "Department · no projects yet",
      activity: null,
      owner: { icon: "user", label: "Manager · not filled" },
      resources: [],
    },
  ];
}

const FIRST = ["Senior", "Junior", "Lead", "Staff", "Principal"];
const JOBS = [
  "Developer",
  "Code Reviewer",
  "QA Engineer",
  "Documentation Writer",
  "Designer",
  "Operations Engineer",
];
const TOOLS = ["Claude Code", "Codex", "Grok", "Kimi", "Ollama"];
const PROJECTS = ["Website", "Shop", "Milepost", "Cloudline", "Waypoint", "Billing"];
const DEPARTMENTS = ["Development", "Operations", "Marketing"];
const STATUSES: Status[] = ["ok", "ok", "ok", "offline", "offline", "pending", "warn", "error"];

export interface SampleWorker {
  id: string;
  name: string;
  status: Status;
  tool: string;
  project: string;
  department: string;
  tasks: number;
  minutes: number;
  cost: number;
  hoursAgo: number;
}

/** `count` workers, for the table and the grid. */
export function sampleWorkers(count: number, seed = 7): SampleWorker[] {
  const rand = seeded(seed);
  return Array.from({ length: count }, (_, i) => {
    const project = pick(rand, PROJECTS);
    return {
      id: `w${i + 1}`,
      name: `${pick(rand, FIRST)} ${pick(rand, JOBS)} ${i + 1}`,
      status: pick(rand, STATUSES),
      tool: pick(rand, TOOLS),
      project,
      department: pick(rand, DEPARTMENTS),
      tasks: Math.floor(rand() * 120),
      minutes: Math.round(rand() * 900) / 10,
      cost: Math.round(rand() * 4000) / 100,
      hoursAgo: Math.round(rand() * 24 * 90),
    };
  });
}

export function sampleTimeline(now: number): TimelineEvent[] {
  const rand = seeded(11);
  return Array.from({ length: 14 }, (_, i) => ({
    at: now - rand() * DAY_MS,
    label: `Event ${i + 1}`,
    status: pick(rand, STATUSES),
  }));
}

/** A delegation tree: the VP hands work to a Manager, who hands it to a Supervisor's team. */
export function sampleMap(): { nodes: MapNode[]; links: MapLink[] } {
  const nodes: MapNode[] = [
    {
      id: "vp",
      label: "VP",
      status: "ok",
      statusLabel: "Working",
      caption: "1 objective",
      icon: "user",
    },
    {
      id: "dev",
      label: "Development Manager",
      status: "ok",
      statusLabel: "Working",
      caption: "3 tasks",
      icon: "department",
    },
    {
      id: "web",
      label: "Website Supervisor",
      status: "pending",
      statusLabel: "Waiting",
      caption: "2 tasks · 14 min",
      icon: "projects",
    },
    {
      id: "ops",
      label: "Shop Supervisor",
      status: "error",
      statusLabel: "Failed",
      caption: "1 task · 3 min",
      icon: "server",
    },
    {
      id: "sd",
      label: "Senior Developer",
      status: "ok",
      statusLabel: "Working",
      caption: "12 min",
      icon: "user",
    },
    {
      id: "cr",
      label: "Code Reviewer",
      status: "offline",
      statusLabel: "Idle",
      caption: "done 4 min ago",
      icon: "user",
    },
    {
      id: "qa",
      label: "QA Engineer",
      status: "warn",
      statusLabel: "Blocked",
      caption: "needs a test server",
      icon: "user",
    },
  ];
  const links: MapLink[] = [
    { from: "vp", to: "dev", label: "objective" },
    { from: "dev", to: "web", label: "hands to" },
    { from: "dev", to: "ops", label: "hands to" },
    { from: "web", to: "sd", label: "build" },
    { from: "web", to: "cr", label: "review" },
    { from: "ops", to: "qa", label: "check" },
  ];
  return { nodes, links };
}
