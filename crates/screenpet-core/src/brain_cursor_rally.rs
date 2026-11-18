//! ⚾🖱️ 커서와 캐치볼 (v0.6.5) — brain.rs의 자식 모듈. 숫자와 시계는 cursor_rally.rs, 흐물이가 받는 길은 brain_rally.rs (Incoming.user).
//!
//! - 흐물이: 사용자가 던진 것을 받으면 (catch_object) 커서 쪽으로 되던짐 (Plan::ToCursor) — 놓을 때 CursorRallyEvent::Back으로 횟수를 알림
//! - 손 없는 동물: 사용자가 던진 것에 맞을 때 가끔 머리로 콩 받아침 (social.rs가 can_boop·boop을 부름) — "boop! 2"
//! - 먼저 놀자고 하기: 사용자가 있고 (앱이 set_cursor_rally_ok) 바닥에서 손이 비었고 근처에 물건이 있으면 가끔 —
//!   흐물이는 주워 커서 쪽으로 던지고, 다른 동물은 툭 밀어 커서 쪽으로 날림 ("catch!"). 이 첫 던지기가 1. 받아 주지 않으면 오래 쉼
//! 주사위는 이 동물만 자기 주사위(CursorRallyMood.rng)로 굴림

use super::juggle_moves::Plan;
use super::*;
use crate::cursor_rally::{self, CursorRallyEvent, Invites};
use crate::juggle::{ObjectSpot, MAX_RALLY};

/// 콩 받아치거나 툭 밀 때 고개를 갸웃하는 시간 (초)
const BOOP_SECS: f64 = 0.6;

/// 커서와 캐치볼의 기억
#[derive(Debug, Clone)]
pub(super) struct CursorRallyMood {
    rng: fastrand::Rng,
    invites: Invites,
    /// 커서와 놀아도 됨 (사용자가 있고, 무엇을 들고 있지 않음 — 앱이 매 틱)
    ok: bool,
    /// 놀자고 주우러 가는 물건
    invite_id: Option<u64>,
    /// 커서 쪽으로 던질 물건과 실을 횟수 (놓을 때 Back으로)
    back: Option<(u64, u32)>,
    event: Option<CursorRallyEvent>,
}

impl CursorRallyMood {
    /// 시드로 난수를 따로 둠
    pub(super) fn new(seed: u64) -> Self {
        CursorRallyMood { rng: fastrand::Rng::with_seed(seed ^ 0x6375_7273), invites: Invites::default(), ok: false, invite_id: None, back: None, event: None }
    }
}

impl Pet {
    /// 매 틱: 커서와 놀아도 되는지 (사용자가 있고 커서 장난이 켜져 있고 아무것도 들고 있지 않음)
    pub fn set_cursor_rally_ok(&mut self, ok: bool) {
        self.cursor_rally.ok = ok;
    }

    /// 앱에 알릴 일 (한 번만)
    pub fn take_cursor_rally(&mut self) -> Option<CursorRallyEvent> {
        self.cursor_rally.event.take()
    }

    /// 지금 커서 자리 (물리 px — hover가 매 틱 채움)
    pub fn cursor_pos(&self) -> (f64, f64) {
        self.cursor
    }

    /// 손 없는 동물이 날아온 것을 콩 받아칠 수 있음: 깨어 바닥에 서 있고, 조종·볼일이 없음 (흐물이는 받으니까 아님)
    pub fn can_boop(&self) -> bool {
        !self.cfg.juggler
            && self.standing.is_some()
            && self.control.is_none()
            && !self.has_errand()
            && !self.presence_quiet()
            && !self.is_held()
            && !matches!(self.state, State::Sleep { .. } | State::Annoyed { .. })
    }

    /// 콩 받아침 (n번째): "boop! n", MAX_RALLY면 "Done!". 사용자와 논 것이니 놀자고 하기는 평소 간격으로
    pub fn boop(&mut self, n: u32) {
        self.hit_calm_until = self.clock + crate::collide::HIT_CALM_SECS;
        self.wobble_timer = stomp_moves::WOBBLE_SECS;
        if n >= MAX_RALLY {
            self.speech = Some(Speech::RallyEnd);
        } else {
            self.speech_text = Some(format!("boop! {n}"));
        }
        let clock = self.clock;
        self.cursor_rally.invites.answered(clock, &mut self.cursor_rally.rng);
    }

    /// 흐물이가 사용자가 던진 것을 받았음 (brain_rally.rs catch_object): 커서 쪽으로 n번째로 되던짐
    pub(super) fn caught_from_cursor(&mut self, id: u64, n: u32) {
        self.juggle.plan = Plan::ToCursor;
        self.cursor_rally.back = Some((id, n));
        self.turn_to_cursor();
        let clock = self.clock;
        self.cursor_rally.invites.answered(clock, &mut self.cursor_rally.rng);
    }

    /// 커서 쪽을 봄
    fn turn_to_cursor(&mut self) {
        let dx = self.cursor.0 - self.body.x;
        if dx.abs() > 1.0 {
            self.facing = dx.signum();
        }
    }

    /// 주운 물건이 놀자고 주우러 간 것이면 커서 쪽으로 던질 계획 (brain_juggle.rs start_lift)
    pub(super) fn invite_plan(&mut self, id: u64) -> Option<Plan> {
        self.cursor_rally.invite_id.take().filter(|&i| i == id)?;
        self.cursor_rally.back = Some((id, 1));
        self.turn_to_cursor();
        Some(Plan::ToCursor)
    }

    /// 손에서 놓았음 (brain_juggle.rs release): 커서 쪽으로 던진 것이었으면 Back으로 알림
    pub(super) fn cursor_released(&mut self, id: u64) {
        if let Some((got, n)) = self.cursor_rally.back.take() {
            if got == id {
                self.cursor_rally.event = Some(CursorRallyEvent::Back { id, rally: n });
                if n == 1 {
                    self.speech = Some(Speech::Invite);
                }
            }
        }
    }

    /// 커서 쪽으로 던지는 속도 (물리 px/s, Plan::ToCursor)
    pub(super) fn to_cursor_velocity(&self, from: (f64, f64)) -> (f64, f64) {
        cursor_rally::to_cursor_velocity(from, self.cursor, self.cfg.size, self.cfg.dpi)
    }

    /// 매 틱 (notice_objects 뒤, update 전): 놀자고 할 때면 근처 물건으로 — 흐물이는 주우러 가고, 다른 동물은 툭 밈
    pub fn notice_cursor_rally(&mut self, spots: &[ObjectSpot], world: &World) {
        let clock = self.clock;
        let mood = &mut self.cursor_rally;
        mood.invites.tick(clock, &mut mood.rng);
        if mood.invite_id.is_some() && !matches!(self.state, State::Juggle { .. }) {
            // 주우러 가다 그만뒀다
            self.cursor_rally.invite_id = None;
        }
        if !self.cursor_rally.ok || !self.free_hands() || self.cursor_rally.invite_id.is_some() || self.guest.is_some() {
            return;
        }
        let mood = &mut self.cursor_rally;
        if !mood.invites.due(clock, &mut mood.rng) {
            return;
        }
        let Some(st) = self.standing else { return };
        let reach = if self.cfg.juggler { cursor_rally::INVITE_REACH } else { cursor_rally::BOOP_REACH } * self.cfg.size;
        let (x, y) = (self.body.x, self.body.y);
        let near = spots
            .iter()
            .filter(|s| !s.giant && !s.local && s.owner == st.owner && (s.y - y).abs() <= crate::juggle::SAME_LEVEL_PX && (s.x - x).abs() <= reach)
            .min_by(|a, b| (a.x - x).abs().total_cmp(&(b.x - x).abs()))
            .copied();
        let Some(spot) = near else { return };
        let mood = &mut self.cursor_rally;
        if mood.rng.f64() >= cursor_rally::INVITE_CHANCE {
            mood.invites.skip(clock, &mut mood.rng);
            return;
        }
        if self.cfg.juggler {
            let Some(to) = self.approach_x(&spot, world) else { return };
            self.cursor_rally.invite_id = Some(spot.id);
            self.cursor_rally.invites.invited(clock);
            self.start_approach(&spot, to);
            return;
        }
        // 손 없는 동물: 물건 쪽을 보고 고개를 갸웃하며 툭 — 물건이 커서 쪽으로 날아감
        self.cursor_rally.invites.invited(clock);
        self.facing = if spot.x >= x { 1.0 } else { -1.0 };
        let from = (spot.x, spot.y - spot.half_w);
        let (vx, vy) = self.to_cursor_velocity(from);
        self.cursor_rally.event = Some(CursorRallyEvent::Boop { id: spot.id, vx, vy, rally: 1 });
        self.speech = Some(Speech::Invite);
        self.pacing.after_beat = None;
        self.clear_hover();
        self.state = State::Trick { kind: Trick::Tilt, remaining: BOOP_SECS };
    }
}
