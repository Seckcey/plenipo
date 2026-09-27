/**
 * The Gallery: every component in the library, in every state, in either theme or both side
 * by side (Phase 12A). The desktop app shows it from Diagnostics, with real data from the
 * Ledger in the first section.
 */

import { useMemo, useState, type ReactNode } from "react";

import {
  CardGrid,
  EntityCard,
  EntityCardSkeleton,
  type CollectionView,
  type EntityCardProps,
} from "../cards";
import {
  Button,
  Checkbox,
  IconButton,
  SearchField,
  Segmented,
  Select,
  Switch,
  Tabs,
  TextField,
} from "../controls";
import { DetailSplitView, PropertyList, TimelineScrubber, type TimelineValue } from "../detail";
import { useFacets, type FacetConfig } from "../facet-logic";
import { FacetPanel } from "../facets";
import { ICON_NAMES } from "../icon-data";
import { Icon } from "../icons";
import { Banner } from "../shell";
import { EmptyState, ErrorState, LoadingState } from "../states";
import {
  ActivityStrip,
  ActivityStripPlaceholder,
  CountBadge,
  HealthBar,
  Sparkline,
  StatusDot,
  StatusPill,
} from "../status";
import { STATUSES } from "../status-types";
import { CellLink, DataTable } from "../table";
import { statusColumn, type Column } from "../table-columns";
import {
  COLOR_ROLES,
  FONT_SIZE,
  RADIUS,
  SPACE,
  colorVar,
  type ColorToken,
  type ThemeName,
} from "../tokens";
import { TopologyMap } from "../topology";
import { DAY_MS } from "../activity";
import {
  sampleActivity,
  sampleCards,
  sampleMap,
  sampleTimeline,
  sampleWorkers,
  STATUS_WORDS,
  type SampleWorker,
} from "./fixtures";

/** Real data for the first section, from the app (the Ledger). */
export interface GalleryLive {
  state: "loading" | "error" | "ready";
  error?: string;
  cards: EntityCardProps[];
}

function Section({
  id,
  title,
  children,
  lead,
}: {
  id: string;
  title: string;
  lead?: string;
  children: ReactNode;
}) {
  return (
    <section className="gallery__section" aria-labelledby={id} data-gallery-section={id}>
      <h2 id={id}>{title}</h2>
      {lead && <p className="gallery__lead">{lead}</p>}
      {children}
    </section>
  );
}

function Variant({
  name,
  children,
  wide = false,
}: {
  name: string;
  children: ReactNode;
  wide?: boolean;
}) {
  return (
    <figure
      className={wide ? "gallery__variant gallery__variant--wide" : "gallery__variant"}
      data-gallery={name}
    >
      <figcaption>{name}</figcaption>
      <div className="gallery__sample">{children}</div>
    </figure>
  );
}

const TABLE_ROWS = 5000;
const GRID_CARDS = 150;

function workerColumns(onOpenProject: (name: string) => void): Column<SampleWorker>[] {
  return [
    { id: "name", header: "Name", cell: (w) => w.name, sortValue: (w) => w.name, width: "220px" },
    statusColumn((w) => ({ status: w.status, label: STATUS_WORDS[w.status] })),
    { id: "tool", header: "AI tool", cell: (w) => w.tool, sortValue: (w) => w.tool },
    {
      id: "project",
      header: "Project",
      cell: (w) => <CellLink onClick={() => onOpenProject(w.project)}>{w.project}</CellLink>,
      sortValue: (w) => w.project,
    },
    {
      id: "department",
      header: "Department",
      cell: (w) => w.department,
      sortValue: (w) => w.department,
    },
    {
      id: "tasks",
      header: "Tasks",
      numeric: true,
      cell: (w) => w.tasks,
      sortValue: (w) => w.tasks,
    },
    {
      id: "minutes",
      header: "Minutes",
      numeric: true,
      cell: (w) => w.minutes.toFixed(1),
      sortValue: (w) => w.minutes,
    },
    {
      id: "cost",
      header: "Usage ($)",
      numeric: true,
      cell: (w) => w.cost.toFixed(2),
      sortValue: (w) => w.cost,
      hidden: true,
    },
    {
      id: "seen",
      header: "Last seen",
      numeric: true,
      cell: (w) =>
        w.hoursAgo < 24 ? `${w.hoursAgo} h ago` : `${Math.round(w.hoursAgo / 24)} d ago`,
      sortValue: (w) => w.hoursAgo,
    },
  ];
}

const facetConfig: FacetConfig<SampleWorker> = {
  search: (w) => `${w.name} ${w.project} ${w.tool}`,
  groups: [
    { id: "status", label: "Status", value: (w) => w.status, labels: STATUS_WORDS },
    { id: "tool", label: "AI tool", value: (w) => w.tool },
    { id: "department", label: "Department", value: (w) => w.department },
  ],
  ranges: [
    {
      id: "seen",
      label: "Last seen",
      min: 0,
      max: 2160,
      step: 24,
      value: (w) => w.hoursAgo,
      format: (h) => (h === 0 ? "Now" : h < 48 ? `${h} h` : `${Math.round(h / 24)} d`),
      marks: [
        { value: 0, label: "Now" },
        { value: 168, label: "1W" },
        { value: 720, label: "1M" },
        { value: 2160, label: "3M" },
      ],
    },
  ],
};

function FilteredTable({ workers, prefix }: { workers: SampleWorker[]; prefix: string }) {
  const facets = useFacets(workers, facetConfig);
  const [collapsed, setCollapsed] = useState(false);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [opened, setOpened] = useState<string | null>(null);
  const columns = useMemo(() => workerColumns(setOpened), []);
  return (
    <div className="gallery__filtered">
      <FacetPanel
        state={facets.state}
        onChange={facets.setState}
        groups={facets.groups}
        ranges={facets.ranges}
        collapsed={collapsed}
        onCollapsedChange={setCollapsed}
        total={workers.length}
        shown={facets.filtered.length}
        searchPlaceholder="Search workers"
      />
      <div className="gallery__filtered-main">
        {opened && (
          <p className="gallery__note" role="status">
            Opened project {opened}
          </p>
        )}
        <DataTable
          label="Workers"
          rows={facets.filtered}
          columns={columns}
          getRowId={(w) => w.id}
          selectable
          selected={selected}
          onSelectedChange={setSelected}
          defaultPageSize="all"
          storageKey={`plenipo.gallery.${prefix}.table`}
          height={420}
        />
      </div>
    </div>
  );
}

function workerCard(w: SampleWorker, now: number): EntityCardProps {
  return {
    title: w.name,
    status: w.status,
    statusLabel: STATUS_WORDS[w.status],
    subtype: `${w.project} · ${w.department}`,
    activity: sampleActivity(Number(w.id.slice(1)), now, 0.5),
    owner: { icon: "aiTools", label: w.tool },
    resources: [
      { icon: "file", label: "Read and change files" },
      { icon: "terminal", label: "Run programs" },
    ],
  };
}

function Grid({ workers, now }: { workers: SampleWorker[]; now: number }) {
  const [view, setView] = useState<CollectionView>("cards");
  const [collapsed, setCollapsed] = useState(false);
  const facets = useFacets(workers, facetConfig);
  const cards = useMemo(
    () => new Map(workers.map((w) => [w.id, workerCard(w, now)])),
    [workers, now],
  );
  return (
    <div className="gallery__filtered">
      <FacetPanel
        state={facets.state}
        onChange={facets.setState}
        groups={facets.groups}
        collapsed={collapsed}
        onCollapsedChange={setCollapsed}
        total={workers.length}
        shown={facets.filtered.length}
        searchPlaceholder="Search cards"
      />
      <div className="gallery__filtered-main">
        <CardGrid
          label="Worker cards"
          items={facets.filtered}
          getKey={(w) => w.id}
          view={view}
          onViewChange={setView}
          height={400}
          renderCard={(w) => (
            <EntityCard {...(cards.get(w.id) ?? workerCard(w, now))} onOpen={() => undefined} />
          )}
          renderRow={(w) => (
            <>
              <StatusDot status={w.status} label={STATUS_WORDS[w.status]} />
              <strong>{w.name}</strong>
              <span className="gallery__muted">{w.project}</span>
              <span className="gallery__muted">{w.tool}</span>
            </>
          )}
        />
      </div>
    </div>
  );
}

function SplitSample({ now, prefix }: { now: number; prefix: string }) {
  const [value, setValue] = useState<TimelineValue>("live");
  const [live, setLive] = useState(true);
  const [map, setMap] = useState(true);
  const [selected, setSelected] = useState<string | null>("web");
  const { nodes, links } = useMemo(() => sampleMap(), []);
  const events = useMemo(() => sampleTimeline(now), [now]);
  const rows = useMemo(() => sampleWorkers(12, 3), []);
  const columns = useMemo(() => workerColumns(() => undefined).slice(0, 6), []);
  return (
    <DetailSplitView
      label="Website Supervisor"
      properties={
        <PropertyList
          title={<StatusDot status="pending" label="Website Supervisor · Waiting for you" />}
          items={[
            { label: "AI tool", value: "Codex" },
            { label: "Model", value: "Automatic" },
            { label: "Project", value: "Website" },
            {
              label: "Live updates",
              value: <Switch label="Live updates" checked={live} onChange={setLive} />,
            },
            {
              label: "Show the map",
              value: <Switch label="Show the map" checked={map} onChange={setMap} />,
            },
          ]}
        />
      }
      timeline={
        <TimelineScrubber
          from={now - DAY_MS}
          to={now}
          events={events}
          value={value}
          onChange={setValue}
          label="Website Supervisor's timeline"
        />
      }
      map={
        map ? (
          <TopologyMap
            label="Delegation tree"
            nodes={nodes}
            links={links}
            selectedId={selected}
            onSelect={setSelected}
          />
        ) : (
          <EmptyState compact title="The map is hidden" />
        )
      }
      table={
        <DataTable
          label={`Team at ${value === "live" ? "now" : "the chosen time"}`}
          rows={rows}
          columns={columns}
          getRowId={(w) => w.id}
          defaultPageSize={25}
          storageKey={`plenipo.gallery.${prefix}.split`}
          height={260}
        />
      }
    />
  );
}

function Swatch({ token }: { token: ColorToken }) {
  return (
    <div className="gallery__swatch" data-gallery={`color-${token}`}>
      <span className="gallery__chip" style={{ background: colorVar(token) }} />
      <span>
        <code>{token}</code>
        <span className="gallery__muted">{COLOR_ROLES[token]}</span>
      </span>
    </div>
  );
}

function GalleryBody({ now, live, prefix }: { now: number; live?: GalleryLive; prefix: string }) {
  const [on, setOn] = useState(true);
  const [checked, setChecked] = useState(true);
  const [search, setSearch] = useState("");
  const [text, setText] = useState("Website");
  const [choice, setChoice] = useState<"a" | "b">("a");
  const [tab, setTab] = useState<"one" | "two" | "three">("one");
  const [view, setView] = useState<CollectionView>("cards");
  const [dismissed, setDismissed] = useState(false);
  const cards = useMemo(() => sampleCards(now), [now]);
  const workers = useMemo(() => sampleWorkers(TABLE_ROWS), []);
  const gridWorkers = useMemo(() => workers.slice(0, GRID_CARDS), [workers]);
  const { nodes, links } = useMemo(() => sampleMap(), []);
  const activity = useMemo(() => sampleActivity(5, now, 1.2), [now]);
  const spark = useMemo(() => activity.buckets.map((b) => b.events), [activity]);
  const smallColumns = useMemo(() => workerColumns(() => undefined).slice(0, 4), []);

  return (
    <div className="gallery__body">
      {live && (
        <Section
          id={`${prefix}-live`}
          title="From your organization"
          lead="Real departments and projects, with activity from the Ledger."
        >
          {live.state === "loading" ? (
            <div className="gallery__cards">
              <EntityCardSkeleton />
              <EntityCardSkeleton />
            </div>
          ) : live.state === "error" ? (
            <ErrorState title="Couldn't read the organization" message={live.error} />
          ) : live.cards.length === 0 ? (
            <EmptyState title="No departments or projects yet" icon="organization">
              Add them on the Organization page.
            </EmptyState>
          ) : (
            <div className="gallery__cards" data-gallery="live-cards">
              {live.cards.map((c) => (
                <EntityCard key={c.title} {...c} now={now} />
              ))}
            </div>
          )}
        </Section>
      )}

      <Section
        id={`${prefix}-status`}
        title="Status"
        lead="A mark and a word, never color alone. Each status has its own mark shape."
      >
        <div className="gallery__row">
          <Variant name="status-dot">
            {STATUSES.map((s) => (
              <StatusDot key={s} status={s} label={STATUS_WORDS[s]} />
            ))}
          </Variant>
          <Variant name="status-pill">
            {STATUSES.map((s) => (
              <StatusPill key={s} status={s} label={STATUS_WORDS[s]} />
            ))}
          </Variant>
          <Variant name="count-badge">
            <CountBadge count={3} label="waiting for you" />
            <CountBadge count={12} label="working" tone="ok" />
            <CountBadge count={2} label="blocked" tone="warn" />
            <CountBadge count={1} label="failed" tone="error" />
            <CountBadge count={0} label="waiting" showZero tone="offline" />
          </Variant>
        </div>
        <div className="gallery__row">
          <Variant name="health-bar">
            <HealthBar label="Tasks done" value={7} max={10} valueText="7 of 10" />
            <HealthBar label="Usage limit" value={82} max={100} status="warn" />
            <HealthBar label="Checks passing" value={2} max={9} valueText="2 of 9" status="error" />
          </Variant>
          <Variant name="sparkline">
            <Sparkline values={spark} label="Events per 15 minutes" />
            <Sparkline values={spark.slice(40)} label="Events" status="ok" />
            <Sparkline values={[]} label="Events" />
          </Variant>
        </div>
        <div className="gallery__row">
          <Variant name="activity-strip" wide>
            <ActivityStrip series={activity} now={now} />
          </Variant>
          <Variant name="activity-strip-states" wide>
            <ActivityStripPlaceholder state="empty" />
            <ActivityStripPlaceholder state="loading" />
            <ActivityStripPlaceholder state="error" />
          </Variant>
        </div>
      </Section>

      <Section
        id={`${prefix}-cards`}
        title="Entity cards"
        lead="A department, project, or worker: status, 24-hour activity, who fills it, and its permissions."
      >
        <div className="gallery__cards">
          {cards.map((c) => (
            <div key={c.title} data-gallery={`card-${c.status}`}>
              <EntityCard
                {...c}
                now={now}
                onOpen={() => undefined}
                selected={c.title === "Website"}
              />
            </div>
          ))}
          <div data-gallery="card-loading-activity">
            <EntityCard
              title="Operations"
              status="ok"
              statusLabel="Working"
              subtype="Department"
              activity="loading"
            />
          </div>
          <div data-gallery="card-error-activity">
            <EntityCard
              title="Marketing"
              status="warn"
              statusLabel="Blocked"
              subtype="Department"
              activity="error"
            />
          </div>
          <div data-gallery="card-skeleton">
            <EntityCardSkeleton />
          </div>
        </div>
      </Section>

      <Section
        id={`${prefix}-grid`}
        title="Card grid"
        lead={`${GRID_CARDS} cards with filters, as many columns as fit, and a Cards/List switch.`}
      >
        <Grid workers={gridWorkers} now={now} />
        <div className="gallery__row">
          <Variant name="grid-loading" wide>
            <CardGrid
              label="Loading cards"
              items={[]}
              getKey={() => ""}
              renderCard={() => null}
              renderRow={() => null}
              view={view}
              onViewChange={setView}
              state="loading"
            />
          </Variant>
          <Variant name="grid-error" wide>
            <CardGrid
              label="Cards that failed"
              items={[]}
              getKey={() => ""}
              renderCard={() => null}
              renderRow={() => null}
              view="cards"
              onViewChange={() => undefined}
              state="error"
              error="The Ledger did not answer."
              onRetry={() => undefined}
            />
          </Variant>
          <Variant name="grid-empty" wide>
            <CardGrid
              label="No cards"
              items={[]}
              getKey={() => ""}
              renderCard={() => null}
              renderRow={() => null}
              view="cards"
              onViewChange={() => undefined}
            />
          </Variant>
        </div>
      </Section>

      <Section
        id={`${prefix}-table`}
        title="Table and filters"
        lead={`${TABLE_ROWS.toLocaleString("en-US")} rows: sort, filter, pick columns, select rows. Only the rows on screen are drawn.`}
      >
        <FilteredTable workers={workers} prefix={prefix} />
        <div className="gallery__row">
          <Variant name="table-loading" wide>
            <DataTable
              label="Loading table"
              rows={[]}
              columns={smallColumns}
              getRowId={(w) => w.id}
              state="loading"
              height={160}
            />
          </Variant>
          <Variant name="table-error" wide>
            <DataTable
              label="Table that failed"
              rows={[]}
              columns={smallColumns}
              getRowId={(w) => w.id}
              state="error"
              error="The Ledger did not answer."
              onRetry={() => undefined}
            />
          </Variant>
          <Variant name="table-empty" wide>
            <DataTable
              label="Empty table"
              rows={[]}
              columns={smallColumns}
              getRowId={(w) => w.id}
              height={160}
            />
          </Variant>
        </div>
      </Section>

      <Section
        id={`${prefix}-split`}
        title="Detail split view"
        lead="Properties and switches, a timeline you can scrub, the map, and a table below."
      >
        <SplitSample now={now} prefix={prefix} />
      </Section>

      <Section
        id={`${prefix}-map`}
        title="Relationship map"
        lead="Who handed work to whom: tiles in their status color, labeled lines, a caption under each."
      >
        <Variant name="map" wide>
          <TopologyMap label="Handoff chain" nodes={nodes} links={links} />
        </Variant>
        <div className="gallery__row">
          <Variant name="map-empty">
            <TopologyMap label="Empty map" nodes={[]} links={[]} />
          </Variant>
          <Variant name="map-loading">
            <TopologyMap label="Map" nodes={[]} links={[]} state="loading" />
          </Variant>
          <Variant name="map-error">
            <TopologyMap
              label="Map"
              nodes={[]}
              links={[]}
              state="error"
              error="The Ledger did not answer."
            />
          </Variant>
        </div>
      </Section>

      <Section
        id={`${prefix}-notices`}
        title="Notices"
        lead="Advisories and things you must do, each with its button and Dismiss."
      >
        <div className="gallery__stack">
          <Banner
            tone="info"
            title="A new version of Plenipo is ready"
            action={
              <Button size="sm" variant="primary">
                Restart
              </Button>
            }
            onDismiss={() => undefined}
          />
          <Banner
            tone="pending"
            title="Senior Developer is waiting for your approval"
            action={
              <Button size="sm" variant="primary">
                Review
              </Button>
            }
          >
            git push origin main
          </Banner>
          <Banner
            tone="warn"
            title="Claude Code reached its usage limit"
            onDismiss={() => undefined}
          >
            Workers that use it wait until the limit resets at 3:15 PM.
          </Banner>
          {!dismissed && (
            <Banner
              tone="error"
              title="The Ledger failed its integrity check"
              role="status"
              onDismiss={() => setDismissed(true)}
            >
              Plenipo made a backup before opening it.
            </Banner>
          )}
          <Banner tone="ok" title="All checks passed" />
        </div>
      </Section>

      <Section id={`${prefix}-controls`} title="Controls">
        <div className="gallery__row">
          <Variant name="buttons">
            <Button variant="primary">Give objective</Button>
            <Button>Cancel</Button>
            <Button variant="quiet" icon="refresh">
              Refresh
            </Button>
            <Button variant="danger">Stop all</Button>
            <Button size="sm" variant="primary">
              Small
            </Button>
            <Button disabled>Disabled</Button>
            <IconButton icon="more" label="More actions" />
          </Variant>
          <Variant name="choices">
            <Switch label="Remote computers (SSH)" checked={on} onChange={setOn} />
            <Checkbox label="Online" count={7} checked={checked} onChange={setChecked} />
            <Segmented
              label="Choice"
              value={choice}
              onChange={setChoice}
              options={[
                { value: "a", label: "Cards" },
                { value: "b", label: "List" },
              ]}
            />
          </Variant>
          <Variant name="fields">
            <SearchField value={search} onChange={setSearch} placeholder="Search workers" />
            <TextField
              label="Project name"
              value={text}
              onChange={setText}
              hint="Shown on every page."
            />
            <Select
              label="When to ask you"
              value="changes"
              onChange={() => undefined}
              options={[
                { value: "every", label: "Every command" },
                { value: "changes", label: "Anything that changes" },
                { value: "allowed", label: "Only as allowed" },
              ]}
            />
          </Variant>
        </div>
        <Variant name="tabs" wide>
          <Tabs
            label="Sample tabs"
            value={tab}
            onChange={setTab}
            idPrefix={`${prefix}-tabs`}
            tabs={[
              { value: "one", label: "Your terminal" },
              {
                value: "two",
                label: "Senior Developer",
                badge: <CountBadge count={2} label="new lines" />,
              },
              { value: "three", label: "Shop server" },
            ]}
          />
        </Variant>
      </Section>

      <Section id={`${prefix}-states`} title="Empty, loading, and error">
        <div className="gallery__row">
          <Variant name="empty">
            <EmptyState
              title="No workers yet"
              action={
                <Button size="sm" variant="primary">
                  Hire a worker
                </Button>
              }
            >
              Workers appear here when a Supervisor brings them in.
            </EmptyState>
          </Variant>
          <Variant name="loading">
            <LoadingState label="Loading workers" />
          </Variant>
          <Variant name="error">
            <ErrorState
              title="Couldn't load workers"
              message="The Ledger did not answer."
              onRetry={() => undefined}
            />
          </Variant>
        </div>
      </Section>

      <Section
        id={`${prefix}-tokens`}
        title="Colors, type, and spacing"
        lead="The design tokens. Every color on every screen comes from this list."
      >
        <div className="gallery__swatches">
          {(Object.keys(COLOR_ROLES) as ColorToken[]).map((t) => (
            <Swatch key={t} token={t} />
          ))}
        </div>
        <div className="gallery__row">
          <Variant name="type-scale">
            {Object.entries(FONT_SIZE).map(([k, v]) => (
              <div key={k} style={{ fontSize: `var(--ui-font-${k})` }}>
                {k} · {v}px · <span className="ui-num">1,234.50</span>
              </div>
            ))}
          </Variant>
          <Variant name="spacing">
            {Object.entries(SPACE).map(([k, v]) => (
              <div key={k} className="gallery__space">
                <span className="gallery__bar" style={{ width: `var(--ui-space-${k})` }} />
                <span className="ui-num">
                  space-{k} · {v}px
                </span>
              </div>
            ))}
          </Variant>
          <Variant name="radius">
            {Object.entries(RADIUS).map(([k, v]) => (
              <span
                key={k}
                className="gallery__radius"
                style={{ borderRadius: `var(--ui-radius-${k})` }}
              >
                {k} {v === 999 ? "" : `${v}px`}
              </span>
            ))}
          </Variant>
          <Variant name="icons">
            <div className="gallery__icons">
              {ICON_NAMES.map((n) => (
                <span key={n} title={n}>
                  <Icon name={n} label={n} />
                </span>
              ))}
            </div>
          </Variant>
        </div>
      </Section>
    </div>
  );
}

export type GalleryLayout = "one" | "both";

export function Gallery({
  now: fixedNow,
  live,
  theme,
}: {
  /** The moment the samples end at (tests fix it; the app leaves it to the clock). */
  now?: number;
  live?: GalleryLive;
  /** The app's current theme (the one-theme layout follows it). */
  theme: ThemeName;
}) {
  const [layout, setLayout] = useState<GalleryLayout>("one");
  const [openedAt] = useState(() => Date.now());
  const now = fixedNow ?? openedAt;
  return (
    <div className="gallery">
      <div className="gallery__toolbar">
        <p className="gallery__lead">
          Every building block, in every state. Switch light and dark in the top bar, or show both.
        </p>
        <Segmented
          label="Themes"
          value={layout}
          onChange={setLayout}
          options={[
            { value: "one", label: theme === "dark" ? "Dark" : "Light" },
            { value: "both", label: "Both side by side" },
          ]}
        />
      </div>
      {layout === "one" ? (
        <GalleryBody now={now} prefix="g" {...(live ? { live } : {})} />
      ) : (
        <div className="gallery__both">
          <div data-theme="dark" className="gallery__pane" aria-label="Dark theme" role="group">
            <GalleryBody now={now} prefix="gd" {...(live ? { live } : {})} />
          </div>
          <div data-theme="light" className="gallery__pane" aria-label="Light theme" role="group">
            <GalleryBody now={now} prefix="gl" {...(live ? { live } : {})} />
          </div>
        </div>
      )}
    </div>
  );
}
