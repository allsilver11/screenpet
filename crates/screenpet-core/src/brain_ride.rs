//! 기대고 있는 창 — brain.rs의 자식 모듈 (비공개 필드에 접근).
//! 전체 창 목록은 100 ms마다만 다시 읽으므로, 게임 루프는 사이 틱마다 이 창의 경계만 새로 읽어
//! (WindowSource::rect_of → World::patch_windows) 창을 끌어 옮길 때 동물이 매 틱 따라가게 한다.

use super::*;

impl Pet {
    /// 지금 자리가 달린 창: 윗면·안쪽 바닥에 서 있거나, 옆면·안쪽 벽에 매달렸거나, 창 안에서 노는 중이거나, 그 창으로 뛰는 중.
    /// 하우스 안·친구 화면에 가 있으면 None
    pub fn ridden_window(&self) -> Option<u64> {
        if self.housed || self.is_away() {
            return None;
        }
        match self.state {
            State::WallClimb { window, .. } | State::InnerClimb { window, .. } => return Some(window),
            State::Leap { owner: Owner::Window(id) | Owner::Inside(id), .. } => return Some(id),
            _ => {}
        }
        match self.standing.map(|s| s.owner) {
            Some(Owner::Window(id) | Owner::Inside(id)) => Some(id),
            _ => self.inside_window(),
        }
    }
}
