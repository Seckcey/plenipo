import {
  createContext,
  useContext,
  useId,
  useRef,
  useState,
  type FormEvent,
  type RefObject,
} from "react";
import type { CardView, Found, MemberView, PeoplePage as Page, ShareProfile } from "@plenipo/types";
import { Button, CountBadge, ErrorState, LoadingState, Select, Tabs } from "@plenipo/ui";

import {
  communityDirectory,
  communityNewThisWeek,
  findInCommunity,
  inviteToCommunity,
  shareMyCommunityProfile,
  toCommandError,
} from "../api/commands";
import type { Go } from "../components/views";
import { PictureCode } from "../remote/DevicesSettings";
import { systemWords } from "../system/words";
import { BlockDialog } from "./BlockDialog";
import { UNBLOCK_IN_SETTINGS, blockedWords } from "./blockReportWords";
import { CommunityCard, type CardActions } from "./CommunityCard";
import type { Target } from "./Conversation";
import { GettingStartedPanel } from "./GettingStartedPanel";
import { Leaderboard } from "./Leaderboard";
import { Messages } from "./Messages";
import { Part } from "./Part";
import { BUSINESS_KINDS, regionOptions } from "./profileWords";
import { ReportDialog } from "./ReportDialog";
import { oneLine } from "./safeText";
import { useCommunity } from "./useCommunity";
import { useGettingStarted } from "./useGettingStarted";
import { unseenCount, useMessages } from "./useMessages";

/** The longest search, as the account service counts it. */
const MOST_SEARCH = 60;

/** Said after every invitation, whether or not the address has an account (ADR-163 §6). */
export const INVITE_SENT =
  "If that address can join Community, 8 West will email it an invitation.";

/** What is said when **Find someone** finds a member who can only be sent a request. */
function requestOnlyWords(name: string): string {
  return `@${oneLine(name)} can only be sent a message request.`;
}

/** Said when **Find someone** finds no one: the same words for every reason (ADR-163 §6). */
export const NO_ONE = "No one in Community has that name.";

/**
 * What the People tab does with a card: opens a conversation (the Messages tab with a box for that
 * person), reports or blocks the person. It knows who you are, so your own card has none of those
 * buttons. Without it (a card on its own), no buttons.
 */
const Messaging = createContext<{
  open: (memberId: string, name: string) => void;
  /** The buttons for a card with this Community name: none for your own. */
  actions: (name: string) => CardActions;
} | null>(null);

/** A page of cards at a time: what is shown, where the next page starts, and what is going on. */
interface Pages {
  cards: CardView[];
  /** Where the next page starts, or `null` when this is the last. */
  next: string | null;
  /** The first page has come. */
  loaded: boolean;
}

const NO_PAGES: Pages = { cards: [], next: null, loaded: false };

/**
 * A list that comes 20 cards at a time (ADR-163 §4). `search` starts over with a way to ask for a
 * page (kept, so "Show more" asks the same question); `more` adds the next page. An answer for a
 * question that was since replaced is dropped, so the list is never a mix of two searches.
 */
function usePages() {
  const [pages, setPages] = useState<Pages>(NO_PAGES);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const turn = useRef(0);
  const ask = useRef<((cursor: string) => Promise<Page>) | null>(null);

  const run = (adding: boolean) => {
    const load = ask.current;
    if (!load || (adding && pages.next === null)) return;
    const mine = ++turn.current;
    setBusy(true);
    setError(null);
    if (!adding) setPages(NO_PAGES);
    load(adding ? (pages.next ?? "") : "")
      .then((page) => {
        if (mine !== turn.current) return;
        setPages((before) => {
          // A card that is on screen already is not added twice.
          const fresh = adding
            ? page.cards.filter((c) => !before.cards.some((b) => b.memberId === c.memberId))
            : page.cards;
          return {
            cards: adding ? [...before.cards, ...fresh] : fresh,
            next: page.next,
            loaded: true,
          };
        });
      })
      .catch((reason: unknown) => {
        if (mine === turn.current) setError(toCommandError(reason).message);
      })
      .finally(() => {
        if (mine === turn.current) setBusy(false);
      });
  };
  return {
    ...pages,
    busy,
    error,
    search: (load: (cursor: string) => Promise<Page>) => {
      ask.current = load;
      run(false);
    },
    more: () => run(true),
  };
}

type PagesState = ReturnType<typeof usePages>;

/** A grid of cards, with "Show more" when there are more, and plain words when there are none. */
function PeopleList({ pages, label, empty }: { pages: PagesState; label: string; empty: string }) {
  const messaging = useContext(Messaging);
  return (
    <>
      {pages.busy && pages.cards.length === 0 && (
        <p className="muted" role="status">
          Looking…
        </p>
      )}
      {pages.loaded && pages.cards.length === 0 && (
        <p role="status" className="people-empty">
          {empty}
        </p>
      )}
      {pages.cards.length > 0 && (
        <ul className="people-grid" aria-label={label}>
          {pages.cards.map((card) => (
            <li key={card.memberId}>
              <CommunityCard card={card} {...messaging?.actions(card.name)} />
            </li>
          ))}
        </ul>
      )}
      {pages.next !== null && (
        <div className="settings-section__actions">
          <Button disabled={pages.busy} onClick={pages.more}>
            Show more
          </Button>
        </div>
      )}
      {pages.error && (
        <p className="form-error" role="alert">
          {pages.error}
        </p>
      )}
    </>
  );
}

/**
 * **Find someone**: an exact Community name. The answer is a card, "can only be sent a message
 * request", or "No one…", and nothing else: it never hints at whether a person exists beyond that
 * (ADR-163 §6). What was typed is sent as it is; Plenipo makes the letters small and drops the "@".
 */
function FindSomeone({
  inputRef,
  onAsked,
}: {
  /** The box, so **Getting started** can put the cursor in it. */
  inputRef: RefObject<HTMLInputElement | null>;
  /** A name was looked up (**Getting started** ticks **Find someone**). */
  onAsked: () => void;
}) {
  const messaging = useContext(Messaging);
  const inputId = useId();
  const [typed, setTyped] = useState("");
  const [found, setFound] = useState<Found | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const turn = useRef(0);
  const name = typed.trim();
  const ready = name.replace(/^@+/, "") !== "" && !busy;

  const change = (next: string) => {
    // An answer is for the name that was typed: another name starts over.
    turn.current += 1;
    setTyped(next);
    setFound(null);
    setError(null);
    setBusy(false);
  };
  const submit = (e: FormEvent) => {
    e.preventDefault();
    if (!ready) return;
    const mine = ++turn.current;
    setBusy(true);
    setFound(null);
    setError(null);
    findInCommunity(name)
      .then((answer) => {
        if (mine === turn.current) setFound(answer);
      })
      .catch((reason: unknown) => {
        if (mine === turn.current) setError(toCommandError(reason).message);
      })
      .finally(() => {
        if (mine === turn.current) setBusy(false);
        onAsked();
      });
  };
  return (
    <Part title="Find someone" hint="Type the exact name a person chose in Community.">
      <form className="people-form" aria-label="Find someone" onSubmit={submit}>
        <div className="ui-field">
          <label htmlFor={inputId}>Their name in Community</label>
          <div className="people-form__row">
            <span className="people-form__at" aria-hidden="true">
              @
            </span>
            <input
              id={inputId}
              ref={inputRef}
              value={typed}
              maxLength={40}
              autoComplete="off"
              spellCheck={false}
              onChange={(e) => change(e.target.value)}
            />
            <Button type="submit" variant="primary" disabled={!ready}>
              Find
            </Button>
          </div>
        </div>
      </form>
      {found?.kind === "card" && (
        <ul className="people-grid people-grid--one" aria-label="Who was found">
          <li>
            <CommunityCard card={found.card} {...messaging?.actions(found.card.name)} />
          </li>
        </ul>
      )}
      {found?.kind === "requestOnly" && (
        <>
          <p role="status">{requestOnlyWords(found.name)}</p>
          {messaging && (
            <div className="settings-section__actions">
              <Button variant="primary" onClick={() => messaging.open(found.memberId, found.name)}>
                Send a message request
              </Button>
            </div>
          )}
        </>
      )}
      {found?.kind === "noOne" && <p role="status">{NO_ONE}</p>}
      {error && (
        <p className="form-error" role="alert">
          {error}
        </p>
      )}
    </Part>
  );
}

/** "Anywhere", then the states and the countries. */
const WHERE: readonly { value: string; label: string; group?: string | undefined }[] = [
  { value: "", label: "Anywhere" },
  ...regionOptions("").filter((o) => o.value !== ""),
];

const KINDS: readonly { value: string; label: string }[] = [
  { value: "", label: "Any kind of business" },
  ...BUSINESS_KINDS,
];

/**
 * **Directory**: search by name, company, or what a business does, by kind of business, and by
 * where. Nothing is asked of the account service until Search is pressed: it lets one account see
 * only so many cards a day (ADR-163 §4), so a look at this page alone uses none.
 */
function Directory() {
  const searchId = useId();
  const [q, setQ] = useState("");
  const [kind, setKind] = useState("");
  const [region, setRegion] = useState("");
  const pages = usePages();
  const submit = (e: FormEvent) => {
    e.preventDefault();
    // What was searched is kept, so "Show more" asks the same question whatever is typed since.
    const [words, ofKind, where] = [q.trim(), kind, region];
    pages.search((cursor) => communityDirectory(words, ofKind, where, cursor));
  };
  return (
    <Part
      title="Directory"
      hint="Press Search to see people. Leave the boxes empty to see everyone."
    >
      <form className="people-form" aria-label="Search the directory" onSubmit={submit}>
        <div className="ui-field">
          <label htmlFor={searchId}>Search by name, company, or what a business does</label>
          <input
            id={searchId}
            type="search"
            value={q}
            maxLength={MOST_SEARCH}
            autoComplete="off"
            onChange={(e) => setQ(e.target.value)}
          />
        </div>
        <div className="people-form__pickers">
          <Select label="Kind of business" value={kind} options={KINDS} onChange={setKind} />
          <Select label="Where" value={region} options={WHERE} onChange={setRegion} />
        </div>
        <div className="settings-section__actions">
          <Button type="submit" variant="primary" icon="search" disabled={pages.busy}>
            Search
          </Button>
        </div>
      </form>
      <PeopleList pages={pages} label="People in the directory" empty="No one matches." />
    </Part>
  );
}

/** **New this week**: the people who joined in the last 7 days. A list of people, not posts. */
function NewThisWeek() {
  const pages = usePages();
  return (
    <Part title="New this week" hint="People who joined Community in the last 7 days.">
      {!pages.loaded && (
        <div className="settings-section__actions">
          <Button disabled={pages.busy} onClick={() => pages.search(communityNewThisWeek)}>
            Show new people
          </Button>
        </div>
      )}
      <PeopleList pages={pages} label="New this week" empty="No one is new this week." />
    </Part>
  );
}

/**
 * **Invite by email**: 8 West emails the address a link to join. Whatever the answer, the words
 * after it are the same, so Plenipo never says whether that address has an account (ADR-163 §6).
 */
function InviteByEmail() {
  const inputId = useId();
  const [email, setEmail] = useState("");
  const [busy, setBusy] = useState(false);
  const [sent, setSent] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const address = email.trim();
  const submit = (e: FormEvent) => {
    e.preventDefault();
    if (address === "" || busy) return;
    setBusy(true);
    setSent(false);
    setError(null);
    inviteToCommunity(address)
      .then(() => {
        setSent(true);
        setEmail("");
      })
      .catch((reason: unknown) => setError(toCommandError(reason).message))
      .finally(() => setBusy(false));
  };
  return (
    <Part title="Invite by email" hint="8 West emails the address a link to join Community.">
      {/* The address is checked by 8 West, not by the box: the answer is the same for all. */}
      <form className="people-form" aria-label="Invite by email" noValidate onSubmit={submit}>
        <div className="ui-field">
          <label htmlFor={inputId}>Their email address</label>
          <div className="people-form__row">
            <input
              id={inputId}
              type="email"
              value={email}
              autoComplete="off"
              spellCheck={false}
              onChange={(e) => {
                setEmail(e.target.value);
                setSent(false);
                setError(null);
              }}
            />
            <Button type="submit" variant="primary" disabled={address === "" || busy}>
              Invite
            </Button>
          </div>
        </div>
      </form>
      {sent && <p role="status">{INVITE_SENT}</p>}
      {error && (
        <p className="form-error" role="alert">
          {error}
        </p>
      )}
    </Part>
  );
}

/**
 * **Share my profile**: a link and a picture code (QR code) to put on a business card or in an
 * email. The page it opens says how to find you in Plenipo, and nothing else about you.
 */
function ShareMyProfile() {
  const [share, setShare] = useState<ShareProfile | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [copied, setCopied] = useState<string | null>(null);
  const show = () => {
    setBusy(true);
    setError(null);
    setCopied(null);
    shareMyCommunityProfile()
      .then(setShare)
      .catch((reason: unknown) => setError(toCommandError(reason).message))
      .finally(() => setBusy(false));
  };
  // What is shown is what is copied.
  const link = share ? oneLine(share.link) : "";
  const copy = () => {
    // Without a clipboard (or if it says no), the link is still there to select and copy.
    const done = () => setCopied("Copied.");
    const failed = () =>
      setCopied("Plenipo couldn't copy it. Select the link and copy it yourself.");
    try {
      navigator.clipboard.writeText(link).then(done, failed);
    } catch {
      failed();
    }
  };
  return (
    <Part
      title="Share my profile"
      hint="Anyone can open this page. It says how to find you in Plenipo, and nothing else about you."
    >
      <div className="settings-section__actions">
        <Button disabled={busy} onClick={show}>
          Share my profile
        </Button>
      </div>
      {share && (
        <div className="people-share">
          <div className="people-share__text">
            <p>
              <code className="people-share__link">{link}</code>
            </p>
            <div className="settings-section__actions">
              <Button icon="link" onClick={copy}>
                Copy link
              </Button>
            </div>
            {copied && (
              <p role="status" className="muted">
                {copied}
              </p>
            )}
          </div>
          <PictureCode
            qr={{ size: share.qrSize, cells: share.qrCells }}
            label="Picture code for your profile link"
          />
        </div>
      )}
      {error && (
        <p className="form-error" role="alert">
          {error}
        </p>
      )}
    </Part>
  );
}

/** The parts of the Community section. */
type CommunityTab = "people" | "messages" | "leaderboard";

/** The People tab: **Getting started** (for you only), finding people, and being found. */
function People({
  go,
  started,
  onMessages,
}: {
  go: Go;
  started: ReturnType<typeof useGettingStarted>;
  /** Open the Messages tab. */
  onMessages: () => void;
}) {
  const findBox = useRef<HTMLInputElement>(null);
  const close = () => {
    void started.close().then((closed) => {
      // The button that was pressed is gone: the cursor goes back to the tabs.
      if (closed) document.getElementById("community-tab-people")?.focus();
    });
  };
  return (
    <div className="people">
      <GettingStartedPanel
        state={started.state}
        error={started.error}
        onProfile={() => go({ view: "settings", id: "community" })}
        onFind={() => findBox.current?.focus()}
        onMessage={onMessages}
        onClose={close}
      />
      <FindSomeone inputRef={findBox} onAsked={started.reload} />
      <Directory />
      <NewThisWeek />
      <InviteByEmail />
      <ShareMyProfile />
    </div>
  );
}

/**
 * The tabs, once you are signed in. Messages is read here, so its unseen count is on the tab
 * whichever tab is open. The People tab stays as it is while Messages is open (its search is
 * kept); Messages is there only while it is open, so a conversation is read, and marked seen,
 * only when you are looking at it. The Leaderboard is there only while it is open too, so it asks
 * 8 West for the points and the board once each time it is shown (never on a timer).
 */
function SignedIn({ go, member }: { go: Go; member: MemberView | null }) {
  const [tab, setTab] = useState<CommunityTab>("people");
  const [target, setTarget] = useState<Target | null>(null);
  // A card's Report or Block that is being asked about, and what was said after a Block.
  const [asking, setAsking] = useState<{
    kind: "report" | "block";
    memberId: string;
    name: string;
  } | null>(null);
  const [blockedNote, setBlockedNote] = useState<string | null>(null);
  const messages = useMessages();
  // Getting started is read when the People tab shows, and when messages change (one was sent).
  const started = useGettingStarted(tab === "people", messages.changes);
  const unseen = unseenCount(messages.list);
  const own = member?.name ?? "";
  // A card, or a request-only name: the Messages tab opens a box for that person, even when
  // there is no conversation yet.
  const open = (memberId: string, name: string) => {
    setTarget({ memberId, name });
    setTab("messages");
  };
  const messaging = {
    open,
    actions: (name: string): CardActions =>
      name === own
        ? {}
        : {
            onMessage: open,
            onReport: (memberId, who) => {
              setBlockedNote(null);
              setAsking({ kind: "report", memberId, name: who });
            },
            onBlock: (memberId, who) => {
              setBlockedNote(null);
              setAsking({ kind: "block", memberId, name: who });
            },
          },
  };
  return (
    <>
      <Tabs<CommunityTab>
        label="Community sections"
        idPrefix="community"
        value={tab}
        onChange={setTab}
        tabs={[
          { value: "people", label: "People" },
          {
            value: "messages",
            label: "Messages",
            badge: <CountBadge count={unseen} label="unseen" />,
          },
          { value: "leaderboard", label: "Leaderboard" },
        ]}
      />
      {blockedNote && (
        <p role="status" className="muted">
          {blockedNote}
        </p>
      )}
      <div
        role="tabpanel"
        id="community-panel-people"
        aria-labelledby="community-tab-people"
        hidden={tab !== "people"}
      >
        <Messaging.Provider value={messaging}>
          <People go={go} started={started} onMessages={() => setTab("messages")} />
        </Messaging.Provider>
      </div>
      {tab === "messages" && (
        <div role="tabpanel" id="community-panel-messages" aria-labelledby="community-tab-messages">
          <Messages
            messages={messages}
            target={target}
            onTarget={setTarget}
            adult={member?.adult !== false}
            go={go}
          />
        </div>
      )}
      {tab === "leaderboard" && (
        <div
          role="tabpanel"
          id="community-panel-leaderboard"
          aria-labelledby="community-tab-leaderboard"
        >
          <Leaderboard adult={member?.adult !== false} own={own} onMessage={open} />
        </div>
      )}
      {asking?.kind === "report" && (
        <ReportDialog
          memberId={asking.memberId}
          name={asking.name}
          about={["person", "profile"]}
          onSent={(alsoBlocked) => {
            if (alsoBlocked) setBlockedNote(`${blockedWords(asking.name)} ${UNBLOCK_IN_SETTINGS}`);
          }}
          onClose={() => setAsking(null)}
        />
      )}
      {asking?.kind === "block" && (
        <BlockDialog
          memberId={asking.memberId}
          name={asking.name}
          onCancel={() => setAsking(null)}
          onBlocked={() => {
            setBlockedNote(`${blockedWords(asking.name)} ${UNBLOCK_IN_SETTINGS}`);
            setAsking(null);
          }}
        />
      )}
    </>
  );
}

/**
 * The Community section on the strip (Phase 24, ADR-163): find people who use Plenipo. It is
 * there only while Community's switch is on. It works once you are signed in; before that it
 * says so, with a way to Settings → Community.
 */
export function PeoplePage({ go }: { go: Go }) {
  const { view, error, reload } = useCommunity();
  return (
    <section className="view community-view" aria-labelledby="community-title">
      <h1 id="community-title">Community</h1>
      <p className="view__lead">Find people who use Plenipo, and let them find you.</p>
      {!view ? (
        error ? (
          <ErrorState title="Couldn't load Community" message={error} onRetry={reload} />
        ) : (
          <LoadingState label="Loading Community" />
        )
      ) : view.stage !== "signedIn" ? (
        <div className="community-view__note">
          <p className="notice-box" role="note">
            You are not signed in to Community on {systemWords().thisComputer}. Open Settings →
            Community to sign in.
          </p>
          <div className="settings-section__actions">
            <Button variant="primary" onClick={() => go({ view: "settings", id: "community" })}>
              Go to Settings → Community
            </Button>
          </div>
        </div>
      ) : (
        <SignedIn go={go} member={view.member} />
      )}
    </section>
  );
}
