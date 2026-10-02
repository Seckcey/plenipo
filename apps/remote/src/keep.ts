import type { KeyPair } from "./lock/noise";
import type { Paired } from "./line/phone";

/**
 * What the phone keeps (ADR-141 §8): the pairing, and the phone's own long-term key, made so it can
 * never be copied out (a non-extractable Web Crypto key, kept as it is in the page's own
 * IndexedDB). Clearing the browser's site data, or removing the phone, forgets both; the phone is
 * then paired again.
 */
export interface Kept {
  paired: Paired;
  keys: KeyPair;
}

export interface Keep {
  load(): Promise<Kept | null>;
  save(kept: Kept): Promise<void>;
  forget(): Promise<void>;
}

const DB = "plenipo-remote";
const STORE = "keep";
const KEY = "phone";

function request<T>(r: IDBRequest<T>): Promise<T> {
  return new Promise((resolve, reject) => {
    r.onsuccess = () => resolve(r.result);
    r.onerror = () => reject(r.error ?? new Error("the phone's storage failed"));
  });
}

async function open(): Promise<IDBDatabase> {
  const r = indexedDB.open(DB, 1);
  r.onupgradeneeded = () => {
    if (!r.result.objectStoreNames.contains(STORE)) r.result.createObjectStore(STORE);
  };
  return request(r);
}

/** The page's own storage in this browser. */
export function browserKeep(): Keep {
  return {
    async load() {
      const db = await open();
      try {
        const kept = (await request(db.transaction(STORE).objectStore(STORE).get(KEY))) as
          Kept | undefined;
        return kept ?? null;
      } finally {
        db.close();
      }
    },
    async save(kept) {
      const db = await open();
      try {
        await request(db.transaction(STORE, "readwrite").objectStore(STORE).put(kept, KEY));
      } finally {
        db.close();
      }
    },
    async forget() {
      const db = await open();
      try {
        await request(db.transaction(STORE, "readwrite").objectStore(STORE).delete(KEY));
      } finally {
        db.close();
      }
    },
  };
}

/** Kept in memory only (the tests). */
export function memoryKeep(start: Kept | null = null): Keep & { kept: Kept | null } {
  const keep = {
    kept: start,
    load: () => Promise.resolve(keep.kept),
    save: (k: Kept) => {
      keep.kept = k;
      return Promise.resolve();
    },
    forget: () => {
      keep.kept = null;
      return Promise.resolve();
    },
  };
  return keep;
}
