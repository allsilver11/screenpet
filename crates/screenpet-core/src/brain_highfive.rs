//! 🙌 하이파이브 (v0.6.3, 모든 동물)의 Pet 동작 — brain.rs의 자식 모듈. 짝 고르기는 social.rs (마주 오다 만남), 숫자는 play.rs.
//!
//! 마주 오다 만난 둘이 HIGH_FIVE_CHANCE로: 손이 닿을 만큼 다가가 (Close) 마주 보고 (Ready — 둘 다 준비되면 코디네이터가 뛰게 한다)
//! 폴짝 뛰어 "High five!" (Jump — 가운데에서 render.clap: 웹뷰가 앞쪽 위에 반짝이를 터뜨린다, 둘의 반짝이가 손 사이에서 만난다),
//! 내려선다 (Land). 새 그림 없이 hop·land 그림으로.

use super::*;
use crate::play;

/// 하이파이브 단계 (State::HighFive — t는 이 단계에 들어선 뒤 흐른 시간)
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HighFivePhase {
    /// 짝 쪽으로 x까지 다가간다
    Close { x: f64, toward: f64 },
    /// 마주 보고 기다린다
    Ready,
    /// 폴짝 — 짝!
    Jump,
    Land,
}

/// 짝을 기다리다 이만큼 지나면 혼자라도 뛴다 (짝이 사라졌을 때는 코디네이터가 먼저 풀어 준다)
const READY_WAIT_SECS: f64 = 2.5;

impl Pet {
    /// 🙌 하이파이브를 시작한다: x에 서서 toward(짝 쪽 ±1)를 본다. speak면 뛸 때 "High five!"
    pub fn start_high_five(&mut self, x: f64, toward: f64, speak: bool) {
        self.pacing.after_beat = None;
        self.clear_hover();
        self.body.vx = 0.0;
        self.facing = toward;
        self.play.speak_five = speak;
        // 뒤로 물러서야 하면 (이미 가깝다) 그 자리에서
        let x = if (x - self.body.x) * toward > 0.0 { x } else { self.body.x };
        self.state = State::HighFive { phase: HighFivePhase::Close { x, toward }, t: 0.0 };
    }

    /// 마주 보고 준비됐다 (잠깐 쳐다본 뒤)
    pub fn high_five_ready(&self) -> bool {
        matches!(self.state, State::HighFive { phase: HighFivePhase::Ready, t } if t >= play::FIVE_LOOK_SECS)
    }

    pub fn in_high_five(&self) -> bool {
        matches!(self.state, State::HighFive { .. })
    }

    /// 짝! — 둘이 같이 뛴다
    pub fn jump_high_five(&mut self) {
        if !matches!(self.state, State::HighFive { phase: HighFivePhase::Close { .. } | HighFivePhase::Ready, .. }) {
            return;
        }
        if std::mem::take(&mut self.play.speak_five) {
            self.speech = Some(Speech::HighFive);
        }
        self.state = State::HighFive { phase: HighFivePhase::Jump, t: 0.0 };
    }

    /// 짝이 사라졌다: 뛰기 전이면 그만둔다
    pub fn drop_high_five(&mut self) {
        if matches!(self.state, State::HighFive { phase: HighFivePhase::Close { .. } | HighFivePhase::Ready, .. }) {
            self.state = self.rest_beat();
        }
    }

    /// 짝! 반짝이 (뛰어 꼭대기 언저리)
    pub(super) fn clapping(&self) -> bool {
        matches!(self.state, State::HighFive { phase: HighFivePhase::Jump, t } if (t - play::FIVE_JUMP_SECS / 2.0).abs() <= play::CLAP_SECS / 2.0)
    }

    pub(super) fn update_high_five(&mut self, dt: f64, world: &World, phase: HighFivePhase, t: f64) {
        if self.romp_interrupted() {
            self.state = self.rest_beat();
            return;
        }
        if !self.follow_anchor(world) {
            self.start_fall();
            return;
        }
        let t = t + dt;
        match phase {
            HighFivePhase::Close { x, toward } => {
                let dx = x - self.body.x;
                if dx.abs() > 1.0 && t < play::FIVE_CLOSE_SECS {
                    match self.juggle_walk(world, dx.signum(), (self.walk_speed() * dt).min(dx.abs())) {
                        None => return,
                        Some(true) => self.state = State::HighFive { phase: HighFivePhase::Ready, t: 0.0 },
                        Some(false) => self.state = State::HighFive { phase, t },
                    }
                } else {
                    self.state = State::HighFive { phase: HighFivePhase::Ready, t: 0.0 };
                }
                self.facing = toward;
            }
            HighFivePhase::Ready => {
                self.state = State::HighFive { phase, t };
                if t >= READY_WAIT_SECS {
                    self.jump_high_five();
                }
            }
            HighFivePhase::Jump => {
                self.state = if t < play::FIVE_JUMP_SECS { State::HighFive { phase, t } } else { State::HighFive { phase: HighFivePhase::Land, t: 0.0 } };
            }
            HighFivePhase::Land => {
                self.state = if t < play::FIVE_LAND_SECS {
                    State::HighFive { phase, t }
                } else {
                    self.heart_timer = HEART_SECS;
                    self.rest_beat()
                };
            }
        }
    }
}
