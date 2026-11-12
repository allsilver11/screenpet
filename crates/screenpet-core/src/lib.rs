//! 🐾 ScreenPet 핵심 크레이트 — 창도 웹뷰도 모르는 순수 규칙 (동물 두뇌, 물리, 디딤판, 설정, 날씨, 친구 방문 등)
//! 앱(src-tauri)과 screenpet-net·screenpet-relay가 씀. OS에 닿는 것은 platform 모듈뿐임
pub mod behavior;
pub mod brain;
pub mod broom;
pub mod care;
pub mod collide;
pub mod config;
pub mod dodge;
pub mod flicker;
pub mod flight;
pub mod food;
pub mod geom;
pub mod gift;
pub mod hop;
pub mod icon;
pub mod linked;
pub mod inside;
pub mod juggle;
pub mod ladder;
pub mod manifest;
pub mod object;
pub mod pacing;
pub mod physics;
pub mod play;
pub mod platform;
pub mod poop;
pub mod presence;
pub mod real_weather;
pub mod sleigh;
pub mod social;
pub mod style;
pub mod surfaces;
pub mod taskbar;
pub mod timeline;
pub mod tint;
pub mod trail;
pub mod tumble;
pub mod typing;
pub mod update;
pub mod visit;
pub mod weather;
pub mod world;

#[cfg(test)]
mod test_util;
