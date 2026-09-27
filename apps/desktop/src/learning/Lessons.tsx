import { useState } from "react";
import type { LearningSnapshot, Lesson } from "@plenipo/types";

import { decideLesson, removeLesson, setLearning, setRoleLearning } from "../api/commands";
import { Toggle } from "../components/SwitchSettings";
import { useRun } from "../guard/useRun";
import type { Learning } from "./useLearning";

/** Settings → Switches: worker learning on or off (ADR-022). */
export function LearningSwitch({ learning }: { learning: Learning }) {
  const { pending, error, run } = useRun((s: LearningSnapshot) => learning.apply(s));
  const s = learning.snapshot;
  return (
    <>
      <Toggle
        label="Worker learning"
        hint="Workers write down lessons from their work. You keep or discard each one on the Approvals page (or let a role learn on its own), and kept lessons go to that role's later workers. Off: no lessons are recorded or used."
        checked={s?.enabled ?? true}
        disabled={pending || !s}
        onChange={(on) => void run(() => setLearning(on))}
      />
      {error && (
        <p className="form-error" role="alert">
          {error}
        </p>
      )}
    </>
  );
}

/** One lesson waiting for the owner: its words (editable), where it came from, Keep or Discard. */
function LessonCard({
  lesson,
  pending,
  onAnswer,
}: {
  lesson: Lesson;
  pending: boolean;
  onAnswer: (keep: boolean, text: string) => void;
}) {
  const [text, setText] = useState(lesson.text);
  return (
    <article className="lesson" aria-label={`Lesson from ${lesson.worker}`}>
      <header className="approval__header">
        <h3 className="approval__title">{lesson.worker} learned something</h3>
        {lesson.fromWeb && (
          <span className="pill pill--warn" title="Its task used websites or your screen">
            From a task that used websites
          </span>
        )}
      </header>
      {lesson.fromWeb && (
        <p className="muted">
          This task (or a worker it handed work to) read web pages or your screen, so check that the
          lesson is really what the worker learned, not something a website told it to write.
        </p>
      )}
      <label className="field">
        <span>The lesson (edit it before keeping, if you like)</span>
        <textarea rows={2} value={text} onChange={(e) => setText(e.target.value)} />
      </label>
      <div className="actions">
        <button
          type="button"
          className="button"
          disabled={pending || text.trim() === ""}
          onClick={() => onAnswer(true, text)}
        >
          Keep
        </button>
        <button
          type="button"
          className="button button--quiet"
          disabled={pending}
          onClick={() => onAnswer(false, text)}
        >
          Discard
        </button>
      </div>
    </article>
  );
}

/** The Approvals page's "New lessons": lessons waiting for the owner's Keep or Discard. */
export function NewLessons({ learning }: { learning: Learning }) {
  const { pending, error, run } = useRun((s: LearningSnapshot) => learning.apply(s));
  const waiting = learning.snapshot?.waiting ?? [];
  if (waiting.length === 0 && !error) return null;
  return (
    <section aria-labelledby="lessons-title">
      <h2 id="lessons-title">New lessons</h2>
      <p className="muted">
        Kept lessons go into the instructions of that role&apos;s later workers. Nothing waits on
        these: answer when you like.
      </p>
      {waiting.map((l) => (
        <LessonCard
          key={l.id}
          lesson={l}
          pending={pending}
          onAnswer={(keep, text) =>
            void run(() =>
              decideLesson(l.id, keep, keep && text.trim() !== l.text ? text : undefined),
            )
          }
        />
      ))}
      {error && (
        <p className="form-error" role="alert">
          {error}
        </p>
      )}
    </section>
  );
}

/** A role's lessons in its details: what it has learned (Remove), and "Learn on its own". */
export function RoleLessons({
  learning,
  roleId,
  roleName,
}: {
  learning: Learning;
  roleId: string;
  roleName: string;
}) {
  const { pending, error, run } = useRun((s: LearningSnapshot) => learning.apply(s));
  const s = learning.snapshot;
  if (!s || !s.enabled) return null;
  const kept = s.kept.filter((l) => l.roleId === roleId);
  return (
    <div className="role-lessons">
      <h4>What it has learned</h4>
      {kept.length === 0 ? (
        <p className="muted">Nothing yet. Lessons you keep for {roleName} show here.</p>
      ) : (
        <ul className="inspector__list">
          {kept.map((l) => (
            <li key={l.id}>
              {l.text}{" "}
              <button
                type="button"
                className="link"
                disabled={pending}
                onClick={() => void run(() => removeLesson(l.id))}
              >
                Remove
              </button>
            </li>
          ))}
        </ul>
      )}
      <Toggle
        label="Learn on its own"
        hint="On: this role's lessons are kept without asking you (lessons from tasks that used websites or your screen still ask)."
        checked={s.autoRoles.includes(roleId)}
        disabled={pending}
        onChange={(on) => void run(() => setRoleLearning(roleId, on))}
      />
      {error && (
        <p className="form-error" role="alert">
          {error}
        </p>
      )}
    </div>
  );
}
