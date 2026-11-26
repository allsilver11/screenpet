// 📮 개발자 건의함 (개발자 비밀키가 있는 컴퓨터만 — spec 2026-11-10-feedback-mailbox §5, 예전 "📬 편지함").
// 목록은 `mailbox-state`([{id, at, from, read, done, rejected, private, ok, text, name, species, stage, style, version, os, photo}], 최신순)로 옴.
// 보낸 사람이 쓴 글은 모두 textContent로만 넣음 (HTML로 해석하지 않음). 열면 읽음, ✅ 반영함 / 🙅 반려 (하나만), 🔒 비공개 (공개 건의함에서 숨기기 — 여기선 그대로 보이고 표시만), 지우기, 이 사람 막기.
// 🖼️ 사진 붙은 편지(photo)는 그 쪽이 보일 때 한 번 `mailbox_photo`로 청하고, `mailbox-photo`({id, kind, data})가 오면 머리 바이트·크기를 다시 보고
// 같은 출처 Blob URL로만 보임 (photo.js). 누르면 크게, 다시 누르거나 Esc면 닫힘. 공개 건의함에는 사진이 없음.
// 공개 건의함(board.js)과 같은 쪽 넘기기 (pager.js, 5개씩): 읽거나 지우거나 막아도 보던 쪽에 남고, 그 쪽이 비면 한 쪽 앞으로.
const { invoke } = window.__TAURI__.core;
const here = window.__TAURI__.webviewWindow.getCurrentWebviewWindow();

const listEl = document.getElementById("list");
const emptyEl = document.getElementById("empty");
let letters = [];
let page = 1;
// 🖼️ 사진 답을 기다리는 시간 (ms)
const PHOTO_WAIT_MS = 10000;
// 🖼️ 편지 id → {state: "loading" | "ok" | "bad", url}
const photos = new Map();
const viewerEl = document.getElementById("viewer");
const viewerImg = document.getElementById("viewer-img");
const pager = makePager(document.getElementById("pager"), (p) => {
  page = p;
  render();
});

// 보낸 펫 이모지 — 없어진 동물(새·기린·레서판다 v0.7.0, 오리·개구리·페르시안 v0.7.1)도 남겨 둠: 옛 버전 사용자가 보낸 편지에 그 종이 적혀 옴
const PET_EMOJI = { cat: "🐱", persian: "🐱", rabbit: "🐰", frog: "🐸", duck: "🦆", bird: "🐦", penguin: "🐧", squirrel: "🐿️", redpanda: "🐾", heumul: "🤸", spider: "🕷️", bat: "🦇", super: "🦸", musoe: "🛡️", harry: "🧙", hermione: "📚", voldemort: "🐍", dementor: "👻", hagrid: "🧔", giraffe: "🦒", pumpkin: "🎃", ghost: "👻", santa: "🎅", rudolph: "🦌", snowman: "⛄" };

// 클래스와 글자를 넣은 요소를 만듦
function el(tag, cls, text) {
  const e = document.createElement(tag);
  if (cls) e.className = cls;
  if (text !== undefined) e.textContent = text;
  return e;
}

// 유닉스 초를 `YYYY.MM.DD HH:MM`으로 바꿈
function when(at) {
  const d = new Date(at * 1000);
  const p = (n) => String(n).padStart(2, "0");
  return `${d.getFullYear()}.${p(d.getMonth() + 1)}.${p(d.getDate())} ${p(d.getHours())}:${p(d.getMinutes())}`;
}

// 새 목록이 와도 보던 쪽 그대로 (비었으면 clampPage가 한 쪽 앞으로). 없어진 편지의 사진 URL은 놓음
function update(next) {
  letters = next;
  const keep = new Set(next.filter((l) => l.photo).map((l) => l.id));
  for (const [id, p] of photos) {
    if (keep.has(id)) continue;
    if (p.url) URL.revokeObjectURL(p.url);
    photos.delete(id);
  }
  render();
}

// 🖼️ 사진을 크게 보임 / 닫음
function showBig(url) {
  viewerImg.src = url;
  viewerEl.style.display = "flex";
}
function hideBig() {
  viewerEl.style.display = "none";
  viewerImg.removeAttribute("src");
}

// 🖼️ 편지 밑 사진 칸 (처음 보이면 청함, 못 받았으면 눌러서 다시)
function photoEl(l) {
  const wrap = el("div", "photo");
  let p = photos.get(l.id);
  if (!p) {
    const asked = { state: "loading" };
    p = asked;
    photos.set(l.id, asked);
    invoke("mailbox_photo", { id: l.id });
    // 답이 안 오면 (끊김 등) 10초 뒤 그만 기다림 — 눌러서 다시
    setTimeout(() => {
      if (photos.get(l.id) !== asked) return;
      photos.set(l.id, { state: "bad" });
      render();
    }, PHOTO_WAIT_MS);
  }
  if (p.state === "ok") {
    const img = el("img", "thumb");
    img.src = p.url;
    img.alt = "붙인 사진";
    img.addEventListener("click", (e) => {
      e.stopPropagation();
      showBig(p.url);
    });
    wrap.append(img);
  } else if (p.state === "loading") {
    wrap.textContent = "🖼️ 사진 불러오는 중…";
  } else {
    wrap.textContent = "🖼️ 사진을 볼 수 없어요";
    wrap.addEventListener("click", (e) => {
      e.stopPropagation();
      photos.delete(l.id);
      render();
    });
  }
  return wrap;
}

// 🖼️ 백엔드가 연 사진: 청한 편지의 것만, WebP/JPEG 머리 바이트가 kind와 맞고 바이트·그림 크기(풀기 전 머리로) 한도 안일 때만 보임
function onPhoto(p) {
  if (!p || !photos.has(p.id)) return;
  const known = p.kind === "image/webp" || p.kind === "image/jpeg";
  const bytes = known && typeof p.data === "string" ? base64ToBytes(p.data) : null;
  if (!bytes || bytes.length > PHOTO_MAX_BYTES || photoKind(bytes) !== p.kind || !photoOk(bytes)) {
    photos.set(p.id, { state: "bad" });
  } else {
    photos.set(p.id, { state: "ok", url: URL.createObjectURL(new Blob([bytes], { type: p.kind })) });
  }
  render();
}

// 지금 쪽의 편지 목록을 그림
function render() {
  page = clampPage(page, letters.length);
  listEl.replaceChildren();
  emptyEl.style.display = letters.length ? "none" : "block";
  pager.set(page, pageCount(letters.length), letters.length > 0);
  for (const l of pageSlice(letters, page)) {
    const box = el("div", ["letter", l.read ? "" : "unread", l.done ? "done" : "", l.rejected ? "rejected" : "", l.private ? "private" : ""].filter(Boolean).join(" "));
    const meta = el("div", "meta");
    const who = `${PET_EMOJI[l.species] || "💌"} ${l.name || "누군가"}`;
    const info = el("span", "");
    // 🔒 비공개 표시 (개발자에게는 그대로 보임)
    if (l.private) info.append(el("span", "badge private", "🔒 비공개"), " ");
    info.append(`${when(l.at)} · v${l.version || "?"} · ${l.os || "?"}`);
    meta.append(el("span", "who", who), info);
    const text = el("div", "text", l.text);
    const row = el("div", "row");
    const del = el("button", "", "지우기");
    del.addEventListener("click", (e) => {
      e.stopPropagation();
      invoke("mailbox_delete", { id: l.id });
    });
    const block = el("button", "", "이 사람 막기");
    // 두 번 눌러야 막음 (맥 웹뷰는 confirm 창을 띄우지 않을 수 있음): 처음엔 "정말 막을까요?"로 바뀌고 3초 안에 한 번 더
    let armed = null;
    block.addEventListener("click", (e) => {
      e.stopPropagation();
      if (armed) {
        clearTimeout(armed);
        invoke("mailbox_block", { from: l.from });
        return;
      }
      block.textContent = "정말 막을까요? (받은 편지도 지워요)";
      armed = setTimeout(() => {
        armed = null;
        block.textContent = "이 사람 막기";
      }, 3000);
    });
    // ✅ 반영함 / 🙅 반려: 하나만 (하나를 켜면 다른 하나가 꺼지고, 켠 것을 끄면 아무것도 아님).
    // 릴레이에 저장돼 두 컴퓨터가 같고, 📮 공개 건의함에도 ✅ 반영됨 / 🙅 반려됨으로 보임
    const status = el("span", "status");
    const checkbox = (text, on) => {
      const label = el("label");
      const box = el("input");
      box.type = "checkbox";
      box.checked = !!on;
      box.addEventListener("click", (e) => e.stopPropagation());
      label.append(box, el("span", "", text));
      label.addEventListener("click", (e) => e.stopPropagation());
      status.append(label);
      return box;
    };
    const check = checkbox("✅ 반영함", l.done);
    const reject = checkbox("🙅 반려", l.rejected && !l.done);
    // 🔒 비공개: 다른 사람의 📮 건의함에서 숨김 (반영·반려와 따로)
    const hide = checkbox("🔒 비공개", l.private);
    hide.addEventListener("change", () => invoke("mailbox_private", { id: l.id, private: hide.checked }));
    check.addEventListener("change", () => {
      if (check.checked) reject.checked = false;
      invoke("mailbox_done", { id: l.id, done: check.checked });
    });
    reject.addEventListener("change", () => {
      if (reject.checked) check.checked = false;
      invoke("mailbox_reject", { id: l.id, rejected: reject.checked });
    });
    row.append(status, del, block);
    box.append(meta, text);
    if (l.photo) box.append(photoEl(l));
    box.append(row);
    // 열어 보면 읽음 (안 읽은 편지만)
    if (!l.read) box.addEventListener("click", () => invoke("mailbox_read", { id: l.id }), { once: true });
    listEl.append(box);
  }
}

document.getElementById("close").addEventListener("click", () => invoke("mailbox_close"));
window.addEventListener("contextmenu", (e) => e.preventDefault());
viewerEl.addEventListener("click", hideBig);
window.addEventListener("keydown", (e) => {
  if (e.key !== "Escape") return;
  if (viewerEl.style.display === "flex") hideBig();
  else invoke("mailbox_close");
});

(async () => {
  await here.listen("mailbox-state", (e) => update(e.payload));
  await here.listen("mailbox-photo", (e) => onPhoto(e.payload));
  update(await invoke("mailbox_state"));
})();
