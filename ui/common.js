// 펫 창(main.js)과 하우스 창(house.js)이 같이 쓰는 것: 시트 읽기, 프레임 계산, 말풍선, 음식 고르기.
// 일반 스크립트끼리는 최상위 const가 겹치면 안 되므로 window.__TAURI__는 함수 안에서만 꺼낸다.

async function decodeSheet(url) {
  const img = new Image();
  img.src = url;
  await img.decode();
  return img;
}

// 동물 id의 pet.json과 단계별 시트 sheet_<stage>.png (없으면 sheet.png) → { manifest, url, img }
async function loadPetSheet(id, stageName) {
  const manifest = await (await fetch(`pets/${id}/pet.json`)).json();
  let url = `pets/${id}/sheet_${stageName}.png`;
  let img;
  try {
    img = await decodeSheet(url);
  } catch {
    url = `pets/${id}/sheet.png`;
    img = await decodeSheet(url);
  }
  return { manifest, url, img };
}

function frameIndex(spec, elapsedMs) {
  const n = Math.floor((elapsedMs / 1000) * spec.fps);
  return spec.loop === false ? Math.min(n, spec.frames - 1) : n % spec.frames;
}

// 말풍선(`pet-say`)과 음식 고르기(`food-picker`). 백엔드는 지금 동물이 있는 창에만(emit_to) 보낸다.
// getManifest() = 지금 동물의 pet.json (대사 목록). 돌려주는 hidePicker는 먹기 시작하면 부른다.
function setupSpeech({ bubbleEl, pickerEl, favBtn, getManifest }) {
  const { invoke } = window.__TAURI__.core;
  // 전역 event.listen은 다른 창 앞으로 보낸(emit_to) 이벤트까지 받는다 — 이 창 앞으로 온 것만 듣는다
  const here = window.__TAURI__.webviewWindow.getCurrentWebviewWindow();
  const listen = (event, handler) => here.listen(event, handler);
  let bubbleTimer = null;
  let pickerTimer = null;

  function hidePicker() {
    pickerEl.hidden = true;
    clearTimeout(pickerTimer);
  }

  // 백엔드가 고른 상황(chatter/tickle/grumpy/hiss/hungry/full/level_up/ignore)에 맞는 대사를
  // pet.json speech에서 랜덤으로 띄운다. 그 동물에 없는 상황이면 말풍선 없음.
  listen("pet-say", (e) => {
    const lines = getManifest()?.speech?.[e.payload] ?? [];
    if (!lines.length || !pickerEl.hidden) return; // 음식 고르기 버튼이 우선
    bubbleEl.textContent = lines[Math.floor(Math.random() * lines.length)];
    bubbleEl.hidden = false;
    clearTimeout(bubbleTimer);
    bubbleTimer = setTimeout(() => {
      bubbleEl.hidden = true;
    }, 1500);
  });

  // 배고플 때 클릭하면 머리 위에 🍪 🍚 (좋아하는 것) 버튼
  listen("food-picker", (e) => {
    favBtn.textContent = e.payload.emoji;
    favBtn.title = e.payload.name;
    bubbleEl.hidden = true;
    pickerEl.hidden = false;
    clearTimeout(pickerTimer);
    pickerTimer = setTimeout(hidePicker, 6000);
  });

  // 하우스로 가기 시작하면 펫 창의 음식 고르기는 닫는다
  listen("hide-picker", hidePicker);

  for (const btn of pickerEl.querySelectorAll("button")) {
    // 버튼을 누르는 게 펫 잡기(하우스 클릭)로 이어지지 않게 한다
    btn.addEventListener("mousedown", (e) => e.stopPropagation());
    btn.addEventListener("click", (e) => {
      e.stopPropagation();
      invoke("feed", { food: btn.dataset.food });
      hidePicker();
    });
  }

  return { hidePicker };
}

// 키보드 UI가 없는 창: 웹뷰 기본 동작(우클릭 메뉴, F5, Ctrl+R, Alt+F4 등)을 막는다.
function blockWebviewDefaults() {
  window.addEventListener("contextmenu", (e) => e.preventDefault());
  window.addEventListener("keydown", (e) => e.preventDefault());
}

// 글자를 클립보드에 (펫 머리 위 친구 코드 띠, 코드 창). 클릭 처리기 안에서 부른다. 성공하면 true
async function copyText(text) {
  try {
    await navigator.clipboard.writeText(text);
    return true;
  } catch {
    // 예전 방식: 숨긴 입력칸을 골라 복사
    const ta = document.createElement("textarea");
    ta.value = text;
    ta.style.position = "fixed";
    ta.style.opacity = "0";
    document.body.appendChild(ta);
    ta.select();
    let ok = false;
    try {
      ok = document.execCommand("copy");
    } catch {}
    ta.remove();
    return ok;
  }
}
