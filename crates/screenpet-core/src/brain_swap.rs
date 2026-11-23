//! 🔄 동물 종류 바꾸기 (트레이 "동물 바꾸기" — 앱은 두뇌는 그대로 두고 cfg만 갈아 끼운 뒤 set_cursor_species를 부름).
//! 하던 일이 그 종만의 것 (흐물이 줄행랑·들어 올리기, 벽 타기, 빗자루, 결투 …)이면 그만두고 쉬거나 떨어짐 —
//! 새 동물엔 그 그림이 없어 웹뷰가 조용히 idle로 바꿔 그리거나 (선 채 미끄러짐) 그 상태가 끝나지 않고 멈춰 있던 버그 (qa_sim)

use super::*;

impl Pet {
    /// 종이 바뀌었음 (from → 새 cfg): 그 종만의 일을 그만두고, 들고 있던 물건을 놓음. 어느 동물이나 하는 상태 (쉬기·잠·걷기·공중·잡힘·먹기·
    /// 드나들기·인사 놀이 …)는 그대로 둠
    pub(super) fn swap_species(&mut self) {
        if self.carried_object().is_some() {
            self.lose_carry();
        }
        // 🦌🦦 패트로누스를 못 부르는 동물이 됐으면 달리던 은빛 동물도 사라짐
        if self.cfg.patronus.is_none() {
            self.duel.stag = None;
        }
        let keep = matches!(
            self.state,
            State::Move { .. }
                | State::Idle { .. }
                | State::Sleep { .. }
                | State::Turn { .. }
                | State::Land { .. }
                | State::Trick { .. }
                | State::Tickled { .. }
                | State::Annoyed { .. }
                | State::Air { .. }
                | State::Pressed { .. }
                | State::Dragged
                | State::Hungry { .. }
                | State::Eating { .. }
                | State::LevelUp { .. }
                | State::GoHome
                | State::HomeLeap { .. }
                | State::Housed
                | State::Exit { .. }
                | State::Enter { .. }
                | State::Farewell { .. }
                | State::Away
                | State::ToFood { .. }
                | State::WantFood { .. }
                | State::Leap { .. }
                | State::Greet { .. }
                | State::Chase { .. }
                | State::Flee { .. }
                | State::CatchMe { .. }
                | State::Tumble { .. }
                | State::Lifted { .. }
                | State::OnHead { .. }
                | State::Gloom { .. }
                | State::Faint { .. }
                | State::Drained { .. }
                | State::Float { .. }
                | State::Tucked { .. }
                | State::Portkey { .. }
        );
        if keep {
            // 😾 고양이만의 "흥" 인사 그림은 아무 동물에나 있는 갸웃으로
            if let State::Greet { look, anim: Anim::Huff, remaining } = self.state {
                self.state = State::Greet { look, anim: Anim::Tilt, remaining };
            }
            return;
        }
        self.pacing.after_beat = None;
        self.flight = None;
        if self.standing.is_some() || self.cfg.flies {
            self.state = self.idle_state();
        } else {
            self.start_fall();
        }
    }
}
