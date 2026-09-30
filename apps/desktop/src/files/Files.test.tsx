import { act, cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { FileRoots, FileView, LedgerEvent, WatchUpdate } from "@plenipo/types";
import { beforeEach, describe, expect, it, vi } from "vitest";

import * as commands from "../api/commands";
import * as events from "../api/events";
import { EditorPage } from "./EditorPage";
import { editorStore } from "./editorStore";
import { FilesPanel } from "./FilesPanel";
import { ObjectiveFilesList } from "./ObjectiveFiles";
import { useObjectiveFiles } from "./useObjectiveFiles";
import { ATTACH_EVENT } from "./refs";

// The editor draws with CodeMirror, which needs a real screen: a stand-in keeps the text.
vi.mock("./editorSetup", () => ({
  SAVE_EVENT: "plenipo:save",
  editorState: (text: string) => ({ doc: { toString: () => text } }),
}));
vi.mock("./CodeEditor", () => ({
  CodeEditor: ({
    state,
    readOnly,
    shown,
    label,
    onChange,
  }: {
    state: { doc: { toString: () => string } };
    readOnly: boolean;
    shown: string | null;
    label: string;
    onChange: (s: unknown) => void;
  }) => (
    <textarea
      aria-label={label}
      readOnly={readOnly || shown !== null}
      value={shown ?? undefined}
      defaultValue={shown === null ? state.doc.toString() : undefined}
      onChange={(e) => {
        const text = e.target.value;
        onChange({ doc: { toString: () => text } });
      }}
    />
  ),
}));

vi.mock("../api/commands", async (importOriginal) => {
  const actual = await importOriginal<typeof commands>();
  return {
    ...actual,
    getFileRoots: vi.fn(),
    listFolder: vi.fn(),
    getChangingFiles: vi.fn(),
    readFile: vi.fn(),
    saveFile: vi.fn(),
    openFileOutside: vi.fn(),
    showInFolder: vi.fn(),
    getWatchChange: vi.fn(),
    cancelAgentTurn: vi.fn(),
    getControlStatus: vi.fn(),
  };
});
vi.mock("../api/events", () => ({
  subscribeLedgerEvents: vi.fn(),
  subscribeWatch: vi.fn(),
  subscribeControl: vi.fn(),
}));

const api = vi.mocked(commands);
let ledger: (e: LedgerEvent) => void = () => undefined;
let watch: (u: WatchUpdate) => void = () => undefined;

const ROOT = "project:p1";
const COPY = "copy:w1";

const roots = (writer = false): FileRoots => ({
  desktopInUse: false,
  roots: [
    {
      id: ROOT,
      projectId: "p1",
      projectName: "Website",
      kind: "projectFolder",
      label: "Project folder",
      path: "/home/owner/website",
      exists: true,
    },
    {
      id: COPY,
      projectId: "p1",
      projectName: "Website",
      kind: "workingCopy",
      label: "plenipo/fix-login-a1b2",
      path: "/data/working-copies/w1",
      exists: true,
      ...(writer
        ? {
            writer: {
              worker: "Senior Developer",
              sessionId: "s1",
              taskId: "t1",
            },
          }
        : {}),
    },
  ],
});

const text = (
  root: string,
  path: string,
  body: string,
  extra: Partial<FileView> = {},
): FileView => ({
  root,
  path,
  name: path.split("/").at(-1) ?? path,
  size: body.length,
  modified: 1,
  hash: "a".repeat(64),
  content: { kind: "text", text: body, bom: false, lineEnding: "lf" },
  runs: false,
  blocked: false,
  ...extra,
});

let seq = 0;
const event = (eventType: string): LedgerEvent => ({
  seq: ++seq,
  id: `e${seq}`,
  taskId: null,
  executionId: null,
  source: "guard",
  destination: null,
  eventType,
  payload: {},
  createdAt: Date.now(),
});

beforeEach(() => {
  cleanup();
  localStorage.clear();
  for (const key of editorStore.snapshot().keys) editorStore.close(key);
  vi.useRealTimers();
  const listening = new Set<(e: LedgerEvent) => void>();
  ledger = (e) => listening.forEach((h) => h(e));
  vi.mocked(events.subscribeLedgerEvents).mockImplementation((h) => {
    listening.add(h);
    return Promise.resolve(() => listening.delete(h));
  });
  const watching = new Set<(u: WatchUpdate) => void>();
  watch = (u) => watching.forEach((h) => h(u));
  vi.mocked(events.subscribeWatch).mockImplementation((h) => {
    watching.add(h);
    return Promise.resolve(() => watching.delete(h));
  });
  vi.mocked(events.subscribeControl).mockResolvedValue(() => undefined);
  api.getControlStatus.mockResolvedValue({
    stopped: false,
    sessions: [],
    revision: 1,
  });
  api.getFileRoots.mockResolvedValue(roots());
  api.getChangingFiles.mockResolvedValue([]);
  api.listFolder.mockImplementation((root, path) =>
    Promise.resolve({
      root,
      path,
      more: 0,
      entries:
        path === ""
          ? [
              {
                name: "src",
                path: "src",
                folder: true,
                size: null,
                modified: null,
                blocked: false,
                runs: false,
              },
              {
                name: ".env",
                path: ".env",
                folder: false,
                size: 12,
                modified: 1,
                blocked: true,
                runs: false,
              },
              {
                name: "README.md",
                path: "README.md",
                folder: false,
                size: 20,
                modified: 1,
                blocked: false,
                runs: false,
              },
            ]
          : [
              {
                name: "app.txt",
                path: "src/app.txt",
                folder: false,
                size: 4,
                modified: 1,
                blocked: false,
                runs: false,
              },
            ],
    }),
  );
  api.openFileOutside.mockResolvedValue();
  api.showInFolder.mockResolvedValue();
  api.cancelAgentTurn.mockResolvedValue({} as never);
});

describe("the Files panel (Phase 21, ADR-093)", () => {
  it("lists each project's folder and working copies, marks, and opens a file in Plenipo", async () => {
    const user = userEvent.setup();
    const go = vi.fn();
    api.getFileRoots.mockResolvedValue(roots(true));
    api.getChangingFiles.mockResolvedValue([
      { root: ROOT, path: "README.md", worker: "Senior Developer" },
    ]);
    render(<FilesPanel go={go} />);
    const tree = await screen.findByRole("tree", { name: "Project folders and working copies" });
    await user.click(within(tree).getByText("Website"));
    await user.keyboard("{Enter}");
    await user.dblClick(await within(tree).findByText("Project folder"));
    expect(await within(tree).findByText("README.md")).toBeInTheDocument();
    // Folders first, then files; a blocked file says so in words.
    const rows = within(tree)
      .getAllByRole("treeitem")
      .map((r) => r.textContent);
    expect(rows.slice(2, 5).map((t) => t?.split(/Blocked|Senior|\d/)[0])).toEqual([
      "src",
      ".env",
      "README.md",
    ]);
    expect(within(tree).getByText("Blocked for workers")).toBeInTheDocument();
    expect(within(tree).getByText("Senior Developer is changing this")).toBeInTheDocument();
    // The working copy a worker writes in says so.
    await user.click(within(tree).getByText("Working copies (1)"));
    await user.keyboard("{Enter}");
    expect(await within(tree).findByText("Senior Developer is writing here")).toBeInTheDocument();
    // Double-click opens the file in Plenipo; the bar offers the rest.
    await user.dblClick(within(tree).getByText("README.md"));
    expect(go).toHaveBeenCalledWith({ view: "file", id: `${ROOT}/README.md` });
    await user.click(screen.getByRole("button", { name: "Open in another program" }));
    expect(api.openFileOutside).toHaveBeenCalledWith(ROOT, "README.md");
    await user.click(screen.getByRole("button", { name: "Show in folder" }));
    expect(api.showInFolder).toHaveBeenCalledWith(ROOT, "README.md");
  });

  it("never offers another program for a program or a script", async () => {
    const user = userEvent.setup();
    api.listFolder.mockImplementation((root, path) =>
      Promise.resolve({
        root,
        path,
        more: 0,
        entries: [
          {
            name: "setup.ps1",
            path: "setup.ps1",
            folder: false,
            size: 9,
            modified: 1,
            blocked: false,
            runs: true,
          },
        ],
      }),
    );
    render(<FilesPanel go={vi.fn()} />);
    const tree = await screen.findByRole("tree", { name: "Project folders and working copies" });
    await user.click(within(tree).getByText("Website"));
    await user.keyboard("{Enter}");
    await user.dblClick(await within(tree).findByText("Project folder"));
    await user.click(await within(tree).findByText("setup.ps1"));
    expect(screen.getByRole("button", { name: "Open in Plenipo" })).toBeEnabled();
    expect(screen.getByRole("button", { name: "Open in another program" })).toBeDisabled();
  });

  it("reads the folders again when a worker's step starts or ends", async () => {
    render(<FilesPanel go={vi.fn()} />);
    await screen.findByRole("tree");
    expect(api.getFileRoots).toHaveBeenCalledTimes(1);
    act(() => ledger(event("guard.grant_opened")));
    await waitFor(() => expect(api.getFileRoots).toHaveBeenCalledTimes(2));
  });
});

describe("the editor (Phase 21, ADR-093)", () => {
  it("edits and saves a file, keeping its fingerprint, and asks before losing unsaved changes", async () => {
    const user = userEvent.setup();
    api.readFile.mockResolvedValue(text(ROOT, "README.md", "# Website\n"));
    api.saveFile.mockResolvedValue({
      kind: "saved",
      hash: "b".repeat(64),
      size: 30,
      modified: 2,
      added: 1,
      removed: 0,
    });
    const go = vi.fn();
    render(<EditorPage id={`${ROOT}/README.md`} go={go} />);
    const box = await screen.findByRole("textbox", { name: "README.md, editable" });
    await user.type(box, "More.");
    expect(screen.getByText("Not saved yet")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Save" }));
    await waitFor(() =>
      expect(api.saveFile).toHaveBeenCalledWith(
        ROOT,
        "README.md",
        "# Website\nMore.",
        false,
        "lf",
        "a".repeat(64),
      ),
    );
    expect(await screen.findByText(/Saved · 1 line added, 0 removed/)).toBeInTheDocument();
    // Typed again and closed: Plenipo asks first.
    await user.type(box, "!");
    await user.click(screen.getByRole("button", { name: "Close README.md" }));
    expect(screen.getByRole("alert")).toHaveTextContent("README.md has changes not saved yet");
    await user.click(screen.getByRole("button", { name: "Keep it open" }));
    expect(editorStore.snapshot().keys).toContain(`${ROOT}/README.md`);
  });

  it("does not save over a file that changed on the disk unless the owner says so", async () => {
    const user = userEvent.setup();
    api.readFile.mockResolvedValue(text(ROOT, "README.md", "x\n"));
    api.saveFile.mockResolvedValueOnce({ kind: "changedOnDisk" }).mockResolvedValueOnce({
      kind: "saved",
      hash: "c".repeat(64),
      size: 3,
      modified: 3,
      added: 1,
      removed: 1,
    });
    render(<EditorPage id={`${ROOT}/README.md`} go={vi.fn()} />);
    await user.type(await screen.findByRole("textbox"), "y");
    await user.click(screen.getByRole("button", { name: "Save" }));
    expect(
      await screen.findByText("This file changed on the disk since you opened it"),
    ).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Save anyway" }));
    await waitFor(() =>
      expect(api.saveFile).toHaveBeenLastCalledWith(ROOT, "README.md", "x\ny", false, "lf", null),
    );
  });

  it("keeps the fingerprint of the file as you started editing it, so a worker's change is never saved over", async () => {
    const user = userEvent.setup();
    const writer = { worker: "Senior Developer", sessionId: "s1", taskId: "t1" };
    api.readFile.mockResolvedValueOnce(text(COPY, "src/app.txt", "a\n"));
    api.saveFile.mockResolvedValue({ kind: "changedOnDisk" });
    render(<EditorPage id={`${COPY}/src/app.txt`} go={vi.fn()} />);
    await user.type(await screen.findByRole("textbox", { name: "app.txt, editable" }), "mine");
    // A worker starts writing here, changes the file, and finishes: each step reads it again.
    api.readFile.mockResolvedValueOnce(
      text(COPY, "src/app.txt", "a\nb\n", {
        hash: "d".repeat(64),
        readOnly: { kind: "writer", writer },
      }),
    );
    act(() => ledger(event("guard.grant_opened")));
    expect(
      await screen.findByText("Senior Developer is writing in this working copy"),
    ).toBeInTheDocument();
    api.readFile.mockResolvedValue(text(COPY, "src/app.txt", "a\nb\n", { hash: "e".repeat(64) }));
    act(() => ledger(event("guard.grant_closed")));
    expect(
      await screen.findByText("Senior Developer is done. You can edit again."),
    ).toBeInTheDocument();
    // Your unsaved text stays, and Save sends the file's fingerprint from before the worker.
    await user.click(screen.getByRole("button", { name: "Save" }));
    await waitFor(() =>
      expect(api.saveFile).toHaveBeenLastCalledWith(
        COPY,
        "src/app.txt",
        "a\nmine",
        false,
        "lf",
        "a".repeat(64),
      ),
    );
    expect(
      await screen.findByText("This file changed on the disk since you opened it"),
    ).toBeInTheDocument();
  });

  it("opens a working copy a worker is writing read-only, with Wait and Stop the worker", async () => {
    const user = userEvent.setup();
    const writer = { worker: "Senior Developer", sessionId: "s1", taskId: "t1" };
    api.readFile.mockResolvedValueOnce(
      text(COPY, "src/app.txt", "a\n", { readOnly: { kind: "writer", writer } }),
    );
    render(<EditorPage id={`${COPY}/src/app.txt`} go={vi.fn()} />);
    expect(
      await screen.findByText("Senior Developer is writing in this working copy"),
    ).toBeInTheDocument();
    expect(screen.getByRole("textbox", { name: "app.txt, read-only" })).toHaveAttribute("readonly");
    expect(screen.getByRole("button", { name: "Save" })).toBeDisabled();
    await user.click(screen.getByRole("button", { name: "Wait" }));
    expect(screen.getByText("Waiting for Senior Developer to finish…")).toBeInTheDocument();
    // A worker's change lands as it is written, then saved with its lines.
    act(() =>
      watch({
        change: {
          id: "c1",
          taskId: "t1",
          sessionId: "s1",
          worker: "Senior Developer",
          objectiveTaskId: "o1",
          path: "src/app.txt",
          root: COPY,
          state: "writing",
          added: 0,
          removed: 0,
          at: 1,
        },
        writing: "a\nb\n",
      }),
    );
    expect(
      await screen.findByText("Senior Developer: being written — not saved yet"),
    ).toBeInTheDocument();
    // Stop the worker, after saying yes.
    await user.click(screen.getByRole("button", { name: "Stop the worker" }));
    expect(screen.getByText(/Stop Senior Developer's task\?/)).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Stop the worker" }));
    expect(api.cancelAgentTurn).toHaveBeenCalledWith("s1");
    // Its step ends: the file is read again, and is the owner's to edit.
    api.readFile.mockResolvedValue(text(COPY, "src/app.txt", "a\nb\n"));
    act(() => ledger(event("guard.grant_closed")));
    expect(
      await screen.findByText("Senior Developer is done. You can edit again."),
    ).toBeInTheDocument();
    expect(screen.getByRole("textbox", { name: "app.txt, editable" })).not.toHaveAttribute(
      "readonly",
    );
  });

  it("never offers another program for a program or a script, and says what other files are", async () => {
    api.readFile.mockResolvedValue({
      ...text(ROOT, "tool.exe", ""),
      content: { kind: "other", what: "A program or a script. Plenipo never starts it." },
      runs: true,
    });
    render(<EditorPage id={`${ROOT}/tool.exe`} go={vi.fn()} />);
    expect(await screen.findByText(/Plenipo never starts it/)).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Open in another program" })).toBeNull();
    expect(screen.getByRole("button", { name: "Show in folder" })).toBeInTheDocument();
  });
});

describe("files on an objective (Phase 21, ADR-093 §19)", () => {
  function Form({ allowed }: { allowed: boolean }) {
    const [files, dropTarget] = useObjectiveFiles(allowed);
    return (
      <form ref={dropTarget} {...files.dropProps} aria-label="Give an objective">
        <ObjectiveFilesList state={files} />
        <output data-testid="sent">{JSON.stringify(files.toSend())}</output>
      </form>
    );
  }

  it("takes files dropped from Files or File Explorer, not folders, and each can be removed", async () => {
    const user = userEvent.setup();
    render(<Form allowed />);
    const form = screen.getByRole("form", { name: "Give an objective" });
    act(() => {
      form.dispatchEvent(
        new CustomEvent(ATTACH_EVENT, {
          detail: [{ root: ROOT, path: "README.md", name: "README.md" }],
        }),
      );
      form.dispatchEvent(
        new CustomEvent(ATTACH_EVENT, {
          detail: [
            { file: { kind: "dropped", drop: "d1", index: 0 }, name: "brief.pdf", size: 2048 },
            { file: { kind: "dropped", drop: "d1", index: 1 }, name: "Photos", folder: true },
          ],
        }),
      );
    });
    const list = screen.getByRole("list", { name: "Files for this objective" });
    expect(within(list).getAllByRole("listitem")).toHaveLength(2);
    expect(screen.getByText(/Photos: a folder/)).toBeInTheDocument();
    expect(JSON.parse(screen.getByTestId("sent").textContent ?? "")).toEqual([
      { kind: "file", root: ROOT, path: "README.md" },
      { kind: "dropped", drop: "d1", index: 0 },
    ]);
    await user.click(screen.getByRole("button", { name: "Remove README.md" }));
    expect(within(list).getAllByRole("listitem")).toHaveLength(1);
  });

  it("says why when the objective has no project", () => {
    render(<Form allowed={false} />);
    const form = screen.getByRole("form", { name: "Give an objective" });
    act(() => {
      form.dispatchEvent(
        new CustomEvent(ATTACH_EVENT, { detail: [{ root: ROOT, path: "a", name: "a" }] }),
      );
    });
    expect(screen.getByText(/objective for a project/)).toBeInTheDocument();
  });
});
