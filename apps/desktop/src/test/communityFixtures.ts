// Samples for Community (Phase 24): what the screens get from the app.

import type { CommunityView, ProfileDraft } from "@plenipo/types";

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
