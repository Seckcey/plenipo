import { useState } from "react";
import type { OrgSnapshot, ProjectInfo } from "@plenipo/types";
import { Button } from "@plenipo/ui";

import { toCommandError, updateProject } from "../../api/commands";
import { EditProjectDialog } from "./OrgDialogs";

/**
 * **Edit project** where a project is shown (the Projects page, the project's own page): the same
 * form as in its Supervisor's details on the map, saved the same way.
 */
export function EditProjectButton({
  snapshot,
  project,
  onSaved,
}: {
  snapshot: OrgSnapshot;
  project: ProjectInfo;
  /** The organization as saved. */
  onSaved: (next: OrgSnapshot) => void;
}) {
  const [open, setOpen] = useState(false);
  return (
    <>
      <Button size="sm" variant="quiet" icon="settings" onClick={() => setOpen(true)}>
        Edit project
      </Button>
      {open && (
        <EditProjectDialog
          snapshot={snapshot}
          project={project}
          onCancel={() => setOpen(false)}
          onSubmit={async (input) => {
            try {
              onSaved(await updateProject(project.id, input));
              setOpen(false);
              return null;
            } catch (reason) {
              return toCommandError(reason).message;
            }
          }}
        />
      )}
    </>
  );
}
