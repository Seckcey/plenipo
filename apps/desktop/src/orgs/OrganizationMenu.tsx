import { useState } from "react";
import type { OrgSummary } from "@plenipo/types";
import { Banner, MenuButton, type MenuItem } from "@plenipo/ui";

import { openOrganizationWindow, switchOrganization, toCommandError } from "../api/commands";
import type { Go } from "../components/views";
import { NewOrganizationDialog } from "./NewOrganizationDialog";
import { useOrganizations } from "./useOrganizations";

const NEW = "new";
const MANAGE = "manage";

/**
 * Your organizations, at the top of every organization's window (Phase 21, ADR-094 §11): switch
 * this window to another one, open one in a new window, or make a new one.
 */
export function OrganizationMenu({ go }: { go: Go }) {
  const { listing } = useOrganizations();
  const [creating, setCreating] = useState(false);
  const [problem, setProblem] = useState<string | null>(null);
  if (!listing) return null;
  const others = listing.organizations.filter((o) => !o.archived && o.id !== listing.current);

  const items: MenuItem[] = [
    ...others.flatMap((o): MenuItem[] =>
      o.inWindow
        ? [{ id: `window:${o.id}`, label: `Show ${o.name}'s window`, icon: "organization" }]
        : [
            { id: `switch:${o.id}`, label: `Switch to ${o.name}`, icon: "organization" },
            { id: `window:${o.id}`, label: `Open ${o.name} in a new window`, icon: "external" },
          ],
    ),
    { id: NEW, label: "New organization…", icon: "plus" },
    { id: MANAGE, label: "Your organizations…", icon: "settings" },
  ];

  const open = (org: Pick<OrgSummary, "id">, how: "switch" | "window") => {
    setProblem(null);
    const call = how === "switch" ? switchOrganization(org.id) : openOrganizationWindow(org.id);
    call.catch((reason: unknown) => setProblem(toCommandError(reason).message));
  };

  return (
    <>
      <MenuButton
        label="Organizations"
        icon="organization"
        variant="quiet"
        title="Switch to another organization, or open one in a new window"
        items={items}
        onSelect={(id) => {
          if (id === NEW) setCreating(true);
          else if (id === MANAGE) go({ view: "settings", id: "organization" });
          else {
            const [how, orgId] = id.split(":");
            if (orgId && (how === "switch" || how === "window")) open({ id: orgId }, how);
          }
        }}
      />
      {problem && (
        <div className="shell__notice-popout">
          <Banner tone="error" role="alert" title={problem} onDismiss={() => setProblem(null)} />
        </div>
      )}
      {creating && (
        <NewOrganizationDialog
          listing={listing}
          onClose={() => setCreating(false)}
          onCreated={(org) => {
            setCreating(false);
            open(org, "window");
          }}
        />
      )}
    </>
  );
}
