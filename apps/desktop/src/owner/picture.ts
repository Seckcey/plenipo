/**
 * Your picture (Phase 18, ADR-056 §3): the window reads the file you chose in Windows' own
 * "Open" box, cuts it square from the middle, and shrinks it to at most 256 × 256 pixels. Plenipo
 * receives only that small PNG, in base64; it never opens a file path itself.
 *
 * The window reads the picture's size first and refuses one over 50 megapixels, then makes only
 * the small square (not the whole photo at full size), so a huge photo cannot use up its memory.
 */

/** The largest side of the picture Plenipo keeps, in pixels. */
export const PICTURE_SIDE = 256;
/** The largest picture Plenipo keeps, in bytes. */
export const MAX_PICTURE_BYTES = 256 * 1024;
/** The largest file the window reads, in bytes (a phone photo is well under this). */
export const MAX_FILE_BYTES = 20 * 1024 * 1024;
/** The most pixels a picture may have (50 megapixels: far more than a usual phone photo). */
export const MAX_PIXELS = 50_000_000;
/** The kinds of pictures the window can read. */
export const PICTURE_TYPES = [
  "image/png",
  "image/jpeg",
  "image/gif",
  "image/webp",
  "image/bmp",
] as const;
/** For the file input: what the "Open" box offers. */
export const PICTURE_ACCEPT = PICTURE_TYPES.join(",");

/** Smaller sizes to try when a picture is still too big at 256 × 256. */
const SMALLER_SIDES = [192, 128, 96, 64];

/** A refusal in plain words, to show as it is. */
export class PictureError extends Error {
  override name = "PictureError";
}

export const NOT_A_PICTURE =
  "That file isn't a picture Plenipo can use. Choose a PNG, JPEG, GIF, WebP, or BMP picture.";
export const FILE_TOO_BIG = "That picture is too big (over 20 MB). Choose a smaller one.";
export const CANNOT_READ = "Plenipo couldn't read that picture. Try another one.";
export const TOO_MANY_PIXELS = "That picture is too large to use: choose one under 50 megapixels.";
export const STILL_TOO_BIG = "That picture is still too big after shrinking it. Try another one.";

/** A square part of a picture: where it starts, and its side, in pixels. */
export interface Crop {
  sx: number;
  sy: number;
  side: number;
}

/** The biggest square in the middle of a picture: where to start, and its side. */
export function squareCrop(width: number, height: number): Crop {
  const side = Math.max(0, Math.floor(Math.min(width, height)));
  return {
    sx: Math.floor((width - side) / 2),
    sy: Math.floor((height - side) / 2),
    side,
  };
}

/** The sizes to draw a square of `side` pixels at, largest first (never larger than it is). */
export function sidesToTry(side: number): number[] {
  const first = Math.min(PICTURE_SIDE, Math.floor(side));
  if (first < 1) return [];
  return [first, ...SMALLER_SIDES.filter((s) => s < first)];
}

/** The base64 inside a PNG `data:` address (what a canvas gives), without the `data:` part. */
export function pngBase64(dataUrl: string): string {
  const prefix = "data:image/png;base64,";
  if (!dataUrl.startsWith(prefix)) throw new PictureError(CANNOT_READ);
  return dataUrl.slice(prefix.length);
}

/** How many bytes a base64 text holds. */
export function base64Bytes(base64: string): number {
  const text = base64.trim();
  const padding = text.endsWith("==") ? 2 : text.endsWith("=") ? 1 : 0;
  return Math.floor((text.length * 3) / 4) - padding;
}

/** Whether the window can read a file as a picture, by its kind (an unknown kind is tried). */
export function isPictureType(type: string): boolean {
  return type === "" || (PICTURE_TYPES as readonly string[]).includes(type.toLowerCase());
}

/** A picture the window has read. */
export interface DecodedPicture {
  source: CanvasImageSource;
  width: number;
  height: number;
  /** Let go of what reading it took. */
  close: () => void;
}

/** How the window reads and draws a picture (tests put a stand-in here: jsdom has no canvas). */
export interface Painter {
  /** The picture's size in pixels, read without making the picture itself. */
  measure: (file: Blob) => Promise<{ width: number; height: number }>;
  /**
   * Read the picture. Where the window can, it makes only `crop` of it, `side` pixels square;
   * otherwise the whole picture (either way, the result's `width` and `height` say what it made).
   */
  decode: (file: Blob, crop: Crop, side: number) => Promise<DecodedPicture>;
  /** Draw `crop` of the picture as a square of `side` pixels; returns a PNG `data:` address. */
  draw: (picture: DecodedPicture, crop: Crop, side: number) => string;
}

/**
 * The file as a `data:` address. Plenipo's pages may show pictures only from themselves or from
 * `data:` addresses (the content policy in `tauri.conf.json`), so never a `blob:` one.
 */
function pictureAddress(file: Blob): Promise<string> {
  return new Promise<string>((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () =>
      typeof reader.result === "string"
        ? resolve(reader.result)
        : reject(new Error("The picture could not be read."));
    reader.onerror = () => reject(new Error("The picture could not be read."));
    reader.readAsDataURL(file);
  });
}

/**
 * The picture's size, from an `<img>` that loads it but is never drawn (so the window does not
 * make the full-size picture for it).
 */
async function measureInWindow(file: Blob): Promise<{ width: number; height: number }> {
  const address = await pictureAddress(file);
  return new Promise<{ width: number; height: number }>((resolve, reject) => {
    const image = new Image();
    image.onload = () => resolve({ width: image.naturalWidth, height: image.naturalHeight });
    image.onerror = () => reject(new Error("The picture could not be read."));
    image.src = address;
  });
}

function fromBitmap(bitmap: ImageBitmap): DecodedPicture {
  return {
    source: bitmap,
    width: bitmap.width,
    height: bitmap.height,
    close: () => bitmap.close(),
  };
}

async function decodeInWindow(file: Blob, crop: Crop, side: number): Promise<DecodedPicture> {
  if (typeof createImageBitmap === "function") {
    try {
      // Only the middle square, already small.
      return fromBitmap(
        await createImageBitmap(file, crop.sx, crop.sy, crop.side, crop.side, {
          resizeWidth: side,
          resizeHeight: side,
          resizeQuality: "high",
        }),
      );
    } catch {
      // A window that cannot shrink as it reads: the whole picture (at most 50 megapixels).
      return fromBitmap(await createImageBitmap(file));
    }
  }
  const image = new Image();
  image.src = await pictureAddress(file);
  await image.decode();
  return {
    source: image,
    width: image.naturalWidth,
    height: image.naturalHeight,
    close: () => undefined,
  };
}

function drawInWindow(picture: DecodedPicture, crop: Crop, side: number): string {
  const canvas = document.createElement("canvas");
  canvas.width = side;
  canvas.height = side;
  const context = canvas.getContext("2d");
  if (!context) throw new PictureError(CANNOT_READ);
  context.imageSmoothingEnabled = true;
  context.imageSmoothingQuality = "high";
  context.drawImage(picture.source, crop.sx, crop.sy, crop.side, crop.side, 0, 0, side, side);
  return canvas.toDataURL("image/png");
}

/** The window's own reading and drawing. */
export const WINDOW_PAINTER: Painter = {
  measure: measureInWindow,
  decode: decodeInWindow,
  draw: drawInWindow,
};

/**
 * Read a chosen picture, cut the middle square, and shrink it to at most 256 × 256 pixels (or
 * smaller, until the PNG is at most 256 KB). Resolves with the PNG in base64, without the
 * `data:` part; refuses with a `PictureError` in plain words.
 */
export async function shrinkToPng(file: Blob, painter: Painter = WINDOW_PAINTER): Promise<string> {
  if (!isPictureType(file.type)) throw new PictureError(NOT_A_PICTURE);
  if (file.size > MAX_FILE_BYTES) throw new PictureError(FILE_TOO_BIG);
  let size: { width: number; height: number };
  try {
    size = await painter.measure(file);
  } catch {
    throw new PictureError(CANNOT_READ);
  }
  if (size.width * size.height > MAX_PIXELS) throw new PictureError(TOO_MANY_PIXELS);
  const middle = squareCrop(size.width, size.height);
  const first = sidesToTry(middle.side)[0];
  if (first === undefined) throw new PictureError(CANNOT_READ);
  let picture: DecodedPicture;
  try {
    picture = await painter.decode(file, middle, first);
  } catch {
    throw new PictureError(CANNOT_READ);
  }
  try {
    // The middle square of what was read: the whole picture's, or all of the square made.
    const crop = squareCrop(picture.width, picture.height);
    const sides = sidesToTry(crop.side);
    if (sides.length === 0) throw new PictureError(CANNOT_READ);
    for (const side of sides) {
      let png: string;
      try {
        png = pngBase64(painter.draw(picture, crop, side));
      } catch {
        throw new PictureError(CANNOT_READ);
      }
      if (png.length > 0 && base64Bytes(png) <= MAX_PICTURE_BYTES) return png;
    }
    throw new PictureError(STILL_TOO_BIG);
  } finally {
    picture.close();
  }
}
