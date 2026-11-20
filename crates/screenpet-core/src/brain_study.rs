//! 📚 헤르미온느 — 📖 책 읽기 · 🙋 "I know!" (pet.json `bookworm` + read·raise_hand 그림) · 👜 끝없는 구슬 가방 청소
//! (pet.json `bag_cleaner` + bag_walk·bag_drag·bag_stuff 그림) — brain.rs의 자식 모듈. 숫자는 wizard.rs.
//!
//! - 📖 한가할 때 꽤 자주 바닥에 앉아 책을 읽음 (State::Study Read — 가끔 "fascinating..."). 훅: is_reading (PetSpot.reading · 📖 공부 방해 social_pester.rs —
//!   해리·해그리드가 방해하면 shush "Shh! I'm studying!" 하고 계속 읽거나 close_book "Oh, alright!" 하고 같이 놂)
//! - 🙋 가끔 손을 번쩍 들고 "I know!" (Know, 앞모습)
//! - 👜 (내 헤르미온느만 — 손님은 친구 물건을 치우지 않음) 같은 디딤판에 바닥의 물건 (똥·쓰레기·거대한 것 — 📌 장난감·사용자가 잡은 것·거미집에 걸린 것은 바닥에 없으니 안 보임)이
//!   BAG_MIN개 넘게 있으면 가끔 "tidy time!" 하고 구슬 가방을 꺼내 가까운 것부터 하나씩 다가가 (Approach) 쏙 넣음 (Stuff — 앱이 뿅 치움, ObjectPlay::Vanish).
//!   넣을수록 가방이 커지고 (render.bag = wizard::bag_growth — 웹뷰가 손 자리 grip에 그림) 커지면 질질 끌며 느려짐 (bag_drag 그림, wizard::bag_pace).
//!   더 없으면 "Phew! All clean!" (Done) 하고 가방이 뿅 사라짐. 끼어들리면 (잡힘 등) 가방은 그대로 두었다가 한가해지면 마저 치움
//!
//! 주사위는 자기 주사위(StudyMood.rng)로 굴림 — 평소 동작과 다른 동물의 주사위 순서는 그대로.

use super::*;
use crate::juggle::ObjectSpot;
use crate::play::ObjectPlay;
use crate::wizard as wz;

/// 👜 가방 청소 단계
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BagPhase {
    /// 물건(id) 옆 x로 걸어감 (obj_x = 물건 가운데)
    Approach { id: u64, x: f64, obj_x: f64 },
    /// 가방에 쏙 (gulped = 이미 넣었음)
    Stuff { id: u64, gulped: bool },
    /// "Phew! All clean!" — 끝나면 가방이 사라짐
    Done,
}

/// 책벌레의 하는 일 (State::Study)
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum StudyAct {
    /// 📖 앉아서 len초 동안 책 읽기
    Read { len: f64 },
    /// 🙋 손 번쩍 "I know!"
    Know,
    /// 👜 가방 청소
    Bag(BagPhase),
}

/// 책·손들기·가방 청소의 기억
#[derive(Debug, Clone)]
pub struct StudyMood {
    rng: fastrand::Rng,
    /// 📖 이 시각(clock)부터 책을 펼 수 있음
    pub(super) read_ready_at: f64,
    /// 👜 이 시각(clock)부터 가방 청소를 할 수 있음
    pub(super) bag_ready_at: f64,
    /// 👜 가방을 꺼냈음 (청소 중 — 끼어들려도 남음)
    pub(super) bag_on: bool,
    /// 👜 가방에 넣은 물건 수
    pub(super) bag_items: u32,
    /// 👜 이번 청소에서 못 닿아 건너뛴 물건
    pub(super) skipped: Vec<u64>,
}

impl StudyMood {
    /// 시드로 난수를 따로 둠
    pub(super) fn new(seed: u64) -> Self {
        StudyMood {
            rng: fastrand::Rng::with_seed(seed ^ 0x7374_7564),
            read_ready_at: wz::READ_FIRST_SECS,
            bag_ready_at: wz::BAG_FIRST_SECS,
            bag_on: false,
            bag_items: 0,
            skipped: Vec::new(),
        }
    }
}

impl Pet {
    /// 📖 앉아서 책을 읽는 중 (다른 펫이 봄 — PetSpot.reading)
    pub fn is_reading(&self) -> bool {
        matches!(self.state, State::Study { act: StudyAct::Read { .. }, .. })
    }

    /// 👜 웹뷰가 그릴 가방 크기 (0 = 안 보임): 청소하는 동안 · 🎒 해리를 숨긴 결투 동안 (brain_tuck.rs)
    pub(super) fn bag_shown(&self) -> f64 {
        if let State::Duel { duel, .. } = self.state {
            return if duel.bag { wz::HIDE_BAG } else { 0.0 };
        }
        if self.study.bag_on && matches!(self.state, State::Study { act: StudyAct::Bag(_), .. }) {
            wz::bag_growth(self.study.bag_items)
        } else {
            0.0
        }
    }

    /// 하는 일별 그림
    pub(super) fn study_anim(&self, act: StudyAct) -> Anim {
        match act {
            StudyAct::Read { .. } => Anim::Read,
            StudyAct::Know => Anim::RaiseHand,
            StudyAct::Bag(BagPhase::Approach { .. }) if wz::bag_growth(self.study.bag_items) >= wz::BAG_DRAG_AT => Anim::BagDrag,
            StudyAct::Bag(BagPhase::Approach { .. }) => Anim::BagWalk,
            StudyAct::Bag(BagPhase::Stuff { .. }) => Anim::BagStuff,
            StudyAct::Bag(BagPhase::Done) => Anim::Idle,
        }
    }

    /// 매 틱 (주문 뒤): 하는 중이면 그 한 틱, 아니면 가끔 가방 청소·책·손들기. 처리했으면 true
    pub(super) fn update_study(&mut self, dt: f64, world: &World) -> bool {
        if !self.cfg.bookworm && !self.cfg.bag_cleaner {
            return false;
        }
        if let State::Study { act, t } = self.state {
            self.tick_study(dt, world, act, t + dt);
            return true;
        }
        if !self.play_free() || self.romp_interrupted() || self.is_user_away() {
            return false;
        }
        // 👜 끼어들렸던 청소를 마저
        if self.study.bag_on {
            self.next_bag_item(world);
            return true;
        }
        let power = self.power();
        if self.cfg.bag_cleaner && self.guest.is_none() && self.clock >= self.study.bag_ready_at && self.study.rng.f64() < wz::BAG_RATE * power * dt && self.bag_candidates(world).len() >= wz::BAG_MIN {
            return self.start_bag(world);
        }
        if !self.cfg.bookworm {
            return false;
        }
        if self.clock >= self.study.read_ready_at && self.study.rng.f64() < wz::READ_RATE * dt {
            return self.start_reading();
        }
        if self.study.rng.f64() < wz::KNOW_RATE * power * dt {
            return self.start_know();
        }
        false
    }

    /// 📖 그 자리에 앉아 책을 폄 (서 있을 때만)
    pub fn start_reading(&mut self) -> bool {
        if !self.cfg.bookworm || self.standing.is_none() {
            return false;
        }
        self.pacing.after_beat = None;
        self.clear_hover();
        let len = behavior::range(&mut self.study.rng, wz::READ_SECS);
        self.state = State::Study { act: StudyAct::Read { len }, t: 0.0 };
        true
    }

    /// 📖 친구가 방해함 (social_pester.rs): "Shh! I'm studying!" 하고 계속 읽음
    pub fn shush(&mut self) {
        if self.is_reading() {
            self.say_now(Speech::Shh);
        }
    }

    /// 📖 친구가 방해함 (social_pester.rs): 책을 덮고 "Oh, alright!" — 일어나 같이 놀 준비 (다음 책은 READ_CALM_SECS 뒤)
    pub fn close_book(&mut self) {
        if !self.is_reading() {
            return;
        }
        self.study.read_ready_at = self.clock + behavior::range(&mut self.study.rng, wz::READ_CALM_SECS);
        self.say_now(Speech::OhAlright);
        self.state = State::Idle { remaining: 1.0, look: false };
    }

    /// 🙋 손 번쩍 "I know!"
    pub(super) fn start_know(&mut self) -> bool {
        if !self.cfg.bookworm || self.standing.is_none() {
            return false;
        }
        self.pacing.after_beat = None;
        self.clear_hover();
        self.speech = Some(Speech::IKnow);
        self.state = State::Study { act: StudyAct::Know, t: 0.0 };
        true
    }

    /// 👜 가방을 꺼내 "tidy time!" 하고 가까운 것부터 치우러 감
    pub(super) fn start_bag(&mut self, world: &World) -> bool {
        if !self.cfg.bag_cleaner || self.standing.is_none() || self.bag_candidates(world).is_empty() {
            return false;
        }
        self.pacing.after_beat = None;
        self.clear_hover();
        self.study.bag_on = true;
        self.study.bag_items = 0;
        self.study.skipped.clear();
        self.speech = Some(Speech::TidyUp);
        self.next_bag_item(world);
        true
    }

    /// 👜 치울 수 있는 물건: 같은 디딤판 (같은 높이)에 놓였고 다가갈 수 있는 것 — 📌 장난감과 건너뛴 것은 빼고, 거대한 것도
    fn bag_candidates(&self, world: &World) -> Vec<ObjectSpot> {
        let Some(st) = self.standing else { return Vec::new() };
        let y = self.body.y;
        self.play
            .spots
            .iter()
            .filter(|s| !s.local && s.owner == st.owner && (s.y - y).abs() <= crate::juggle::SAME_LEVEL_PX && !self.study.skipped.contains(&s.id))
            .filter(|s| self.approach_x(s, world).is_some())
            .copied()
            .collect()
    }

    /// 👜 가까운 다음 물건으로 — 없으면 (하나라도 넣었으면) "Phew! All clean!", 아무것도 못 넣었으면 그냥 가방을 넣음
    fn next_bag_item(&mut self, world: &World) {
        let x = self.body.x;
        let pick = self.bag_candidates(world).into_iter().min_by(|a, b| (a.x - x).abs().total_cmp(&(b.x - x).abs()));
        match pick.and_then(|s| self.approach_x(&s, world).map(|ax| (ax, s))) {
            Some((ax, s)) => self.state = State::Study { act: StudyAct::Bag(BagPhase::Approach { id: s.id, x: ax, obj_x: s.x }), t: 0.0 },
            None if self.study.bag_items > 0 => {
                self.speech = Some(Speech::AllClean);
                self.state = State::Study { act: StudyAct::Bag(BagPhase::Done), t: 0.0 };
            }
            None => self.end_bag(),
        }
    }

    /// 👜 가방을 넣고 끝 (다음 청소는 BAG_CALM_SECS 뒤)
    fn end_bag(&mut self) {
        self.study.bag_on = false;
        self.study.bag_items = 0;
        self.study.skipped.clear();
        self.study.bag_ready_at = self.clock + behavior::range(&mut self.study.rng, wz::BAG_CALM_SECS);
        self.state = self.rest_beat();
    }

    /// 책·손들기·가방 한 틱 (t = 이번 틱까지 흐른 시간)
    fn tick_study(&mut self, dt: f64, world: &World, act: StudyAct, t: f64) {
        if self.romp_interrupted() || self.is_user_away() {
            // 가방은 그대로 — 한가해지면 마저 치움
            self.state = self.idle_state();
            return;
        }
        if !self.follow_anchor(world) {
            self.start_fall();
            return;
        }
        match act {
            StudyAct::Read { len } => {
                if t >= len {
                    self.study.read_ready_at = self.clock + behavior::range(&mut self.study.rng, wz::READ_CALM_SECS);
                    self.state = self.rest_beat();
                    return;
                }
                if self.speech.is_none() && self.study.rng.f64() < wz::STUDY_LINE_RATE * dt {
                    self.speech = Some(Speech::Study);
                }
                self.state = State::Study { act, t };
            }
            StudyAct::Know => {
                self.state = if t < wz::KNOW_SECS { State::Study { act, t } } else { self.rest_beat() };
            }
            StudyAct::Bag(phase) => self.tick_bag(dt, world, phase, t),
        }
    }

    /// 👜 가방 청소 한 틱: 다가가 넣고, 다음 것으로
    fn tick_bag(&mut self, dt: f64, world: &World, phase: BagPhase, t: f64) {
        match phase {
            BagPhase::Approach { id, .. } => {
                // 물건이 아직 바닥에 있나 (치워졌거나 누가 들었거나 날아갔으면 다음 것)
                let Some(spot) = self.play.spots.iter().find(|s| s.id == id).copied() else {
                    self.next_bag_item(world);
                    return;
                };
                let Some(x) = self.approach_x(&spot, world) else {
                    self.study.skipped.push(id);
                    self.next_bag_item(world);
                    return;
                };
                if t > wz::BAG_APPROACH_SECS {
                    self.study.skipped.push(id);
                    self.next_bag_item(world);
                    return;
                }
                let dx = x - self.body.x;
                if dx.abs() > 1.0 {
                    let speed = self.walk_speed() * wz::bag_pace(wz::bag_growth(self.study.bag_items));
                    self.facing = dx.signum();
                    match self.juggle_walk(world, dx.signum(), (speed * dt).min(dx.abs())) {
                        None => return,
                        // 조각 끝에 막힘: 이 물건은 건너뜀
                        Some(true) => {
                            self.study.skipped.push(id);
                            self.next_bag_item(world);
                            return;
                        }
                        Some(false) => {}
                    }
                    self.state = State::Study { act: StudyAct::Bag(BagPhase::Approach { id, x, obj_x: spot.x }), t };
                    return;
                }
                if (spot.x - self.body.x).abs() > 1.0 {
                    self.facing = (spot.x - self.body.x).signum();
                }
                self.state = State::Study { act: StudyAct::Bag(BagPhase::Stuff { id, gulped: false }), t: 0.0 };
            }
            BagPhase::Stuff { id, gulped } => {
                let gulped = gulped || {
                    let now = t >= wz::BAG_GULP_AT;
                    if now {
                        self.play.event = Some(ObjectPlay::Vanish { id });
                        self.study.bag_items += 1;
                    }
                    now
                };
                if t < wz::BAG_STUFF_SECS {
                    self.state = State::Study { act: StudyAct::Bag(BagPhase::Stuff { id, gulped }), t };
                } else {
                    // 방금 넣은 것은 다음 틱까지 목록에 남아 있을 수 있음
                    self.study.skipped.push(id);
                    self.next_bag_item(world);
                }
            }
            BagPhase::Done => {
                if t < wz::BAG_DONE_SECS {
                    self.state = State::Study { act: StudyAct::Bag(BagPhase::Done), t };
                } else {
                    self.end_bag();
                }
            }
        }
    }
}
