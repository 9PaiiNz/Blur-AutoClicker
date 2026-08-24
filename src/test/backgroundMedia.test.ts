import { describe, it, expect } from "vitest";
import {
  classifyRawInput,
  escapeCssUrl,
  getExtension,
  isLegacyVideoExtension,
  isPlayableVideoExtension,
  isSafeConvertedAssetUrl,
  isVideoExtension,
  resolveBackgroundSource,
  sanitizeDataImageUrl,
  sanitizeHttpUrl,
} from "../backgroundMedia";

describe("backgroundMedia helpers", () => {
  it("getExtension handles paths and urls", () => {
    expect(getExtension("C:\\a\\b.png")).toBe("png");
    expect(getExtension("/home/user/photo.HEIC")).toBe("heic");
    expect(getExtension("https://example.com/path/video.mp4?x=1#frag")).toBe(
      "mp4",
    );
    expect(getExtension("noext")).toBeNull();
    expect(getExtension("file.")).toBeNull();
    expect(getExtension("C:\\path\\image.svg")).toBe("svg");
  });

  it("isVideoExtension and playable distinction", () => {
    expect(isVideoExtension("mp4")).toBe(true);
    expect(isVideoExtension("mov")).toBe(true);
    expect(isVideoExtension("png")).toBe(false);
    expect(isPlayableVideoExtension("mp4")).toBe(true);
    expect(isPlayableVideoExtension("mov")).toBe(false);
    expect(isLegacyVideoExtension("mov")).toBe(true);
    expect(isLegacyVideoExtension("mp4")).toBe(false);
  });

  it("escapeCssUrl escapes css break chars in order", () => {
    expect(escapeCssUrl('a"b')).toBe('a\\"b');
    expect(escapeCssUrl("a'b")).toBe("a\\'b");
    expect(escapeCssUrl("a)b")).toBe("a\\)b");
    expect(escapeCssUrl("a\\b")).toBe("a\\\\b");
    expect(escapeCssUrl("a\nb\rc\0d")).toBe("abcd");
  });

  it("sanitizeHttpUrl validates", () => {
    expect(sanitizeHttpUrl("https://example.com/image.png")).toBe(
      "https://example.com/image.png",
    );
    expect(sanitizeHttpUrl("http://example.com/video.mp4")).toBe(
      "http://example.com/video.mp4",
    );
    expect(sanitizeHttpUrl("javascript:alert(1)")).toBeNull();
    expect(
      sanitizeHttpUrl("https://user:pass@example.com/image.png"),
    ).toBeNull();
    expect(sanitizeHttpUrl("https://example.com/image.png\ninject")).toBeNull();
    expect(sanitizeHttpUrl("")).toBeNull();
    expect(sanitizeHttpUrl("blob:https://example.com/uuid")).toBeNull();
  });

  it("sanitizeDataImageUrl allows only allowlisted mimes base64", () => {
    expect(
      sanitizeDataImageUrl(
        "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAAB",
      ),
    ).not.toBeNull();
    expect(
      sanitizeDataImageUrl("data:image/svg+xml;base64,PHN2Zz48L3N2Zz4="),
    ).not.toBeNull();
    expect(
      sanitizeDataImageUrl("data:text/html;base64,PHNjcmlwdD5hbGVydA=="),
    ).toBeNull();
    expect(sanitizeDataImageUrl("data:image/png;base64,INVALID!!!")).toBeNull();
    expect(
      sanitizeDataImageUrl("data:image/png;charset=utf-8;base64,abcd"),
    ).toBeNull();
  });

  it("isSafeConvertedAssetUrl strict host", () => {
    expect(isSafeConvertedAssetUrl("http://asset.localhost/foo")).toBe(true);
    expect(isSafeConvertedAssetUrl("https://asset.localhost/foo")).toBe(true);
    expect(isSafeConvertedAssetUrl("asset://localhost/foo")).toBe(true);
    expect(isSafeConvertedAssetUrl("https://evil.com/foo")).toBe(false);
    expect(isSafeConvertedAssetUrl("http://evil.asset.localhost/foo")).toBe(
      false,
    );
    expect(isSafeConvertedAssetUrl("http://asset.localhost.evil.com/foo")).toBe(
      false,
    );
  });

  it("classifyRawInput handles schemes and windows paths", () => {
    expect(classifyRawInput("https://example.com/image.png").type).toBe("http");
    expect(classifyRawInput("https://example.com/video.mp4").kind).toBe(
      "video",
    );
    expect(classifyRawInput("https://example.com/noext").kind).toBe("image");
    expect(classifyRawInput("javascript:alert(1)").type).toBe("blocked");
    expect(classifyRawInput("blob:https://example.com/uuid").type).toBe(
      "blocked",
    );
    expect(classifyRawInput("file:///C:/a.png").type).toBe("blocked");
    expect(classifyRawInput("C:\\Users\\Luca\\Videos\\loop.mp4").type).toBe(
      "local",
    );
    expect(classifyRawInput("C:\\Users\\Luca\\Videos\\loop.mp4").kind).toBe(
      "video",
    );
    expect(classifyRawInput("C:\\path\\transparent.png").kind).toBe("image");
    expect(classifyRawInput("").type).toBe("empty");
    expect(classifyRawInput("DATA:image/png;base64,abcd").type).toBe("data");
    expect(classifyRawInput("HTTPS://example.com/image.png").type).toBe("http");
  });

  it("resolveBackgroundSource image vs video vs blocked", () => {
    expect(
      resolveBackgroundSource("https://example.com/image.png", null).kind,
    ).toBe("image");
    expect(
      resolveBackgroundSource("https://example.com/video.mp4", null).kind,
    ).toBe("video");
    expect(resolveBackgroundSource("javascript:alert(1)", null).kind).toBe(
      "none",
    );
    expect(
      resolveBackgroundSource("data:text/html;base64,abcd", null).kind,
    ).toBe("none");
    expect(
      resolveBackgroundSource(
        "C:\\Users\\Me\\vid.mp4",
        "http://asset.localhost/vid.mp4",
      ).kind,
    ).toBe("video");
    expect(
      resolveBackgroundSource("C:\\a\\b.png", "http://asset.localhost/a/b.png")
        .kind,
    ).toBe("image");
    expect(resolveBackgroundSource("C:\\a\\b.png", null).kind).toBe("none");
    expect(
      resolveBackgroundSource("C:\\a\\b.png", "https://evil.com/b.png").kind,
    ).toBe("none");
  });

  it("resolveBackgroundSource escapes css url for image", () => {
    const res = resolveBackgroundSource("https://example.com/a.jpg", null);
    expect(res.cssUrl).toBe("https://example.com/a.jpg");
    const inj = resolveBackgroundSource(
      'https://example.com/a.jpg"); background: red; x="',
      null,
    );
    // Should be sanitized via URL.href encoding + escaped, not break out
    expect(inj.kind).toBe("image");
    expect(inj.cssUrl).not.toContain('";');
    expect(inj.cssUrl).toContain("\\");
  });

  it("resolveBackgroundSource handles legacy mov still as video but playable check separate", () => {
    const res = resolveBackgroundSource(
      "C:\\vid\\a.mov",
      "http://asset.localhost/a.mov",
    );
    expect(res.kind).toBe("video");
    expect(isLegacyVideoExtension("mov")).toBe(true);
    expect(isPlayableVideoExtension("mov")).toBe(false);
  });

  it("handles data url length limit separate", () => {
    const small = "data:image/png;base64," + "A".repeat(5000);
    expect(sanitizeDataImageUrl(small)).not.toBeNull();
    const hugeHttp = "https://example.com/" + "a".repeat(9000);
    expect(sanitizeHttpUrl(hugeHttp)).toBeNull();
    expect(classifyRawInput(hugeHttp).type).toBe("blocked");
  });

  it("blocks credentials and newline injection", () => {
    expect(
      resolveBackgroundSource("https://user:pass@example.com/image.png", null)
        .kind,
    ).toBe("none");
    expect(
      resolveBackgroundSource("https://example.com/image.png\nbody{}", null)
        .kind,
    ).toBe("none");
    expect(
      resolveBackgroundSource("https://example.com/image.png\0", null).kind,
    ).toBe("none");
  });
});
