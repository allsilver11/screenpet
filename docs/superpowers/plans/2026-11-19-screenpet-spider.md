# 🕷️ 거미인간 (spider) — 구현 계획

설계: `docs/superpowers/specs/2026-11-19-screenpet-spider-design.md`. 순서마다 실패하는 시험 먼저.

1. **그림 규격** — `tools/sprite_spec.py`: OPTIONAL_ANIMS 끝에 `shoot`·`web_swing`·`hero_land`·`web_pull`, SPEECH·PERSONALITY·FAVORITE_FOOD·
   `WEB_SLINGER` 표 · `draw_pets.species_fields`에 `web_slinger`. 시험: `tools/test_draw_spider.py` 줄 순서.
2. **그림** — `tools/draw_spider.py` (draw_pets 엔진, 자기 포즈 전부, `register()`), `draw_pets.py` 끝에서 불러 등록.
   시험: 모든 칸 1px 여백·바닥선·다리 8개·옷 색·가슴 무늬·grip·저장소 그림과 같음. 생성 → PNG 눈으로 확인 → 미리보기.
   `python tools/draw_pixel_world.py --round-digest`로 해시 갱신. docs/sprites.md에 거미인간 절.
3. **코어 manifest** — `WEB_ANIMS`, `web_slinger`, `slings_webs()`, `GripTable` 세 칸, `PetConfig::slings_webs`. 번들 pet.json 시험.
4. **코어 web.rs** — 모서리·닻·진자·끌어오기 속도 (단위 시험).
5. **코어 brain_web.rs** — State::Web, 거미집·그네·끌어오기·영웅 착지, `take_web`·`web_line`·`ObjectPlay::Yank`·`be_yanked`
   (`brain_web_tests.rs`). 다른 종 시험(전 종 순회)이 그대로 통과하는지.
6. **앱** — `trail.rs` 줄·거미집·치우기, `objects.rs` Yank, `game_loop.rs` 연결. 소스 검사 시험 (`tools/test_web_ui.py`).
7. **UI** — `ui/doodle.js`(덮개 창 아래 캔버스)에 거미집·줄 그리기, `ui/main.js` 대사 기본값, `ui/mailbox.js` 🕷️, help.html·README.
   node 하네스(`tools/web_harness.js`)로 그리기·흐려짐·치우기.
8. 관련 pytest만, `cargo test -p screenpet-core`, `cargo check -p screenpet-core --target x86_64-pc-windows-gnu`, 커밋.
