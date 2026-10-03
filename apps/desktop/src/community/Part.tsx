import { useId, type ReactNode } from "react";

/** A part of a Community page, with its title (and a line saying what it is for). */
export function Part({
  title,
  hint,
  children,
}: {
  title: string;
  hint?: string;
  children: ReactNode;
}) {
  const id = useId();
  return (
    <section className="people-part" aria-labelledby={id}>
      <h2 id={id}>{title}</h2>
      {hint && <p className="muted">{hint}</p>}
      {children}
    </section>
  );
}
