/**
 * Plenipo's brand, drawn from the owner's approved kit (docs/brand/pip-brand-kit): Pip's poses and
 * the logo's shapes. The shapes are copied from the kit's outlined wordmark
 * (source/approved-wordmark.svg); their colors come from the brand tokens, so the logo follows the
 * theme. Components live in brand.tsx.
 */

import analytics from "./assets/pip/pip-09-analytics.png";
import celebrating from "./assets/pip/pip-05-celebrating.png";
import coding from "./assets/pip/pip-01-coding.png";
import launch from "./assets/pip/pip-11-launch.png";
import learning from "./assets/pip/pip-13-learning.png";
import maintenance from "./assets/pip/pip-12-maintenance.png";
import planning from "./assets/pip/pip-08-planning.png";
import presenting from "./assets/pip/pip-03-presenting.png";
import qualityCheck from "./assets/pip/pip-06-quality-check.png";
import recharging from "./assets/pip/pip-15-recharging.png";
import security from "./assets/pip/pip-07-security.png";
import support from "./assets/pip/pip-14-support.png";
import teamwork from "./assets/pip/pip-10-teamwork.png";
import thinking from "./assets/pip/pip-04-thinking.png";
import welcome from "./assets/pip/pip-02-welcome.png";

/** Pip's 15 poses, in the kit's order. */
export type PipPose =
  | "coding"
  | "welcome"
  | "presenting"
  | "thinking"
  | "celebrating"
  | "quality-check"
  | "security"
  | "planning"
  | "analytics"
  | "teamwork"
  | "launch"
  | "maintenance"
  | "learning"
  | "support"
  | "recharging";

/** Each pose, what Pip is doing (plain words), and where the app uses it. */
export const PIP_POSES: readonly { pose: PipPose; doing: string; use: string }[] = [
  { pose: "coding", doing: "At the laptop", use: "The logo; the terminal" },
  { pose: "welcome", doing: "Waving hello", use: "Home, the first time" },
  { pose: "presenting", doing: "Pointing the way", use: "Something waits for you" },
  { pose: "thinking", doing: "Thinking it over", use: "Objectives and plans" },
  { pose: "celebrating", doing: "Celebrating", use: "Work finished" },
  { pose: "quality-check", doing: "Checking the work", use: "Reviews and Diagnostics" },
  { pose: "security", doing: "Keeping things safe", use: "Approvals and permissions" },
  { pose: "planning", doing: "Checking the list", use: "Tasks and projects" },
  { pose: "analytics", doing: "Reading the numbers", use: "Activity" },
  { pose: "teamwork", doing: "Fitting the pieces together", use: "The organization and workers" },
  { pose: "launch", doing: "Getting started", use: "Setting things up" },
  { pose: "maintenance", doing: "Fixing things", use: "Settings and servers" },
  { pose: "learning", doing: "Reading up", use: "Lessons workers learn" },
  { pose: "support", doing: "Here to help", use: "Something went wrong" },
  { pose: "recharging", doing: "Resting", use: "All quiet" },
];

/** The image for each pose (400 px copies of the kit's 1254 px artwork). */
export const PIP_IMAGES: Record<PipPose, string> = {
  coding,
  welcome,
  presenting,
  thinking,
  celebrating,
  "quality-check": qualityCheck,
  security,
  planning,
  analytics,
  teamwork,
  launch,
  maintenance,
  learning,
  support,
  recharging,
};

/** The P: three rails, drawn in a 100 x 100 box (the kit's favicon geometry). */
export const P_RAILS = [
  { rail: "primary", d: "M19 15H56A26 26 0 0 1 56 67H43A24 24 0 0 0 19 91" },
  { rail: "middle", d: "M19 30H54A12 12 0 0 1 54 54H42A10 10 0 0 0 32 64V91" },
  { rail: "inner", d: "M19 43H44V91" },
] as const;

/** "lenipo" after the P, in Sentient, outlined (each letter's offset and outline). */
export const WORDMARK_LETTERS: readonly { letter: string; x: number; d: string }[] = [
  {
    letter: "l",
    x: 0,
    d: "M303 0L25 0L25 30L63 47Q85 58 92.5 70Q100 82 100 108L100 573Q100 607 92 620.5Q84 634 63 641L28 652L28 682L201 750L228 733L228 108Q228 82 235.5 70Q243 58 265 47L303 30Z",
  },
  {
    letter: "e",
    x: 52.74,
    d: "M289 -10Q176 -10 109.5 57Q43 124 43 249Q43 324 74.5 384Q106 444 161.5 478.5Q217 513 288 513Q355 513 404.5 486.5Q454 460 482 409Q510 358 511 286L511 263L182 263L182 253Q182 164 226 117Q270 70 347 70Q382 70 411.5 80.5Q441 91 472 111L501 74Q459 32 406.5 11Q354 -10 289 -10ZM184 316L382 320Q381 384 356.5 421Q332 458 290 458Q245 458 218.5 422.5Q192 387 184 316Z",
  },
  {
    letter: "n",
    x: 145.8,
    d: "M299 0L25 0L25 30L63 47Q86 58 93 70Q100 82 100 108L100 333Q100 368 92.5 382.5Q85 397 63 403L28 414L28 443L190 513L217 497L217 402Q256 454 304 483.5Q352 513 400 513Q478 513 523 460Q568 407 568 313L568 108Q568 82 575.5 70Q583 58 605 47L642 30L642 0L369 0L369 30L407 47Q427 56 433.5 68Q440 80 440 108L440 287Q440 348 414 379Q388 410 339 410Q309 410 279.5 394Q250 378 228 352L228 108Q228 80 234.5 68Q241 56 261 47L299 30Z",
  },
  {
    letter: "i",
    x: 258.12,
    d: "M303 0L25 0L25 30L63 47Q85 58 92.5 70Q100 82 100 108L100 335Q100 370 92 384Q84 398 63 404L28 415L28 444L201 513L228 497L228 108Q228 82 235.5 70Q243 58 265 47L303 30ZM154 585Q118 585 95 609.5Q72 634 72 668Q72 706 96 728.5Q120 751 154 751Q188 751 212.5 728.5Q237 706 237 668Q237 632 214 608.5Q191 585 154 585Z",
  },
  {
    letter: "p",
    x: 310.86,
    d: "M324 -240L22 -240L22 -210L60 -193Q83 -183 91.5 -169.5Q100 -156 100 -132L100 334Q100 368 92 382.5Q84 397 63 403L28 414L28 444L190 513L217 497L217 421Q256 465 299.5 489Q343 513 385 513Q445 513 491.5 482Q538 451 565 397Q592 343 592 272Q592 184 558 121Q524 58 462 24Q400 -10 314 -10Q271 -10 228 9L228 -132Q228 -154 239 -167Q250 -180 283 -193L324 -210ZM324 44Q363 44 392 68Q421 92 437 135.5Q453 179 453 236Q453 323 419.5 371.5Q386 420 326 420Q299 420 273.5 407Q248 394 228 371L228 142Q230 97 256 70.5Q282 44 324 44Z",
  },
  {
    letter: "o",
    x: 418.86,
    d: "M302 -10Q225 -10 166.5 22.5Q108 55 75.5 114Q43 173 43 252Q43 308 62.5 355.5Q82 403 118 438.5Q154 474 200.5 493.5Q247 513 302 513Q380 513 438 480.5Q496 448 528.5 389Q561 330 561 250Q561 176 527 117Q493 58 434 24Q375 -10 302 -10ZM302 45Q358 45 390 99.5Q422 154 422 250Q422 348 390 403Q358 458 302 458Q247 458 214.5 403.5Q182 349 182 252Q182 155 214.5 100Q247 45 302 45Z",
  },
];
