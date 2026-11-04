// 사다리타기 창 (spec docs/superpowers/specs/2026-11-04-screenpet-ladder-design.md).
// 백엔드(게임 루프의 src-tauri/src/ladder.rs)가 화면 상태를 `ladder-state`로 보내고, 버튼은 ladder_* 커맨드로 답한다.
// 결과(순열·가로줄·길)는 백엔드가 시드에서 이미 만들어 보낸다 — 여기서는 그리고 움직이기만 한다.
// 맨 위 LADDER는 DOM 없는 계산만 모은 것 (tools/ladder_harness.js가 node로 불러 시험한다).
// 일반 스크립트라 common.js의 최상위 이름(copyText, frameIndex …)과 겹치지 않게 LADDER와 즉시 실행 함수 안에만 둔다.

const LADDER = (() => {
  const MAX_PLAYERS = 10;
  const RESULT_MAX_CHARS = 12;
  const NAME_CHARS = 6;
  // 이름표: 11px 글꼴에서 한글 한 자 ≈ 11px, 좌우 여백·테두리 ≈ 12px. 홀짝 칸이 두 줄로 엇갈려 한 이름표가 두 칸 폭까지 쓴다
  const TAG_CHAR_PX = 11;
  const TAG_PAD_PX = 12;
  const TAG_ROW_PX = 18;
  const SLOT_CHARS = 6;
  const MOOD_EMOJI = { good: "😀", bad: "😢", none: "·" };
  const MOODS = ["good", "bad", "none"];
  // 참가자별 길 색 (10명)
  const PALETTE = ["#e8735a", "#4f9de0", "#5fb86a", "#c77dd8", "#e0a02f", "#3fb7b0", "#e06a9c", "#8a8fe8", "#9aa63a", "#b5835a"];

  // 글자 수(코드 포인트)로 자른다
  function ellipsize(s, max) {
    const chars = Array.from(s ?? "");
    return chars.length <= max ? chars.join("") : chars.slice(0, max).join("") + "…";
  }

  // 결과 칸 글자: 앞뒤 공백 없이, 제어 문자 없이, 12자까지 (코어 ladder::clean_result와 같다) — 안 맞으면 null
  function cleanText(raw) {
    const s = (raw ?? "").trim();
    if (!s || /[\u0000-\u001f\u007f-\u009f]/.test(s) || Array.from(s).length > RESULT_MAX_CHARS) return null;
    return s;
  }

  // 미리 채우기: 당첨 k개 + 꽝 나머지 (당첨 수는 인원을 넘지 않는다)
  function presetSlots(count, winners) {
    const w = Math.max(0, Math.min(count, Math.floor(winners) || 0));
    return Array.from({ length: count }, (_, i) => (i < w ? { text: "당첨", mood: "good" } : { text: "꽝", mood: "bad" }));
  }

  // 직접 적은 칸을 인원에 맞춘다: 모자라면 빈 칸을 덧붙이고, 남으면 **지우지 않는다** (방장이 고른다 — spec §1-3)
  function growSlots(slots, count) {
    const out = slots.map((s) => ({ ...s }));
    while (out.length < count) out.push({ text: "", mood: "none" });
    return out;
  }

  // 칸 문제: { extra: n } 남는 칸 / { blank: [i…] } 비었거나 맞지 않는 글자 / null 괜찮음
  function slotProblem(slots, count) {
    if (slots.length > count) return { extra: slots.length - count };
    if (slots.length < count) return { missing: count - slots.length };
    const blank = slots.map((s, i) => (cleanText(s.text) == null ? i : -1)).filter((i) => i >= 0);
    return blank.length ? { blank } : null;
  }

  function slotWarning(p) {
    if (!p) return "";
    if (p.extra) return `칸이 ${p.extra}개 많아요 — 지울 칸을 골라 주세요 (✕)`;
    if (p.missing) return `칸이 ${p.missing}개 모자라요`;
    return "빈 칸이 있어요 (12자까지)";
  }

  function nextMood(m) {
    return MOODS[(MOODS.indexOf(m) + 1) % MOODS.length];
  }

  // 받침 있으면 a, 없으면 b (한글이 아니면 "a(b)")
  function josa(word, a, b) {
    const c = word.charCodeAt(word.length - 1);
    if (c >= 0xac00 && c <= 0xd7a3) return word + ((c - 0xac00) % 28 ? a : b);
    return `${word}${a}(${b})`;
  }

  const ANSWERS = {
    waiting: ["기다리는 중", ""],
    joined: ["참가", "joined"],
    no: ["안 한대요", "out"],
    busy: ["바빠요", "out"],
    playing: ["다른 판 중", "out"],
    timeout: ["답 없음", "out"],
    left: ["나감", "out"],
  };
  function answerLabel(a) {
    return ANSWERS[a] ?? ["", ""];
  }

  // 취소 안내 (누가 일부러 그랬다고 탓하지 않는다 — spec §3.4)
  function cancelText(c) {
    const who = c?.who;
    switch (c?.reason) {
      case "host":
        return "방장이 사다리타기를 끝냈어요";
      case "nobody":
        return "아무도 참가하지 않았어요";
      case "left":
        return who ? `${josa(who, "과", "와")} 연결이 끊겨 취소됐어요` : "연결이 끊겨 취소됐어요";
      case "timeout":
        return who ? `${who}에게서 답이 오지 않아 취소됐어요` : "시간이 지나 취소됐어요";
      default:
        return "사다리 정보가 맞지 않아 취소됐어요";
    }
  }

  // 칸 너비에 들어가는 이름표 글자 수 (2~6자, 줄이면 붙는 "…" 한 자 몫을 뺀다)
  function nameChars(col) {
    return Math.max(2, Math.min(NAME_CHARS, Math.floor((2 * col - 2 - TAG_PAD_PX) / TAG_CHAR_PX) - 1));
  }

  // 이름표 어림 너비 (줄였을 때 — 글자 + "…")
  function tagWidth(col) {
    return (nameChars(col) + 1) * TAG_CHAR_PX + TAG_PAD_PX;
  }

  // 이름표 줄 (짝수 칸 아래 줄, 홀수 칸 위 줄 — 이웃 이름표가 겹치지 않게)
  function tagLift(i) {
    return i % 2 ? TAG_ROW_PX : 0;
  }

  // 게임 판 배치 (CSS px): 칸 가운데 x, 사다리 위·아래 y, 줄마다 y, 펫 크기
  function layout(game, width, height) {
    const n = game.players.length;
    const col = game.column;
    const left = (width - col * n) / 2;
    const pet = Math.min(48, col - 8);
    const petTop = 4 + 2 * TAG_ROW_PX + 4;
    const top = petTop + pet + 6;
    const slotTop = height - 40 - 46;
    const bottom = slotTop - 8;
    const rows = game.rows.length;
    return {
      xs: Array.from({ length: n }, (_, i) => left + col * (i + 0.5)),
      top,
      bottom,
      rowYs: Array.from({ length: rows }, (_, r) => top + ((r + 1) * (bottom - top)) / (rows + 1)),
      slotTop,
      pet,
      petTop,
      col,
    };
  }

  // 위 칸 i의 길: 꼭짓점 목록 { x, y } (세로 → 가로 → 세로 …, 마지막은 사다리 아래)
  function route(game, lay, i) {
    const path = game.paths[i];
    // 펫이 앉아 있던 자리에서 출발한다 (사다리 꼭대기로 미끄러져 내려간다)
    const pts = [{ x: lay.xs[path[0]], y: lay.petTop + lay.pet / 2 }, { x: lay.xs[path[0]], y: lay.top }];
    lay.rowYs.forEach((y, r) => {
      pts.push({ x: lay.xs[path[r]], y });
      if (path[r + 1] !== path[r]) pts.push({ x: lay.xs[path[r + 1]], y });
    });
    pts.push({ x: lay.xs[path[path.length - 1]], y: lay.bottom });
    return pts;
  }

  function length(pts) {
    let d = 0;
    for (let i = 1; i < pts.length; i++) d += Math.hypot(pts[i].x - pts[i - 1].x, pts[i].y - pts[i - 1].y);
    return d;
  }

  // 길 위 거리 d의 점과 지금 구간 방향 ("down" | "left" | "right"), 지나온 꼭짓점
  function pointAt(pts, d) {
    let left = d;
    for (let i = 1; i < pts.length; i++) {
      const a = pts[i - 1];
      const b = pts[i];
      const seg = Math.hypot(b.x - a.x, b.y - a.y);
      if (left <= seg || i === pts.length - 1) {
        const t = seg ? Math.min(1, left / seg) : 1;
        const p = { x: a.x + (b.x - a.x) * t, y: a.y + (b.y - a.y) * t };
        const dir = b.x > a.x ? "right" : b.x < a.x ? "left" : "down";
        return { p, dir, trail: [...pts.slice(0, i), p] };
      }
      left -= seg;
    }
    return { p: pts[0], dir: "down", trail: [pts[0]] };
  }

  // 결과 표 (이름 → 결과): 위 칸 순서
  function resultRows(game) {
    return game.players.map((p, i) => {
      const end = game.paths[i][game.paths[i].length - 1];
      const slot = game.results[end];
      return { name: p.name, result: slot.text, mood: slot.mood, me: i === game.me };
    });
  }

  // 결과 복사 글자 (채팅방에 붙여 넣으면 지문도 같이 비교된다)
  function copyLines(host, print, rows) {
    const head = `🎲 ${host}의 사다리타기 · 지문 ${print}`;
    return [head, ...rows.map((r) => `${r.name} → ${r.result}${r.mood === "good" ? " 😀" : r.mood === "bad" ? " 😢" : ""}`)].join("\n");
  }

  // 초대 화면은 키로 답하지 않는다 (포커스를 안 가져가는 창이지만, 혹시 키가 와도)
  function ignoreKey(view, key) {
    return view === "invite" && (key === "Enter" || key === " " || key === "Spacebar");
  }

  return {
    MAX_PLAYERS, NAME_CHARS, SLOT_CHARS, MOOD_EMOJI, PALETTE, TAG_CHAR_PX, TAG_PAD_PX, TAG_ROW_PX, nameChars, tagWidth, tagLift,
    ellipsize, cleanText, presetSlots, growSlots, slotProblem, slotWarning, nextMood, josa,
    answerLabel, cancelText, layout, route, length, pointAt, resultRows, copyLines, ignoreKey,
  };
})();

if (typeof module !== "undefined") module.exports = LADDER;

if (typeof window !== "undefined" && window.__TAURI__) {
  (() => {
    const { invoke } = window.__TAURI__.core;
    const here = window.__TAURI__.webviewWindow.getCurrentWebviewWindow();
    const $ = (id) => document.getElementById(id);
    const views = { setup: $("v-setup"), invite: $("v-invite"), small: $("v-small"), game: $("v-game") };
    const SPEED = 150; // 펫이 사다리를 따라가는 속도 (px/s)
    const START_AFTER_MS = 1500; // 방장 신호가 안 와도 이만큼 뒤 시작

    let state = null; // 백엔드가 보낸 화면
    // 방장 편집 상태 (설정 → 초대 중까지 이어진다)
    // begunFor = 시작을 요청한 참가 인원 (인원이 바뀌면 다시 시작해 본다 — 백엔드가 칸 수가 안 맞아 거절했을 수 있다)
    const host = { selected: new Set(), mode: "preset", winners: 1, slots: [], begunFor: null };

    function el(tag, props = {}, ...kids) {
      const e = document.createElement(tag);
      Object.assign(e, props);
      for (const k of kids) e.append(k);
      return e;
    }

    function showOnly(name) {
      for (const [k, v] of Object.entries(views)) v.hidden = k !== name;
    }

    // ---- 방장: 친구 고르기 + 결과 칸 ----
    function slotCount() {
      if (!state) return 1;
      if (state.view === "setup") return host.selected.size + 1;
      // 초대 중: 아직 올 수 있는 사람까지 (참가 + 기다리는 중) + 방장
      const maybe = state.friends.filter((f) => f.answer === "joined" || f.answer === "waiting").length;
      return maybe + 1;
    }

    function syncSlots(count) {
      if (host.mode === "preset") host.slots = LADDER.presetSlots(count, host.winners);
      else host.slots = LADDER.growSlots(host.slots, count);
    }

    function renderSlots(listEl, count) {
      syncSlots(count);
      // 직접 적는 중에 친구 답이 와도 글자 칸을 다시 만들지 않는다 (쓰던 글자·커서가 날아가지 않게).
      // 단 칸이 남게 되면(누가 거절) ✕를 보여야 하므로 다시 그린다
      const extra = host.slots.length > count;
      const typing = listEl.contains(document.activeElement) && listEl.children.length === host.slots.length;
      if (typing && listEl.dataset.extra === String(extra)) return;
      listEl.dataset.extra = String(extra);
      listEl.replaceChildren();
      host.slots.forEach((s, i) => {
        const text = el("input", { type: "text", value: s.text, maxLength: 12, placeholder: `${i + 1}번 칸`, disabled: host.mode === "preset" });
        text.addEventListener("input", () => {
          host.slots[i].text = text.value;
          updateWarn();
        });
        const mood = el("button", { className: "mood", textContent: LADDER.MOOD_EMOJI[s.mood], title: "좋음 😀 / 나쁨 😢 / 없음 ·", disabled: host.mode === "preset" });
        mood.addEventListener("click", () => {
          host.slots[i].mood = LADDER.nextMood(host.slots[i].mood);
          mood.textContent = LADDER.MOOD_EMOJI[host.slots[i].mood];
        });
        const li = el("li", {}, text, mood);
        if (extra) {
          const x = el("button", { className: "small", textContent: "✕", title: "이 칸 지우기" });
          x.addEventListener("click", () => {
            host.slots.splice(i, 1);
            render();
          });
          li.append(x);
        }
        listEl.append(li);
      });
    }

    function updateWarn() {
      if (!state || (state.view !== "setup" && state.view !== "inviting")) return;
      const count = slotCount();
      const p = LADDER.slotProblem(host.slots, count);
      $("setup-warn").textContent = LADDER.slotWarning(p);
      const btn = $("invite");
      if (state.view === "setup") {
        btn.disabled = host.selected.size === 0 || !!p;
      } else {
        // 시작: 지금 참가한 사람 + 방장 = 칸 수일 때 (기다리는 사람은 시작하면 빠진다)
        const startCount = state.joined + 1;
        const startProblem = LADDER.slotProblem(host.mode === "preset" ? LADDER.presetSlots(startCount, host.winners) : host.slots, startCount);
        btn.disabled = state.joined === 0 || !!startProblem;
        if (!p && startProblem && state.joined > 0 && host.mode === "custom") {
          $("setup-warn").textContent = `지금 시작하면 ${startCount}명이에요 — 칸을 ${startCount}개로 맞춰 주세요`;
        }
      }
    }

    function renderSetup() {
      showOnly("setup");
      const inviting = state.view === "inviting";
      // 골라 둔 친구가 끊겼거나 초대할 수 없게 됐으면 고른 목록에서도 뺀다 (안 보이는 친구가 칸 수·인원 제한을 차지하지 않게)
      if (!inviting) {
        const can = new Set(state.friends.filter((f) => f.can_invite).map((f) => f.id));
        for (const id of [...host.selected]) if (!can.has(id)) host.selected.delete(id);
      }
      views.setup.querySelector("h1").textContent = inviting ? "🎲 친구들의 답을 기다리는 중" : "🎲 사다리타기";
      views.setup.querySelector(".note").textContent = inviting
        ? `참가 ${state.joined}명 · 모두 답하면 바로 시작해요`
        : "같이 할 친구를 고르세요 (방장 포함 10명까지)";
      const list = $("friends");
      list.replaceChildren();
      if (!state.friends.length) list.append(el("li", { className: "dim", textContent: "연결된 친구가 없어요" }));
      for (const f of state.friends) {
        const li = el("li");
        const name = el("span", { className: "grow", textContent: f.name });
        if (inviting) {
          const [label, cls] = LADDER.answerLabel(f.answer);
          li.append(name, el("span", { className: `tag ${cls}`, textContent: label }));
        } else if (f.can_invite) {
          const box = el("input", { type: "checkbox", checked: host.selected.has(f.id) });
          box.addEventListener("change", () => {
            if (box.checked && host.selected.size >= LADDER.MAX_PLAYERS - 1) {
              box.checked = false;
              return;
            }
            box.checked ? host.selected.add(f.id) : host.selected.delete(f.id);
            render();
          });
          li.append(box, name);
        } else {
          // 예전 판 친구: 사다리 메시지를 받으면 연결이 끊기므로 초대할 수 없다
          li.className = "dim";
          li.title = "친구가 ScreenPet을 새 버전으로 업데이트하면 같이 할 수 있어요";
          li.append(el("input", { type: "checkbox", disabled: true }), name, el("span", { className: "tag", textContent: "업데이트 필요" }));
        }
        list.append(li);
      }
      for (const r of document.querySelectorAll('input[name="mode"]')) r.checked = r.value === host.mode;
      if (document.activeElement !== $("winners")) $("winners").value = host.winners;
      $("winners").disabled = host.mode !== "preset";
      renderSlots($("slots"), slotCount());
      $("invite").textContent = inviting ? "시작" : "초대";
      $("setup-close").textContent = inviting ? "그만두기" : "닫기";
      updateWarn();
      const notice = {
        busy: "집중 중이거나 펫이 하우스 안이라 초대할 수 없어요",
        nobody: "고른 친구가 모두 연결이 끊겼어요",
        slots: "결과 칸을 확인해 주세요 (빈 칸 없이 12자까지)",
        invalid: "초대할 수 없어요 — 내 이름을 확인해 주세요",
      }[state.notice];
      if (notice && !inviting) $("setup-warn").textContent = notice;
      // 모두 답했고 칸이 맞으면 저절로 시작 (칸이 안 맞으면 방장이 고칠 때까지 기다린다)
      if (inviting && state.all_answered && state.joined > 0 && host.begunFor !== state.joined) {
        const count = state.joined + 1;
        if (host.mode === "preset") host.slots = LADDER.presetSlots(count, host.winners);
        if (!LADDER.slotProblem(host.slots, count)) begin();
      }
    }

    function begin() {
      const count = state.joined + 1;
      const slots = host.mode === "preset" ? LADDER.presetSlots(count, host.winners) : host.slots;
      if (LADDER.slotProblem(slots, count)) return;
      host.begunFor = state.joined;
      invoke("ladder_begin", { results: slots.map((s) => ({ text: LADDER.cleanText(s.text), mood: s.mood })) });
    }

    for (const r of document.querySelectorAll('input[name="mode"]')) {
      r.addEventListener("change", () => {
        if (!r.checked) return;
        // 직접 적기로 바꾸면 미리 채운 칸에서 시작한다
        host.mode = r.value;
        render();
      });
    }
    // 숫자를 지우고 새로 칠 수 있게: 칠 때마다 다시 그리지 않고, 다 치면(change) 반영한다
    $("winners").addEventListener("change", () => {
      host.winners = Math.max(1, Math.min(9, parseInt($("winners").value, 10) || 1));
      render();
    });
    $("invite").addEventListener("click", () => {
      if (state?.view === "setup") {
        const count = host.selected.size + 1;
        if (LADDER.slotProblem(host.slots, count)) return;
        host.begunFor = null;
        invoke("ladder_host", { friends: [...host.selected], results: host.slots.map((s) => ({ text: LADDER.cleanText(s.text), mood: s.mood })) });
      } else if (state?.view === "inviting") {
        begin();
      }
    });
    $("setup-close").addEventListener("click", () => invoke("ladder_close"));

    // ---- 참가자: 초대 ----
    function renderInvite() {
      showOnly("invite");
      $("invite-title").textContent = `🎲 ${state.host_name}의 사다리타기 — 참가할래요?`;
      $("invite-count").textContent = `${state.invited + 1}명까지 · 결과 칸`;
      const list = $("invite-slots");
      list.replaceChildren();
      for (const s of state.draft) {
        list.append(el("span", { className: "chip", textContent: `${s.text} ${LADDER.MOOD_EMOJI[s.mood] ?? "·"}` }));
      }
    }
    $("join").addEventListener("click", () => invoke("ladder_answer", { join: true }));
    $("decline").addEventListener("click", () => invoke("ladder_answer", { join: false }));

    // ---- 짧은 안내 ----
    function renderSmall(title, note, button) {
      showOnly("small");
      $("small-title").textContent = title;
      $("small-note").textContent = note;
      $("small-close").textContent = button;
    }
    $("small-close").addEventListener("click", () => invoke("ladder_close"));

    // ---- 게임 ----
    let game = null; // { view, lay, runners, order, started, finished }
    const sheets = new Map(); // "pet|style" → loadPetSheet 결과

    async function sheetFor(pet, style) {
      const key = `${pet}|${style}`;
      if (!sheets.has(key)) {
        const want = style === "pixel" ? "pixel" : "round";
        sheets.set(
          key,
          loadPetSheet(pet, "baby", want).catch(() => loadPetSheet("cat", "baby", want)),
        );
      }
      return sheets.get(key);
    }

    function animFor(sheet, name) {
      const a = sheet.manifest.animations;
      return a[name] ?? a.idle;
    }

    function paintRunner(r, now) {
      const { sheet, side } = r;
      if (!sheet) return;
      const spec = animFor(sheet, r.anim);
      const f = frameIndex(spec, now - r.animStart);
      r.el.style.backgroundPosition = `${-f * side}px ${-spec.row * side}px`;
      const flip = r.dir === "left" && !["slide", "level_up", "annoyed", "sit_front"].includes(r.anim);
      r.el.style.transform = `translate(${r.p.x - side / 2}px, ${r.p.y - side / 2}px)${flip ? " scaleX(-1)" : ""}`;
      r.tag.style.left = `${r.p.x}px`;
      r.tag.style.top = `${r.p.y - side / 2 - LADDER.TAG_ROW_PX - (r.phase === "wait" ? LADDER.tagLift(r.i) : 0)}px`;
    }

    function setAnim(r, name, now) {
      if (r.anim !== name) {
        r.anim = name;
        r.animStart = now;
      }
    }

    async function renderGame() {
      const g = state.game;
      if (game?.view.game_id === g.game_id) return; // 같은 판을 다시 받으면 그대로 둔다
      showOnly("game");
      $("table").hidden = true;
      const board = $("board");
      board.querySelectorAll(".runner, .nametag, .slot").forEach((e) => e.remove());
      const svg = $("lines");
      svg.replaceChildren();
      const ns = "http://www.w3.org/2000/svg";
      const vlines = g.players.map(() => {
        const line = document.createElementNS(ns, "line");
        Object.entries({ stroke: "#c9b79f", "stroke-width": 3, "stroke-linecap": "round" }).forEach(([k, v]) => line.setAttribute(k, v));
        svg.append(line);
        return line;
      });
      const slots = g.results.map((s) => {
        const slot = el("div", { className: `slot ${s.mood}`, textContent: LADDER.ellipsize(s.text, LADDER.SLOT_CHARS) });
        slot.title = s.text;
        board.append(slot);
        return slot;
      });
      const pixel = g.players.some((p) => p.style === "pixel");
      setPixelBody(pixel);
      const pet = Math.min(48, g.column - 8);
      const runners = g.players.map((p, i) => {
        const trail = document.createElementNS(ns, "polyline");
        Object.entries({ fill: "none", stroke: LADDER.PALETTE[i % LADDER.PALETTE.length], "stroke-width": 4, "stroke-linejoin": "round", "stroke-linecap": "round", opacity: 0.85 }).forEach(([k, v]) => trail.setAttribute(k, v));
        svg.append(trail);
        const runner = el("div", { className: "runner" });
        const tag = el("div", { className: `nametag${i === g.me ? " me" : ""}`, textContent: LADDER.ellipsize(p.name, LADDER.nameChars(g.column)), title: p.name });
        tag.style.borderColor = LADDER.PALETTE[i % LADDER.PALETTE.length];
        board.append(runner, tag);
        const r = { i, el: runner, tag, trail, pts: [], total: 0, d: 0, p: { x: 0, y: 0 }, dir: "down", anim: "sit_front", animStart: performance.now(), phase: "wait", side: pet, sheet: null };
        sheetFor(p.pet, p.style).then((sheet) => {
          const side = sheet.style === "pixel" ? snapPixelSide(pet, sheet.manifest.frame_size) : pet;
          const k = side / sheet.manifest.frame_size;
          r.sheet = sheet;
          r.side = side;
          runner.style.width = runner.style.height = `${side}px`;
          runner.style.backgroundImage = `url(${sheet.url})`;
          runner.style.backgroundSize = `${sheet.img.width * k}px ${sheet.img.height * k}px`;
        });
        return r;
      });
      game = { view: g, lay: null, runners, vlines, slots, next: 0, started: false, finished: 0, all: false };
      placeBoard();
      $("game-note").textContent = `${g.host_name}의 사다리타기`;
      $("all").hidden = !!g.skip;
      if (g.skip) {
        // 집중 중·하우스 안: 진행 없이 결과 표만
        showTable();
        return;
      }
      setTimeout(startRun, startPending ? 0 : START_AFTER_MS);
      startPending = false;
      requestAnimationFrame(tick);
    }

    // 판 배치를 지금 창 크기에 맞춘다. 게임 화면이 뜰 때 창은 아직 이전 화면(초대·대기) 크기일 수 있다 —
    // 백엔드가 창을 키우면 resize가 오고, 내려가던 펫은 진행한 비율 그대로 새 판에서 이어 간다
    function placeBoard() {
      const g = game.view;
      const W = window.innerWidth;
      const H = window.innerHeight;
      const lay = LADDER.layout(g, W, H);
      game.lay = lay;
      const svg = $("lines");
      svg.setAttribute("width", W);
      svg.setAttribute("height", H);
      game.vlines.forEach((line, i) => {
        Object.entries({ x1: lay.xs[i], y1: lay.top, x2: lay.xs[i], y2: lay.bottom }).forEach(([k, v]) => line.setAttribute(k, v));
      });
      game.slots.forEach((slot, i) => {
        slot.style.left = `${lay.xs[i]}px`;
        slot.style.top = `${lay.slotTop}px`;
        slot.style.width = `${lay.col - 6}px`;
      });
      const now = performance.now();
      for (const r of game.runners) {
        const frac = r.total ? r.d / r.total : 0;
        r.pts = LADDER.route(g, lay, r.i);
        r.total = LADDER.length(r.pts);
        if (r.phase === "run") {
          r.d = frac * r.total;
          r.t0 = now - (r.d / SPEED) * 1000;
          const at = LADDER.pointAt(r.pts, r.d);
          r.p = at.p;
          r.trail.setAttribute("points", at.trail.map((q) => `${q.x},${q.y}`).join(" "));
        } else if (r.phase === "done") {
          r.d = r.total;
          const end = g.paths[r.i][g.paths[r.i].length - 1];
          r.p = { x: lay.xs[end], y: lay.slotTop - r.side / 2 - 2 };
          r.trail.setAttribute("points", r.pts.map((q) => `${q.x},${q.y}`).join(" "));
        } else {
          r.p = { x: r.pts[0].x, y: r.pts[0].y };
        }
        paintRunner(r, now);
      }
    }

    window.addEventListener("resize", () => {
      if (game && !views.game.hidden) placeBoard();
    });

    // 방장 신호가 게임 화면보다 먼저 오면 기억했다가 화면이 뜨면 바로 시작한다
    let startPending = false;

    function startRun() {
      if (!game) {
        startPending = true;
        return;
      }
      if (game.started) return;
      game.started = true;
      launchNext();
    }

    function launchNext() {
      const r = game.runners.find((x) => x.phase === "wait");
      if (!r) return;
      r.phase = "run";
      r.t0 = performance.now();
    }

    $("all").addEventListener("click", () => {
      if (!game) return;
      game.started = true;
      game.all = true;
      const now = performance.now();
      for (const r of game.runners) {
        if (r.phase === "wait") {
          r.phase = "run";
          r.t0 = now;
        }
      }
      $("all").hidden = true;
    });

    function tick(now) {
      if (!game || views.game.hidden) return;
      for (const r of game.runners) {
        if (r.phase === "run") {
          r.d = Math.min(r.total, ((now - r.t0) / 1000) * SPEED);
          const at = LADDER.pointAt(r.pts, r.d);
          r.p = at.p;
          r.dir = at.dir;
          const vertical = r.sheet?.manifest.animations.slide ? "slide" : "fall";
          setAnim(r, at.dir === "down" ? vertical : "walk", now);
          r.trail.setAttribute("points", at.trail.map((q) => `${q.x},${q.y}`).join(" "));
          if (r.d >= r.total) arrive(r, now);
        }
        paintRunner(r, now);
      }
      requestAnimationFrame(tick);
    }

    function arrive(r, now) {
      r.phase = "done";
      const g = game.view;
      const end = g.paths[r.i][g.paths[r.i].length - 1];
      const slot = g.results[end];
      r.p = { x: game.lay.xs[end], y: game.lay.slotTop - r.side / 2 - 2 };
      setAnim(r, slot.mood === "good" ? "level_up" : slot.mood === "bad" ? "annoyed" : "sit_front", now);
      document.querySelectorAll(".slot")[end]?.classList.add("hit");
      if (r.i === g.me) invoke("ladder_seen");
      game.finished += 1;
      if (game.finished === game.runners.length) {
        $("all").hidden = true;
        // 그사이 창을 닫았거나 다른 판으로 바뀌었으면 보이지 않는다
        const id = game.view.game_id;
        setTimeout(() => {
          if (game?.view.game_id === id && !views.game.hidden) showTable();
        }, 1400);
      } else if (!game.all) {
        launchNext();
      }
    }

    function showTable(rows, hostName, print) {
      const g = game?.view;
      rows = Array.isArray(rows) ? rows : LADDER.resultRows(g);
      hostName = hostName ?? g.host_name;
      print = print ?? g.print;
      $("table-title").textContent = `🎲 ${hostName}의 사다리타기 결과`;
      const list = $("table-rows");
      list.replaceChildren();
      for (const r of rows) {
        const name = el("span", { className: "grow", textContent: r.name });
        if (r.me) name.style.fontWeight = "700";
        list.append(el("li", {}, name, el("span", { textContent: `${r.result} ${LADDER.MOOD_EMOJI[r.mood] ?? ""}` })));
      }
      $("table-print").textContent = print;
      $("copy").onclick = async () => {
        const ok = await copyText(LADDER.copyLines(hostName, print, rows));
        $("copy").textContent = ok ? "복사했어요" : "복사 실패";
        setTimeout(() => ($("copy").textContent = "결과 복사"), 1500);
      };
      $("table").hidden = false;
    }
    $("game-close").addEventListener("click", () => invoke("ladder_close"));

    function renderLast() {
      showOnly("game");
      game = null;
      $("board").querySelectorAll(".runner, .nametag, .slot").forEach((e) => e.remove());
      $("lines").replaceChildren();
      $("all").hidden = true;
      $("game-note").textContent = "";
      const last = state.last;
      if (!last) return;
      showTable(last.rows, last.host, last.print);
    }

    // ---- 화면 고르기 ----
    function render() {
      if (!state) return;
      switch (state.view) {
        case "setup":
        case "inviting":
          renderSetup();
          break;
        case "invite":
          renderInvite();
          break;
        case "joined":
          renderSmall("🙋 참가했어요!", "다른 친구들을 기다리는 중…", "그만두기");
          break;
        case "locked":
          renderSmall("🎲 사다리를 정하는 중…", "모두의 숫자를 모아 사다리를 만들어요", "그만두기");
          break;
        case "cancelled":
          game = null;
          renderSmall("🎲 사다리타기가 취소됐어요", LADDER.cancelText(state.cancel), "닫기");
          break;
        case "game":
          renderGame();
          break;
        case "last":
          renderLast();
          break;
      }
    }

    function apply(v) {
      if (!v) return;
      // 새 판의 설정 화면이면 방장 편집 상태를 처음부터
      if (v.view === "setup" && state?.view !== "setup") {
        host.selected.clear();
        host.mode = "preset";
        host.winners = 1;
        host.slots = [];
        host.begunFor = null;
      }
      // 진행 화면으로 가는 길(잠김·게임)이 아니면 앞 판의 늦은 시작 신호를 버린다
      if (v.view !== "locked" && v.view !== "game") startPending = false;
      state = v;
      render();
    }

    here.listen("ladder-start", startRun);
    window.addEventListener("contextmenu", (e) => e.preventDefault());
    window.addEventListener(
      "keydown",
      (e) => {
        if (LADDER.ignoreKey(state?.view, e.key)) {
          e.preventDefault();
          e.stopPropagation();
          return;
        }
        // 글자 칸 밖에서는 웹뷰 기본 동작(새로 고침 등)을 막는다
        if (e.target.tagName !== "INPUT" && e.key !== "Tab") e.preventDefault();
      },
      true,
    );

    (async () => {
      await here.listen("ladder-state", (e) => apply(e.payload));
      apply(await invoke("ladder_state"));
    })();
  })();
}
