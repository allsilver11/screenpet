//! 🐾 ScreenPet 핵심 크레이트 — 창도 웹뷰도 모르는 순수 규칙 (동물 두뇌, 물리, 디딤판, 설정, 날씨, 친구 방문 등)
//! 앱(src-tauri)과 screenpet-net·screenpet-relay가 씀. OS에 닿는 것은 platform 모듈뿐임
pub mod away;
pub mod behavior;
pub mod brain;
pub mod broom;
pub mod care;
pub mod collide;
pub mod config;
pub mod cursor_rally;
pub mod dodge;
pub mod flicker;
pub mod flight;
pub mod food;
pub mod geom;
pub mod hop;
pub mod icon;
pub mod linked;
pub mod inside;
pub mod juggle;
pub mod ladder;
pub mod manifest;
pub mod object;
pub mod own_pets;
pub mod pacing;
pub mod physics;
pub mod play;
pub mod platform;
pub mod poop;
pub mod presence;
pub mod real_weather;
pub mod sleigh;
pub mod snowball;
pub mod snowplay;
pub mod social;
pub mod stage;
pub mod surfaces;
pub mod taskbar;
pub mod timeline;
pub mod tint;
pub mod trail;
pub mod tumble;
pub mod typhoon;
pub mod typing;
pub mod update;
/// 🚗 탈것 (자동차·헬리콥터·킥보드·픽업 트럭)
pub mod vehicle;
pub mod visit;
pub mod weather;
pub mod web;
/// 🦇 박쥐인간 오토바이·박쥐 신호의 숫자
pub mod bike;
/// 🦸 슈퍼인간 낮게 날기·활공의 숫자
pub mod soar;
/// 🧹 마법사 빗자루 비행·🟡 골든 스니치의 숫자와 길
pub mod broom_fly;
/// 🧙 마법사 주문·📖 책·👜 가방 청소의 숫자
pub mod wizard;
/// ⚔️ 길고 치열한 결투 한 판 짜기 (합·저주·맞음·피하기)
pub mod duel_bout;
/// 🌗 루모스·녹스로 화면 밝기 바꾸기 (OS 라이트·다크 모드 잠깐 — 되돌리기·표시 파일·횟수 제한)
pub mod lumos;
/// ✨ 덮개 창에 그릴 마법 효과 (⚡ 빛줄기·⚔️ 결투·🐍 나기니·🦌 사슴·🥶 서리)
pub mod magic;
pub mod world;

#[cfg(test)]
mod test_util;
