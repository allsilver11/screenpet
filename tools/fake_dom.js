// 웹뷰 없이 ui 스크립트를 돌리는 하네스들(levelup_harness, nightcap_harness)이 같이 쓰는 가짜 DOM 요소.
class El {
  constructor(id) {
    this.id = id;
    this.style = { setProperty: (k, v) => (this.style[k] = v) };
    this.children = [];
    this.parentNode = null;
    this.dataset = {};
    this.hidden = false;
    this.clientWidth = 128;
    this.clientHeight = 164;
    this.offsetWidth = 128;
    this.classes = new Set();
    this.handlers = {};
    this.textContent = "";
    const self = this;
    this.classList = { add: (...c) => c.forEach((x) => self.classes.add(x)), remove: (...c) => c.forEach((x) => self.classes.delete(x)), contains: (c) => self.classes.has(c) };
  }
  set innerHTML(_) {
    this.children = ["nk-body", "nk-tile", "nk-head"].map((n) => {
      const c = new El(n);
      c.parentNode = this;
      return c;
    });
  }
  append(...xs) {
    for (const x of xs) {
      if (x.parentNode) x.parentNode.children = x.parentNode.children.filter((c) => c !== x);
      x.parentNode = this;
      this.children.push(x);
    }
  }
  appendChild(x) {
    this.append(x);
  }
  insertBefore(x) {
    this.append(x);
  }
  replaceChildren(...xs) {
    this.textContent = xs.map((x) => x.textContent).join("");
  }
  after() {}
  focus() {}
  remove() {
    if (this.parentNode) this.parentNode.children = this.parentNode.children.filter((c) => c !== this);
    this.parentNode = null;
  }
  toggleAttribute(name, on) {
    if (name === "hidden") this.hidden = on;
  }
  setAttribute() {}
  addEventListener(ev, f) {
    this.handlers[ev] = f;
  }
  getBoundingClientRect() {
    return { left: 0, top: 0, width: 34, height: 26, bottom: 26, right: 34 };
  }
  querySelector() {
    return new El("q");
  }
  querySelectorAll() {
    return [];
  }
  getContext() {
    return { drawImage() {}, clearRect() {}, fillRect: () => (this.filled = (this.filled ?? 0) + 1), getImageData: () => ({ data: new Uint8ClampedArray(128 * 128 * 4) }) };
  }
}

// 요소를 id로 하나씩만 만든다 (document.getElementById 대신)
function elementStore() {
  const els = new Map();
  return (id) => {
    if (!els.has(id)) els.set(id, new El(id));
    return els.get(id);
  };
}

module.exports = { El, elementStore };
