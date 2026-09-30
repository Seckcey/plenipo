import type { ReactNode } from "react";
import { IconButton, Icon } from "@plenipo/ui";

import { sizeWords } from "./refs";
import type { ObjectiveFilesState } from "./useObjectiveFiles";

/** The files on the objective, each with Remove, and how to add more. */
export function ObjectiveFilesList({
  state,
  hint,
}: {
  state: ObjectiveFilesState;
  hint?: ReactNode;
}) {
  return (
    <div className="objective-files">
      {state.files.length > 0 ? (
        <ul className="objective-files__list" aria-label="Files for this objective">
          {state.files.map((f, i) => (
            <li key={`${f.name}-${i}`}>
              <Icon name="file" size={12} />
              <span>{f.name}</span>
              {typeof f.size === "number" && <span className="muted">{sizeWords(f.size)}</span>}
              <IconButton icon="close" label={`Remove ${f.name}`} onClick={() => state.remove(i)} />
            </li>
          ))}
        </ul>
      ) : (
        <p className="muted objective-files__hint">
          {hint ??
            "Drop files here, from Files or from File Explorer, to put them on the objective."}
        </p>
      )}
      {state.note && (
        <p className="hint" role="status">
          {state.note}
        </p>
      )}
    </div>
  );
}
