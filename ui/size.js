// 크기 조절 창: 움직이는 동안 바로 보여 주고, 놓거나 기본값을 누르면 저장한다.
const { invoke } = window.__TAURI__.core;
const here = window.__TAURI__.webviewWindow.getCurrentWebviewWindow();

const petEl = document.getElementById("pet");
const objectEl = document.getElementById("object");
const petValue = document.getElementById("pet-value");
const objectValue = document.getElementById("object-value");
const petReset = document.getElementById("pet-reset");
const objectReset = document.getElementById("object-reset");
const resetEl = document.getElementById("reset");

let defaultPct = 100;

function paint() {
  petValue.textContent = `${petEl.value}%`;
  objectValue.textContent = `${objectEl.value}%`;
  petReset.disabled = Number(petEl.value) === defaultPct;
  objectReset.disabled = Number(objectEl.value) === defaultPct;
  resetEl.disabled = petReset.disabled && objectReset.disabled;
}

function send(save) {
  paint();
  invoke("size_set", { pet: Number(petEl.value), object: Number(objectEl.value), save });
}

function show(view) {
  defaultPct = view.default;
  for (const el of [petEl, objectEl]) {
    el.min = view.min;
    el.max = view.max;
  }
  petEl.value = view.pet;
  objectEl.value = view.object;
  paint();
}

function resetTo(...els) {
  for (const el of els) el.value = defaultPct;
  send(true);
}

for (const el of [petEl, objectEl]) {
  el.addEventListener("input", () => send(false));
  el.addEventListener("change", () => send(true));
}
petReset.addEventListener("click", () => resetTo(petEl));
objectReset.addEventListener("click", () => resetTo(objectEl));
resetEl.addEventListener("click", () => resetTo(petEl, objectEl));
document.getElementById("close").addEventListener("click", () => invoke("size_close"));

setupFormKeys({ close: () => invoke("size_close") });

(async () => {
  // 듣기 시작한 뒤에 지금 상태를 묻는다
  await here.listen("size-state", (e) => show(e.payload));
  show(await invoke("size_state"));
})();
