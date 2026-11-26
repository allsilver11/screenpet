// 💌 건의하기 창 (트레이 "💌 개발자에게 건의하기…"): 편지를 쓰고 "보내기" → 백엔드가 개발자 열쇠로 봉해 릴레이 편지함에 맡김
// (spec 2026-11-10-feedback-mailbox §4). 하루 5통·글자 수 같은 제한은 릴레이가 셈 — 여기서는 결과를 한 줄로 보여 줄 뿐.
// 결과는 `feedback-result`({ok, reason, photo_dropped}) 이벤트로 옴. 보냈으면 2초 뒤 창이 닫히고, 못 보냈으면 쓴 글이 그대로 남음.
// 🖼️ 사진 한 장: [📷 사진 붙이기] 또는 Ctrl+V로 캡처 → photo.js가 작게 다시 인코딩 (긴 변 640px, 120 KiB) → 미리보기와 ✕.
// 옛 서버면 글만 가고 그렇다고 알려 줌 (photo_dropped).
const { invoke } = window.__TAURI__.core;
const here = window.__TAURI__.webviewWindow.getCurrentWebviewWindow();

const MAX_CHARS = 1000;
const inputEl = document.getElementById("input");
const countEl = document.getElementById("count");
const noteEl = document.getElementById("note");
const okBtn = document.getElementById("ok");
const cancelBtn = document.getElementById("cancel");
const fileEl = document.getElementById("photo-file");
const pickBtn = document.getElementById("photo-pick");
const photoBox = document.getElementById("photo-box");
const thumbEl = document.getElementById("thumb");
const hintEl = document.getElementById("photo-hint");
let sending = false;
// 붙인 사진 {bytes, type, url} (없으면 null)
let photo = null;
// 🖼️ 줄이는 중인 사진 수, 마지막 붙이기 번호 (나중에 붙인 것만 남김). 줄이는 동안은 보내지 않음 (글만 가고 사진이 뒤늦게 붙어 보이지 않게)
let shrinking = 0;
let attachNo = 0;

// 릴레이(또는 앱)가 알려 준 이유 → 한 줄
function reasonText(reason) {
  switch (reason) {
    case "daily":
      return "건의는 하루에 5통까지 보낼 수 있어요 — 내일 또 보내 주세요";
    case "new":
      return "앱을 쓴 지 하루가 지나야 보낼 수 있어요";
    case "ip":
    case "global":
      return "오늘은 편지가 많이 왔어요 — 내일 보내 주세요";
    case "full":
      return "건의함이 꽉 찼어요 — 나중에 보내 주세요";
    case "size":
      return "너무 길어요";
    case "photo_busy":
      return "지금 사진 편지가 몰려요 — 잠시 뒤 다시 보내 주세요";
    case "photo":
      return "사진을 붙일 수 없어요 — 다른 사진을 골라 주세요";
    default:
      return "지금은 보낼 수 없어요";
  }
}

// 보냈을 때 한 줄 (옛 서버라 사진이 빠졌으면 그렇다고)
function sentText(photoDropped) {
  return photoDropped ? "보냈어요 💌 (서버가 아직 사진을 못 받아서 글만 갔어요)" : "보냈어요 💌 고마워요!";
}

// 붙인 사진을 뺌 (미리보기 URL도 놓음)
function clearPhoto() {
  if (photo) URL.revokeObjectURL(photo.url);
  photo = null;
  thumbEl.removeAttribute("src");
  photoBox.style.display = "none";
  pickBtn.style.display = "";
  hintEl.style.display = "";
}

// 고른·붙인 파일을 작게 줄여 붙임 (한 장만 — 새로 붙이면 바뀜)
async function attach(file) {
  if (sending) return;
  const no = ++attachNo;
  shrinking++;
  okBtn.disabled = true;
  noteEl.textContent = "사진을 줄이는 중…";
  let r;
  try {
    r = await shrinkPhoto(file);
  } finally {
    shrinking--;
    okBtn.disabled = sending || shrinking > 0;
  }
  // 그새 더 새 사진을 붙였거나 보내기 시작했으면 버림
  if (no !== attachNo || sending) return;
  if (r.error) {
    noteEl.textContent = r.error === "big" ? "사진이 너무 커요 — 화면 일부만 캡처해 주세요" : "사진을 붙일 수 없어요 — 다른 사진을 골라 주세요";
    return;
  }
  clearPhoto();
  photo = { bytes: r.bytes, type: r.type, url: URL.createObjectURL(new Blob([r.bytes], { type: r.type })) };
  thumbEl.src = photo.url;
  photoBox.style.display = "block";
  pickBtn.style.display = "none";
  hintEl.style.display = "none";
  noteEl.textContent = `사진 ${r.w}×${r.h} · ${Math.ceil(r.bytes.length / 1024)} KB`;
}

// 글자 수를 보임
function count() {
  countEl.textContent = `${[...inputEl.value].length}/${MAX_CHARS}자`;
}

// 편지를 보냄 (보내는 중이면 무시)
function submit() {
  const text = inputEl.value.trim();
  if (!text || sending || shrinking > 0) return;
  sending = true;
  okBtn.disabled = true;
  noteEl.textContent = "펫이 편지를 들고 가는 중…";
  invoke("feedback_submit", { text: inputEl.value, photo: photo ? bytesToBase64(photo.bytes) : null });
}

// 보낸 결과를 한 줄로 보이고 버튼을 다시 켬
function result({ ok, reason, photo_dropped }) {
  sending = false;
  okBtn.disabled = shrinking > 0;
  if (ok) {
    noteEl.textContent = sentText(!!photo_dropped);
    inputEl.value = "";
    clearPhoto();
    count();
    setTimeout(() => invoke("feedback_close"), 2000);
  } else {
    noteEl.textContent = reasonText(reason);
  }
}

inputEl.addEventListener("input", () => {
  noteEl.textContent = "";
  count();
});
okBtn.addEventListener("click", submit);
pickBtn.addEventListener("click", () => fileEl.click());
fileEl.addEventListener("change", () => {
  const f = fileEl.files && fileEl.files[0];
  fileEl.value = "";
  if (f) attach(f);
});
document.getElementById("photo-remove").addEventListener("click", () => {
  clearPhoto();
  noteEl.textContent = "";
});
// 📋 캡처를 Ctrl+V로: 붙여 넣은 것 중 그림이 있으면 그것을 (글은 글 상자에 그대로)
window.addEventListener("paste", (e) => {
  const items = (e.clipboardData && e.clipboardData.items) || [];
  for (const it of items) {
    if (it.kind === "file" && /^image\//.test(it.type)) {
      e.preventDefault();
      attach(it.getAsFile());
      return;
    }
  }
});
cancelBtn.addEventListener("click", () => invoke("feedback_close"));
// 여러 줄 글이라 Enter는 줄바꿈 — Ctrl/⌘+Enter로 보내기, Esc로 닫기
window.addEventListener("contextmenu", (e) => e.preventDefault());
window.addEventListener("keydown", (e) => {
  if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) {
    e.preventDefault();
    submit();
  } else if (e.key === "Escape") {
    invoke("feedback_close");
  }
});

(async () => {
  await here.listen("feedback-result", (e) => result(e.payload));
  await here.listen("feedback-open", () => {
    noteEl.textContent = "";
    inputEl.focus();
  });
  count();
  inputEl.focus();
})();
