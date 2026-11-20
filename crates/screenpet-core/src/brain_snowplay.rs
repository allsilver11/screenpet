//! ⛄🍂 눈·낙엽 더미에서 놀기 (v0.6.5) — brain.rs의 자식 모듈 (비공개 필드에 접근). 숫자와 고르기는 snowplay.rs, 창은 src-tauri.
//!
//! 앱이 매 틱 발밑 더미와 같은 화면의 다른 동물을 알려 주면 (notice_pile), 바닥에서 쉬는 박자가 끝날 때 가끔 (PLAY_EVERY_SECS에 한 번쯤):
//! - ⛄ 눈: 기지개 켜듯 발밑 눈을 떠 뭉쳐서 던짐 ("snowball!") — 다른 동물·커서·친구 화면 너머(내 동물만)·아무 데나.
//!   눈덩이에 맞은 동물은 REVENGE_SECS 안에 쉬면 거의 늘 되던짐 (눈싸움)
//! - ⛄ 눈: 가끔 눈밭에서 데굴 ("fluffy!", 못 구르는 동물은 배를 보이며) → 눈가루
//! - 🍂 낙엽: 폴짝 높이 뛰어들어 내려앉은 자리의 낙엽이 확 흩어짐 ("whee!")
//! 그림은 있는 것만 씀 (기지개·구르기·배·폴짝). 주사위는 이 동물만 자기 주사위(SnowPlayMood.rng)로 굴림 — 다른 주사위 순서는 그대로.

use super::*;
use crate::juggle::Target;
use crate::snowplay::{self, Aim, Play, SnowPlayEvent};
use crate::weather::Weather;

/// 던지기·뛰어들기 사이에 기다리는 일
#[derive(Debug, Clone, Copy, PartialEq)]
enum Pending {
    /// 기지개 켜며 눈을 뭉치는 중: at에 던지고, until이 지나면 그만둠. revenge = 맞아서 되던짐
    Throw { at: f64, until: f64, revenge: bool },
    /// 낙엽에 뛰어드는 중: 내려서면 흩어짐 (until이 지나면 그만둠)
    Dive { until: f64 },
}

/// 눈·낙엽 놀이의 기억
#[derive(Debug, Clone)]
pub(super) struct SnowPlayMood {
    pub(super) rng: fastrand::Rng,
    /// 발밑 더미 (그림, 평균 높이 — 논리 px)
    pile: Option<(Weather, f32)>,
    /// 같은 화면의 다른 동물 (맞을 수 있는 것만)
    targets: Vec<Target>,
    /// 커서를 노려도 됨 (앱이 장난을 막지 않음)
    cursor_ok: bool,
    /// 이때부터 다시 놂 (동물 시계)
    next_at: f64,
    /// 눈덩이에 맞아 이때까지 쉬면 되던짐
    revenge_until: f64,
    pending: Option<Pending>,
    event: Option<SnowPlayEvent>,
}

impl SnowPlayMood {
    /// 시드로 난수를 따로 둠
    pub(super) fn new(seed: u64) -> Self {
        SnowPlayMood { rng: fastrand::Rng::with_seed(seed ^ 0x736e_6f77), pile: None, targets: Vec::new(), cursor_ok: false, next_at: 0.0, revenge_until: 0.0, pending: None, event: None }
    }
}

impl Pet {
    /// 매 틱 (update 전): 발밑 더미 (없으면 None), 같은 화면의 다른 동물, 커서를 노려도 되는지
    pub fn notice_pile(&mut self, pile: Option<(Weather, f32)>, targets: &[Target], cursor_ok: bool) {
        self.snowplay.pile = pile;
        self.snowplay.targets.clear();
        self.snowplay.targets.extend_from_slice(targets);
        self.snowplay.cursor_ok = cursor_ok;
    }

    /// 앱에 알릴 일 (한 번만)
    pub fn take_snow_play(&mut self) -> Option<SnowPlayEvent> {
        self.snowplay.event.take()
    }

    /// 눈덩이에 맞았음: 곧 쉬면 되던짐
    pub(super) fn snowball_hit(&mut self) {
        self.snowplay.revenge_until = self.clock + snowplay::REVENGE_SECS;
    }

    /// 더미에서 놀 수 있음: 걷는 동물이 바닥(창 윗면 포함)에 서 있고, 볼일·든 것이 없음
    fn can_snow_play(&self) -> bool {
        !self.cfg.flies
            && self.standing.is_some()
            && self.inside.is_none()
            && !self.housed
            && !self.has_errand()
            && !self.presence_quiet()
            && !self.is_heading_out()
            && self.juggle.carry.is_none()
            && self.snowplay.pending.is_none()
    }

    /// 쉬는 박자가 끝날 때: 가끔 더미에서 놂 (맞았으면 거의 늘 되던짐)
    pub(super) fn snow_play_rest(&mut self) -> Option<State> {
        let (kind, depth) = self.snowplay.pile?;
        if !self.can_snow_play() {
            return None;
        }
        let revenge = self.clock < self.snowplay.revenge_until && kind.is_snow() && depth >= snowplay::MIN_DEPTH;
        let mood = &mut self.snowplay;
        let play = if revenge {
            mood.revenge_until = 0.0;
            if mood.rng.f64() >= snowplay::REVENGE_CHANCE {
                return None;
            }
            Play::Throw
        } else {
            if self.clock < mood.next_at {
                return None;
            }
            mood.next_at = self.clock + behavior::range(&mut mood.rng, snowplay::PLAY_EVERY_SECS);
            if mood.rng.f64() >= snowplay::PLAY_CHANCE {
                return None;
            }
            snowplay::pick_play(&mut mood.rng, kind, depth)?
        };
        mood.next_at = mood.next_at.max(self.clock + snowplay::PLAY_EVERY_SECS.0);
        self.start_snow_play(play, revenge);
        Some(self.state)
    }

    /// 놀이를 시작함 (상태를 바꾸고, 던지기·뛰어들기는 기다리는 일로 둠)
    fn start_snow_play(&mut self, play: Play, revenge: bool) {
        self.pacing.after_beat = None;
        self.clear_hover();
        match play {
            Play::Throw => {
                self.snowplay.pending = Some(Pending::Throw { at: self.clock + snowplay::SCOOP_RELEASE, until: self.clock + snowplay::SCOOP_SECS + 0.5, revenge });
                self.state = State::Trick { kind: Trick::Stretch, remaining: snowplay::SCOOP_SECS };
            }
            Play::Flop => {
                self.snowplay.event = Some(SnowPlayEvent::Burst { x: self.body.x, y: self.body.y });
                self.speech = Some(Speech::Flop);
                let rolls = !self.cfg.no_roll;
                self.state = if rolls {
                    State::Move { kind: Move::Roll, remaining: snowplay::FLOP_SECS }
                } else {
                    State::Trick { kind: Trick::Belly, remaining: snowplay::FLOP_SECS }
                };
            }
            Play::Dive => {
                let dir = if self.snowplay.rng.bool() { 1.0 } else { -1.0 };
                self.facing = dir;
                self.start_hop(dir);
                self.body.vy *= snowplay::DIVE_LIFT;
                self.snowplay.pending = Some(Pending::Dive { until: self.clock + 3.0 });
                self.speech = Some(Speech::Whee);
            }
        }
    }

    /// update 끝에: 뭉친 눈을 던질 때·뛰어든 뒤 내려섰는지 봄. 하던 것이 끊겼으면 (잡혔다·놀랐다 등) 그만둠
    pub(super) fn tick_snow_play(&mut self, world: &World) {
        let Some(pending) = self.snowplay.pending else { return };
        match pending {
            Pending::Throw { at, until, revenge } => {
                if self.clock > until || !matches!(self.state, State::Trick { kind: Trick::Stretch, .. }) {
                    self.snowplay.pending = None;
                } else if self.clock >= at {
                    self.snowplay.pending = None;
                    self.throw_snowball(world, revenge);
                }
            }
            Pending::Dive { until } => {
                if self.clock > until || self.is_held() {
                    self.snowplay.pending = None;
                } else if self.standing.is_some() && !matches!(self.state, State::Air { .. }) {
                    self.snowplay.pending = None;
                    self.snowplay.event = Some(SnowPlayEvent::Burst { x: self.body.x, y: self.body.y });
                }
            }
        }
    }

    /// 뭉친 눈을 던짐: 노릴 곳을 고르고 그쪽을 봄
    fn throw_snowball(&mut self, world: &World, revenge: bool) {
        let size = self.cfg.size;
        let from = (self.body.x, self.body.y - 0.9 * size);
        let foot = (self.body.x, self.body.y);
        let cursor = Some(self.cursor).filter(|&c| self.snowplay.cursor_ok && snowplay::cursor_in_reach(foot, size, c));
        // 🎯 친구 화면 너머로는 내 동물만 (손님은 주인 화면 쪽 끝이 따로 있음)
        let mut edges: Vec<f64> = Vec::new();
        if self.guest.is_none() {
            let b = world.bounds;
            for (open, x) in [(self.exit_edges.left, b.left), (self.exit_edges.right, b.right)] {
                if open && snowplay::lob_reaches(from.0, x, self.cfg.dpi) {
                    edges.push(x);
                }
            }
        }
        let mood = &mut self.snowplay;
        let aim = snowplay::pick_aim(&mut mood.rng, from, size, &mood.targets, cursor, &edges, revenge);
        let (vx, vy) = snowplay::aim_velocity(&mut mood.rng, aim, from, size, self.cfg.dpi);
        if !matches!(aim, Aim::Toss(_)) && vx != 0.0 {
            self.facing = vx.signum();
        }
        self.snowplay.event = Some(SnowPlayEvent::Throw { foot_x: foot.0, foot_y: foot.1, x: from.0, y: from.1, vx, vy });
        self.speech = Some(Speech::Snowball);
    }
}

#[cfg(test)]
impl Pet {
    /// 테스트: 지금 바로 놀이를 시작함
    pub(crate) fn force_snow_play(&mut self, play: Play) {
        self.start_snow_play(play, false);
    }
}
