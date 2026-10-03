// Samples for Community (Phase 24): what the screens get from the app.

import type {
  CardView,
  CommunityView,
  ConversationSummary,
  ConversationView,
  GettingStarted,
  LeaderboardView,
  LeaderView,
  MessageView,
  PeoplePage,
  PointsView,
  ProfileDraft,
} from "@plenipo/types";

/** A profile with every box ticked and nothing typed, as the app starts it (ADR-163 §2). */
export function profileDraft(patch: Partial<ProfileDraft> = {}): ProfileDraft {
  return {
    displayName: "",
    company: "",
    businessKinds: [],
    businessLine: "",
    region: "",
    shown: {
      picture: true,
      name: true,
      status: true,
      mood: true,
      message: true,
      company: true,
      business: true,
      region: true,
    },
    ...patch,
  };
}

/** Community as the app gives it: off to begin with, and nothing about anyone yet. */
export function communityView(patch: Partial<CommunityView> = {}): CommunityView {
  return {
    stage: "off",
    switchedOn: false,
    comingSoon: false,
    code: null,
    codeExpiresAt: null,
    accountName: null,
    terms: null,
    member: null,
    pro: false,
    linksOpen: false,
    collaboratorsOpen: false,
    problem: null,
    profile: profileDraft(),
    ...patch,
  };
}

/** A member's card, as the app gives it: a name, available, and nothing else yet. */
export function cardView(patch: Partial<CardView> = {}): CardView {
  return {
    memberId: "cm_01J9Z8Y7X6W5V4T3S2R1Q0P9N8",
    name: "pat-lee",
    displayName: "Pat Lee",
    status: "available",
    mood: null,
    message: null,
    company: null,
    businessKinds: [],
    businessLine: null,
    region: null,
    hasPicture: false,
    pictureVersion: null,
    badges: [],
    points: 0,
    thankedBy: 0,
    ...patch,
  };
}

/** A page of cards, with where the next one starts (`null`: this is the last). */
export function peoplePage(cards: CardView[], next: string | null = null): PeoplePage {
  return { cards, next };
}

/** One line of the Messages list: a conversation with Pat Lee, accepted, nothing new. */
export function conversationSummary(patch: Partial<ConversationSummary> = {}): ConversationSummary {
  return {
    memberId: "cm_01J9Z8Y7X6W5V4T3S2R1Q0P9N8",
    name: "pat-lee",
    displayName: "Pat Lee",
    state: "accepted",
    unseen: 0,
    lastAt: 1_790_000_000,
    computersChanged: false,
    ...patch,
  };
}

/** A message from Pat, received, with nothing on it. */
export function messageView(patch: Partial<MessageView> = {}): MessageView {
  return {
    itemId: "ci_01J9Z8Y7X6W5V4T3S2R1Q0P9N1",
    outgoing: false,
    text: "Hello",
    hasGif: false,
    hasSticker: false,
    replyTo: null,
    sentAt: 1_790_000_000,
    acceptedAt: 1_790_000_001,
    state: "received",
    request: false,
    reactions: [],
    reportable: true,
    ...patch,
  };
}

/** A conversation as the app gives it: the person, the safety code, and the messages. */
export function conversationView(
  person: ConversationSummary,
  messages: MessageView[] = [],
  safetyCode: string | null = "5373 9207 7552",
): ConversationView {
  // Opening a conversation marks its messages seen: its line has none unseen.
  return { person: { ...person, unseen: 0 }, safetyCode, messages };
}

/** Your points, as the app gives them: none yet, no places, no badges, and no changes. */
export function pointsView(patch: Partial<PointsView> = {}): PointsView {
  return {
    total: 0,
    week: 0,
    placeWeek: null,
    placeAll: null,
    badges: [],
    thankedBy: 0,
    recent: [],
    ...patch,
  };
}

/** One place on the leaderboard: first, Pat Lee, no picture, no badges, no points. */
export function leaderView(patch: Partial<LeaderView> = {}): LeaderView {
  return {
    place: 1,
    memberId: "cm_01J9Z8Y7X6W5V4T3S2R1Q0P9N8",
    name: "pat-lee",
    displayName: "Pat Lee",
    hasPicture: false,
    pictureVersion: null,
    badges: [],
    points: 0,
    ...patch,
  };
}

/** The leaderboard for This week, with these places, and you not on it. */
export function leaderboardView(
  top: LeaderView[] = [],
  patch: Partial<LeaderboardView> = {},
): LeaderboardView {
  return { allTime: false, since: null, top, myPlace: null, myPoints: 0, ...patch };
}

/** Getting started: no step done, and not closed. */
export function gettingStarted(patch: Partial<GettingStarted> = {}): GettingStarted {
  return { profile: false, foundSomeone: false, sentAMessage: false, closed: false, ...patch };
}
