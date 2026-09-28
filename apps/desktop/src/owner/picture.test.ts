import { describe, expect, it, vi, type Mock } from "vitest";

import {
  CANNOT_READ,
  FILE_TOO_BIG,
  MAX_FILE_BYTES,
  MAX_PICTURE_BYTES,
  NOT_A_PICTURE,
  PICTURE_ACCEPT,
  PictureError,
  STILL_TOO_BIG,
  base64Bytes,
  isPictureType,
  pngBase64,
  shrinkToPng,
  sidesToTry,
  squareCrop,
  type DecodedPicture,
  type Painter,
} from "./picture";

/** Base64 text that holds exactly `bytes` bytes. */
const base64Of = (bytes: number) =>
  "A".repeat(Math.floor(bytes / 3) * 4) + ["", "AA==", "AAA="][bytes % 3];

/** A stand-in for the window's reading and drawing: a picture of `width` × `height`. */
function fakePainter(
  width: number,
  height: number,
  bytesAt: (side: number) => number = () => 1000,
): Painter & { close: Mock<() => void>; draw: Mock<Painter["draw"]> } {
  const close = vi.fn<() => void>();
  const draw = vi.fn<Painter["draw"]>(
    (_: DecodedPicture, __: unknown, side: number) =>
      `data:image/png;base64,${base64Of(bytesAt(side))}`,
  );
  return {
    decode: () => Promise.resolve({ source: {} as CanvasImageSource, width, height, close }),
    draw,
    close,
  };
}

const png = (size = 1000, type = "image/png") =>
  ({ type, size, name: "me.png" }) as unknown as Blob;

describe("cutting the picture square", () => {
  it("takes the biggest square from the middle", () => {
    expect(squareCrop(400, 300)).toEqual({ sx: 50, sy: 0, side: 300 });
    expect(squareCrop(300, 400)).toEqual({ sx: 0, sy: 50, side: 300 });
    expect(squareCrop(256, 256)).toEqual({ sx: 0, sy: 0, side: 256 });
    expect(squareCrop(101, 50)).toEqual({ sx: 25, sy: 0, side: 50 });
    expect(squareCrop(0, 10)).toEqual({ sx: 0, sy: 5, side: 0 });
  });

  it("shrinks to 256 at most, and never grows a small picture", () => {
    expect(sidesToTry(4000)).toEqual([256, 192, 128, 96, 64]);
    expect(sidesToTry(256)).toEqual([256, 192, 128, 96, 64]);
    expect(sidesToTry(100)).toEqual([100, 96, 64]);
    expect(sidesToTry(40)).toEqual([40]);
    expect(sidesToTry(0)).toEqual([]);
  });
});

describe("the PNG's base64", () => {
  it("drops the data: part", () => {
    expect(pngBase64("data:image/png;base64,iVBORw0KGgo=")).toBe("iVBORw0KGgo=");
    expect(() => pngBase64("data:image/jpeg;base64,AAAA")).toThrow(PictureError);
    expect(() => pngBase64("data:,")).toThrow(CANNOT_READ);
  });

  it("counts the bytes it holds", () => {
    expect(base64Bytes("QUJD")).toBe(3);
    expect(base64Bytes("QUI=")).toBe(2);
    expect(base64Bytes("QQ==")).toBe(1);
    expect(base64Bytes("")).toBe(0);
  });
});

describe("the kinds of pictures", () => {
  it("reads PNG, JPEG, GIF, WebP, and BMP, and tries a file of unknown kind", () => {
    expect(PICTURE_ACCEPT).toBe("image/png,image/jpeg,image/gif,image/webp,image/bmp");
    for (const type of ["image/png", "image/jpeg", "image/gif", "image/webp", "image/bmp", ""]) {
      expect(isPictureType(type)).toBe(true);
    }
    for (const type of ["text/plain", "image/svg+xml", "application/pdf", "image/heic"]) {
      expect(isPictureType(type)).toBe(false);
    }
  });
});

describe("shrinkToPng", () => {
  it("cuts the middle square and draws it at 256 × 256", async () => {
    const painter = fakePainter(1200, 800);
    await expect(shrinkToPng(png(), painter)).resolves.toBe(base64Of(1000));
    expect(painter.draw).toHaveBeenCalledTimes(1);
    expect(painter.draw.mock.calls[0]?.[1]).toEqual({ sx: 200, sy: 0, side: 800 });
    expect(painter.draw.mock.calls[0]?.[2]).toBe(256);
    expect(painter.close).toHaveBeenCalledTimes(1);
  });

  it("keeps a small picture at its own size", async () => {
    const painter = fakePainter(90, 120);
    await shrinkToPng(png(), painter);
    expect(painter.draw.mock.calls[0]?.[1]).toEqual({ sx: 0, sy: 15, side: 90 });
    expect(painter.draw.mock.calls[0]?.[2]).toBe(90);
  });

  it("tries smaller sizes until the PNG is at most 256 KB", async () => {
    const painter = fakePainter(2000, 2000, (side) =>
      side > 128 ? MAX_PICTURE_BYTES + 1 : MAX_PICTURE_BYTES,
    );
    await expect(shrinkToPng(png(), painter)).resolves.toBe(base64Of(MAX_PICTURE_BYTES));
    expect(painter.draw.mock.calls.map((c) => c[2])).toEqual([256, 192, 128]);
  });

  it("refuses, in plain words, a picture still too big at every size", async () => {
    const painter = fakePainter(2000, 2000, () => MAX_PICTURE_BYTES + 3);
    await expect(shrinkToPng(png(), painter)).rejects.toThrow(STILL_TOO_BIG);
    expect(painter.close).toHaveBeenCalledTimes(1);
  });

  it("refuses a file that is not a picture, or too big, before reading it", async () => {
    const painter = fakePainter(10, 10);
    const decode = vi.spyOn(painter, "decode");
    await expect(shrinkToPng(png(10, "text/plain"), painter)).rejects.toThrow(NOT_A_PICTURE);
    await expect(shrinkToPng(png(MAX_FILE_BYTES + 1), painter)).rejects.toThrow(FILE_TOO_BIG);
    expect(decode).not.toHaveBeenCalled();
  });

  it("says so when the window cannot read the picture", async () => {
    const broken: Painter = {
      decode: () => Promise.reject(new Error("The source image could not be decoded.")),
      draw: () => "",
    };
    await expect(shrinkToPng(png(), broken)).rejects.toThrow(CANNOT_READ);
    const empty = fakePainter(0, 0);
    await expect(shrinkToPng(png(), empty)).rejects.toThrow(CANNOT_READ);
    expect(empty.close).toHaveBeenCalledTimes(1);
    const noCanvas: Painter = {
      ...fakePainter(10, 10),
      draw: () => {
        throw new Error("no canvas");
      },
    };
    await expect(shrinkToPng(png(), noCanvas)).rejects.toBeInstanceOf(PictureError);
  });
});
