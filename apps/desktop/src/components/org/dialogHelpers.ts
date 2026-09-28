/** Shared by the organization's dialogs: choices, forms, and a role's working instructions. */
import { useState } from "react";
import type { PositionInfo, RoleJob } from "@plenipo/types";

import { rankName, type TitleSet } from "../../org/titles";

export const OWNER_VALUE = "__owner__";
export const MAX_OBJECTIVE_FIELD = 4000;

/** "Website Supervisor", or "Engineering Lead — Manager" when the title does not say its rank
 * (a worker's role, for workers). */
export function positionChoiceLabel(t: TitleSet, p: PositionInfo): string {
  const label = p.kind === "worker" ? p.roleName : rankName(t, p.kind);
  return p.title === label || p.title.endsWith(` ${label}`) ? p.title : `${p.title} — ${label}`;
}

export function useSubmit() {
  const [pending, setPending] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const run = async (work: () => Promise<string | null>) => {
    setPending(true);
    setError(null);
    const failure = await work();
    setPending(false);
    setError(failure);
  };
  return { pending, error, run };
}

export const JOB_FIELDS: { key: keyof RoleJob; label: string; hint: string }[] = [
  {
    key: "duties",
    label: "Its job",
    hint: "What it is responsible for, one item per line.",
  },
  {
    key: "returns",
    label: "What it hands back",
    hint: "What its lead gets when it is done, one item per line.",
  },
  {
    key: "limits",
    label: "What it must not do",
    hint: "Its limits, one item per line. Its permission set still decides what it can do.",
  },
  {
    key: "askLead",
    label: "When it asks its lead for help",
    hint: "One situation per line.",
  },
];

export const EMPTY_JOB: RoleJob = { duties: [], returns: [], limits: [], askLead: [] };

export const jobLines = (text: string) =>
  text
    .split("\n")
    .map((l) => l.replace(/^\s*[-*•]\s*/, "").trim())
    .filter((l) => l !== "");
