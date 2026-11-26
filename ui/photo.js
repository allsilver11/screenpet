// 🖼️ 건의 편지 사진 한 장 (💌 건의하기 창에서 줄이고, 📮 개발자 건의함 창에서 다시 확인해 보임).
// 고른·붙여 넣은 그림을 canvas에 다시 그려 긴 변 640px, WebP(안 되면 JPEG) 품질 0.8부터, 120 KiB 안에 들 때까지 품질·크기를 낮춤.
// 다시 인코딩하므로 EXIF·위치 같은 메타데이터는 남지 않음. 그림은 스크립트가 만든 같은 출처 Blob URL로만 읽음 (CSP img-src에 blob 하나만 더함).
// 한도는 screenpet_proto::MAX_IMAGE와 같음 — 백엔드·개발자 앱이 다시 확인함.

/** 긴 변 최대 px */
const PHOTO_MAX_SIDE = 640;
/** 인코딩한 사진 최대 바이트 (screenpet_proto::MAX_IMAGE) */
const PHOTO_MAX_BYTES = 120 * 1024;
/** 고를 수 있는 원본 파일 최대 바이트 (이보다 큰 것은 읽지도 않음) */
const PHOTO_MAX_INPUT = 20 * 1024 * 1024;
/** 원본 최대 픽셀 수 (풀어 놓으면 메모리를 많이 씀) */
const PHOTO_MAX_PIXELS = 40_000_000;
/** 차례로 낮춰 보는 품질 */
const PHOTO_QUALITIES = [0.8, 0.7, 0.6, 0.5];
/** 긴 변이 이만큼 작아져도 안 들면 포기함 */
const PHOTO_MIN_SIDE = 160;

/** 머리 바이트로 본 형식 (WebP, JPEG만 — Rust letter::photo_kind와 같음) */
function photoKind(b) {
  if (b.length >= 12 && b[0] === 0x52 && b[1] === 0x49 && b[2] === 0x46 && b[3] === 0x46 && b[8] === 0x57 && b[9] === 0x45 && b[10] === 0x42 && b[11] === 0x50) return "image/webp";
  if (b.length >= 3 && b[0] === 0xff && b[1] === 0xd8 && b[2] === 0xff) return "image/jpeg";
  return null;
}

/** w×h 그림을 encode(w, h, type, quality) → Blob으로 줄여 한도 안에 넣음: {blob, w, h, type} 또는 null (아무리 해도 안 듦).
 *  WebP를 못 쓰는 웹뷰(타입이 다른 Blob이 옴)는 JPEG로 바꿈. 키우지는 않음 */
async function fitPhoto(w, h, encode) {
  let scale = Math.min(1, PHOTO_MAX_SIDE / Math.max(w, h));
  let type = "image/webp";
  for (;;) {
    const cw = Math.max(1, Math.round(w * scale));
    const ch = Math.max(1, Math.round(h * scale));
    for (const q of PHOTO_QUALITIES) {
      let blob = await encode(cw, ch, type, q);
      if (blob && type === "image/webp" && blob.type !== "image/webp") {
        type = "image/jpeg";
        blob = await encode(cw, ch, type, q);
      }
      if (blob && blob.type === type && blob.size <= PHOTO_MAX_BYTES) return { blob, w: cw, h: ch, type };
    }
    if (Math.max(cw, ch) <= PHOTO_MIN_SIDE) return null;
    scale *= 0.75;
  }
}

/** Blob URL을 그림으로 읽음 */
function loadPhotoImage(url) {
  return new Promise((ok, fail) => {
    const img = new Image();
    img.onload = () => ok(img);
    img.onerror = () => fail(new Error("not an image"));
    img.src = url;
  });
}

/** 고른·붙인 파일을 작은 사진으로: {bytes, type, w, h} 또는 {error: "type" | "big"} */
async function shrinkPhoto(file) {
  if (!file || !/^image\//.test(file.type || "")) return { error: "type" };
  if (file.size > PHOTO_MAX_INPUT) return { error: "big" };
  const url = URL.createObjectURL(file);
  try {
    const img = await loadPhotoImage(url);
    const [w, h] = [img.naturalWidth, img.naturalHeight];
    if (!w || !h) return { error: "type" };
    if (w * h > PHOTO_MAX_PIXELS) return { error: "big" };
    const canvas = document.createElement("canvas");
    const g = canvas.getContext("2d");
    const encode = (cw, ch, type, q) => {
      canvas.width = cw;
      canvas.height = ch;
      g.fillStyle = "#fff";
      g.fillRect(0, 0, cw, ch);
      g.drawImage(img, 0, 0, cw, ch);
      return new Promise((done) => canvas.toBlob(done, type, q));
    };
    const fit = await fitPhoto(w, h, encode);
    if (!fit) return { error: "big" };
    const bytes = new Uint8Array(await fit.blob.arrayBuffer());
    if (photoKind(bytes) !== fit.type || bytes.length > PHOTO_MAX_BYTES) return { error: "type" };
    return { bytes, type: fit.type, w: fit.w, h: fit.h };
  } catch (_) {
    return { error: "type" };
  } finally {
    URL.revokeObjectURL(url);
  }
}

/** 바이트 → 표준 base64 */
function bytesToBase64(bytes) {
  let s = "";
  for (let i = 0; i < bytes.length; i += 0x8000) s += String.fromCharCode.apply(null, bytes.subarray(i, i + 0x8000));
  return btoa(s);
}

/** 표준 base64 → 바이트 (깨졌으면 null) */
function base64ToBytes(text) {
  try {
    const s = atob(text);
    const out = new Uint8Array(s.length);
    for (let i = 0; i < s.length; i++) out[i] = s.charCodeAt(i);
    return out;
  } catch (_) {
    return null;
  }
}
