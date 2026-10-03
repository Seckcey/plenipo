import { Fragment, useMemo } from "react";

import { pieces } from "./safeText";

/**
 * The parts of a message's words, shown safely (ADR-164 §4), to go inside a box of their own. They
 * are another person's words, so they are only ever text: never a web page, a picture, or a
 * button that does something by itself.
 *
 * - a character a person cannot see (a right-to-left mark, a zero-width space) is a visible mark
 *   with its name, so nothing hides;
 * - a web address is text. With `onOpenLink`, a small **Open link** sits beside it; pressing it
 *   only asks `onOpenLink`, which says "Open this link in your web browser?" before anything
 *   opens. Without it (a list to tick messages in), the address is plain text and nothing more.
 */
export function MessageParts({
  text,
  onOpenLink,
}: {
  text: string;
  onOpenLink?: ((address: string) => void) | undefined;
}) {
  const parts = useMemo(() => pieces(text), [text]);
  return (
    <>
      {parts.map((part, i) => {
        if (part.kind === "text") return <Fragment key={i}>{part.text}</Fragment>;
        if (part.kind === "hidden") {
          return (
            <span key={i} className="message__mark" title={`${part.name} (${part.code})`}>
              {`‹${part.name}›`}
            </span>
          );
        }
        if (!onOpenLink) {
          return (
            <span key={i} className="message__address">
              {part.text}
            </span>
          );
        }
        return (
          <span key={i} className="message__link">
            <span className="message__address">{part.text}</span>{" "}
            <button type="button" className="message__open" onClick={() => onOpenLink(part.url)}>
              Open link
            </button>
          </span>
        );
      })}
    </>
  );
}

/**
 * The words of a message, shown safely (see `MessageParts`), as a paragraph of their own. Line
 * breaks stay as the person wrote them (the style keeps them).
 */
export function MessageText({
  text,
  onOpenLink,
}: {
  text: string;
  onOpenLink: (address: string) => void;
}) {
  return (
    <p className="message__text">
      <MessageParts text={text} onOpenLink={onOpenLink} />
    </p>
  );
}
