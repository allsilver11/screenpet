// 🖼️ ui/photo.js의 줄이기 계산을 웹뷰 없이 돌려 본다: 가짜 인코더(크기 = 픽셀 × 품질 × 비율)로, 결과를 JSON으로.
// node photo_harness.js → stdout JSON (test_feedback_ui.py가 본다)
const fs = require("fs");
const path = require("path");
const vm = require("vm");

const ctx = vm.createContext({ console, Math, Promise, Uint8Array, String, btoa, atob });
vm.runInContext(fs.readFileSync(path.join(__dirname, "..", "ui", "photo.js"), "utf8"), ctx, { filename: "photo.js" });
const run = (code) => vm.runInContext(code, ctx);

// 바이트 수 = w·h·q·ratio 인 가짜 인코더. webp=false면 WebP를 청해도 PNG가 옴 (사파리처럼)
function encoder(ratio, webp = true) {
  const tries = [];
  const encode = async (w, h, type, q) => {
    tries.push([w, h, type, q]);
    const t = type === "image/webp" && !webp ? "image/png" : type;
    return { type: t, size: Math.round(w * h * q * ratio) };
  };
  return { tries, encode };
}

async function fit(w, h, ratio, webp) {
  const e = encoder(ratio, webp);
  const r = await run("fitPhoto")(w, h, e.encode);
  return r && { w: r.w, h: r.h, type: r.type, bytes: r.blob.size, tries: e.tries };
}

(async () => {
  const out = { MAX_SIDE: run("PHOTO_MAX_SIDE"), MAX_BYTES: run("PHOTO_MAX_BYTES") };
  out.big = await fit(1920, 1080, 0.3);
  out.small = await fit(300, 200, 0.3);
  out.busy = await fit(1280, 720, 1.2);
  out.safari = await fit(800, 600, 0.3, false);
  out.never = await fit(4000, 3000, 1000);
  const kind = run("photoKind");
  const webp = [0x52, 0x49, 0x46, 0x46, 0, 0, 0, 0, 0x57, 0x45, 0x42, 0x50];
  out.kinds = [webp, [0xff, 0xd8, 0xff, 0xe0], [0x89, 0x50, 0x4e, 0x47, 13, 10, 26, 10, 0, 0, 0, 0], [0x52, 0x49, 0x46, 0x46], [0x3c, 0x73, 0x76, 0x67]].map((b) => kind(Uint8Array.from(b)));
  // 머리에서 읽은 크기와 폭탄 거절 (Rust letter::photo_size·photo_ok와 같은 규칙)
  const vp8 = [0x52, 0x49, 0x46, 0x46, 0, 0, 0, 0, 0x57, 0x45, 0x42, 0x50, 0x56, 0x50, 0x38, 0x20, 0, 0, 0, 0, 0x10, 0x02, 0x00, 0x9d, 0x01, 0x2a, 0x80, 0x02, 0x68, 0x01, 7, 7];
  const vp8l = [0x52, 0x49, 0x46, 0x46, 0, 0, 0, 0, 0x57, 0x45, 0x42, 0x50, 0x56, 0x50, 0x38, 0x4c, 0, 0, 0, 0, 0x2f, 0xfe, 0xff, 0xff, 0x0f, 0];
  const vp8x = [0x52, 0x49, 0x46, 0x46, 0, 0, 0, 0, 0x57, 0x45, 0x42, 0x50, 0x56, 0x50, 0x38, 0x58, 10, 0, 0, 0, 0, 0, 0, 0, 0xff, 0x0f, 0, 0xff, 0x0f, 0];
  const jpeg = [0xff, 0xd8, 0xff, 0xe0, 0, 4, 0, 0, 0xff, 0xc0, 0, 17, 8, 0x02, 0x58, 0x03, 0x20, 3];
  const size = run("photoSize");
  out.sizes = [vp8, vp8l, vp8x, jpeg, vp8.slice(0, 20)].map((b) => size(Uint8Array.from(b)));
  out.ok = [vp8, vp8l, vp8x, jpeg].map((b) => run("photoOk")(Uint8Array.from(b)));
  const b64 = run("bytesToBase64")(Uint8Array.from([0, 1, 2, 250, 255, 7]));
  out.b64 = Array.from(run("base64ToBytes")(b64));
  console.log(JSON.stringify(out));
})();
