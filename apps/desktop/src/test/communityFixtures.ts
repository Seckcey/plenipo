// Samples for Community (Phase 24): what the screens get from the app.

import type { CardView, CommunityView, PeoplePage, ProfileDraft } from "@plenipo/types";

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
