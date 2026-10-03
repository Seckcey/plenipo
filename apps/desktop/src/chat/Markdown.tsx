import { Fragment, memo, useMemo, useState, type ReactNode } from "react";

import { copyText } from "./clipboard";
import { parseParts, type Block, type Inline, type ListItem } from "./markdown";

interface LinkProps {
  /** What to do when a link is chosen. Without it a link only shows where it goes. */
  onOpenLink?: ((url: string) => void) | undefined;
}

/**
 * An agent's words with their formatting: paragraphs, headings, lists, code, quotes, tables,
 * bold, italic, and links (ADR-200). Drawn with React elements only, never as HTML, so nothing an
 * agent writes can run. Text that is still being written draws as it is: an open code block stays
 * a code block. Only the last block changes while words arrive, so the others are not drawn again.
 */
export function Markdown({ text, onOpenLink }: { text: string } & LinkProps) {
  const parts = useMemo(() => parseParts(text), [text]);
  return (
    <div className="md">
      {parts.map((p, i) => (
        <BlockView key={i} raw={p.raw} block={p.block} onOpenLink={onOpenLink} />
      ))}
    </div>
  );
}

const BlockView = memo(
  function BlockView({ block, onOpenLink }: { raw: string; block: Block } & LinkProps) {
    return <BlockNode block={block} onOpenLink={onOpenLink} />;
  },
  (a, b) => a.raw === b.raw && a.onOpenLink === b.onOpenLink,
);

function BlockNode({ block, onOpenLink }: { block: Block } & LinkProps): ReactNode {
  switch (block.t) {
    case "p":
      return (
        <p>
          <Inlines items={block.c} onOpenLink={onOpenLink} />
        </p>
      );
    case "h":
      // Below the page's own headings, so the page's outline is not disturbed.
      return (
        <div
          className={`md__heading md__heading--${block.level}`}
          role="heading"
          aria-level={Math.min(6, block.level + 3)}
        >
          <Inlines items={block.c} onOpenLink={onOpenLink} />
        </div>
      );
    case "code":
      return <CodeBlock lang={block.lang} value={block.v} open={block.open} />;
    case "ul":
      return (
        <ul>
          <Items items={block.items} onOpenLink={onOpenLink} />
        </ul>
      );
    case "ol":
      return (
        <ol start={block.start}>
          <Items items={block.items} onOpenLink={onOpenLink} />
        </ol>
      );
    case "quote":
      return (
        <blockquote>
          <Blocks blocks={block.c} onOpenLink={onOpenLink} />
        </blockquote>
      );
    case "hr":
      return <hr />;
    case "table":
      return (
        <div className="md__table" role="region" aria-label="Table" tabIndex={0}>
          <table>
            <thead>
              <tr>
                {block.head.map((cell, i) => (
                  <th key={i} style={align(block.align[i])}>
                    <Inlines items={cell} onOpenLink={onOpenLink} />
                  </th>
                ))}
              </tr>
            </thead>
            <tbody>
              {block.rows.map((row, r) => (
                <tr key={r}>
                  {row.map((cell, i) => (
                    <td key={i} style={align(block.align[i])}>
                      <Inlines items={cell} onOpenLink={onOpenLink} />
                    </td>
                  ))}
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      );
  }
}

function align(
  a: "left" | "center" | "right" | null | undefined,
): { textAlign: "left" | "center" | "right" } | undefined {
  return a ? { textAlign: a } : undefined;
}

function Blocks({ blocks, onOpenLink }: { blocks: Block[] } & LinkProps) {
  return (
    <>
      {blocks.map((b, i) => (
        <BlockNode key={i} block={b} onOpenLink={onOpenLink} />
      ))}
    </>
  );
}

function Items({ items, onOpenLink }: { items: ListItem[] } & LinkProps) {
  return (
    <>
      {items.map((item, i) => (
        <li key={i} className={item.task === null ? undefined : "md__task-item"}>
          {item.task !== null && (
            <span
              className={`md__task md__task--${item.task ? "done" : "open"}`}
              role="img"
              aria-label={item.task ? "done" : "not done"}
            >
              {item.task ? "✓" : "○"}
            </span>
          )}
          <Blocks blocks={item.blocks} onOpenLink={onOpenLink} />
        </li>
      ))}
    </>
  );
}

function Inlines({ items, onOpenLink }: { items: Inline[] } & LinkProps) {
  return (
    <>
      {items.map((node, i) => (
        <InlineNode key={i} node={node} onOpenLink={onOpenLink} />
      ))}
    </>
  );
}

function InlineNode({ node, onOpenLink }: { node: Inline } & LinkProps): ReactNode {
  switch (node.t) {
    case "text":
      return <Fragment>{node.v}</Fragment>;
    case "code":
      return <code className="md__code">{node.v}</code>;
    case "strong":
      return (
        <strong>
          <Inlines items={node.c} onOpenLink={onOpenLink} />
        </strong>
      );
    case "em":
      return (
        <em>
          <Inlines items={node.c} onOpenLink={onOpenLink} />
        </em>
      );
    case "del":
      return (
        <del>
          <Inlines items={node.c} onOpenLink={onOpenLink} />
        </del>
      );
    case "br":
      return <br />;
    case "link":
      return (
        <a
          className="md__link"
          href={node.href}
          title={node.href}
          rel="noreferrer noopener"
          // A link never takes over the window: Plenipo decides what choosing it does.
          onClick={(e) => {
            e.preventDefault();
            onOpenLink?.(node.href);
          }}
          onAuxClick={(e) => e.preventDefault()}
        >
          <Inlines items={node.c} onOpenLink={onOpenLink} />
        </a>
      );
  }
}

function CodeBlock({ lang, value, open }: { lang: string; value: string; open: boolean }) {
  const [state, setState] = useState<"idle" | "copied" | "failed">("idle");
  const copy = () => {
    void copyText(value).then((ok) => {
      setState(ok ? "copied" : "failed");
      setTimeout(() => setState("idle"), 1800);
    });
  };
  return (
    <figure className="md__fence">
      <figcaption>
        <span className="md__lang">{lang || "text"}</span>
        {open && <span className="muted"> being written…</span>}
        <button type="button" className="md__copy" onClick={copy} aria-live="polite">
          {state === "copied" ? "Copied" : state === "failed" ? "Could not copy" : "Copy"}
        </button>
      </figcaption>
      <pre tabIndex={0} aria-label={lang ? `${lang} code` : "Code"}>
        <code>{value}</code>
      </pre>
    </figure>
  );
}
