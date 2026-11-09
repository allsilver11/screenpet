//! 여러 테스트 파일이 같이 쓰는 도우미 (테스트 빌드에만 들어간다).

use crate::behavior::Personality;
use crate::brain::{Pet, PetConfig, Speech, State};
use crate::care::{Care, Stage};
use crate::presence::{DailyMarks, LocalMoment, Rhythm, RhythmInput};
use crate::world::World;

/// 테스트 시계의 한 틱 (60fps)
pub(crate) const DT: f64 = 1.0 / 60.0;

/// 평범한 동물: 128px, 걷기 60px/s, 특기 없음
pub(crate) fn pet_cfg() -> PetConfig {
    PetConfig {
        hang_grip: (0.0, 0.06),
        no_roll: false,
        size: 128.0,
        half_w: 38.0,
        walk_speed: 60.0,
        special: None,
        personality: Personality::Friendly,
        can_climb: false,
        flies: false,
        swift: false,
        hops: false,
        hop_share: 0.0,
        looks_up: false,
        snow_cloud: false,
        dpi: 1.0,
    }
}

/// 나는 동물 (테스트 설정)
pub(crate) fn flyer() -> PetConfig {
    PetConfig { flies: true, ..pet_cfg() }
}

/// 한참 쉬는 중
pub(crate) fn idle() -> State {
    State::Idle { remaining: 100.0, look: false }
}

/// done이 참이 될 때까지 (최대 ticks틱) 돌린다
pub(crate) fn run_until(p: &mut Pet, w: &World, ticks: usize, done: impl Fn(&Pet) -> bool) -> bool {
    for _ in 0..ticks {
        p.update(DT, w);
        if done(p) {
            return true;
        }
    }
    false
}

/// ui/pets에 있는 동물 전부 (이름순). 새 동물을 더하면 테스트가 저절로 그 동물도 본다 — pet.json이 없는 폴더는 없어야 한다
pub(crate) fn species() -> Vec<String> {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../ui/pets");
    let mut out: Vec<String> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("{dir:?}: {e}"))
        .map(|e| e.unwrap().path())
        .filter(|path| path.is_dir())
        .map(|path| {
            assert!(path.join("pet.json").is_file(), "{path:?} has no pet.json");
            path.file_name().unwrap().to_string_lossy().into_owned()
        })
        .collect();
    assert!(!out.is_empty(), "no pets in {dir:?}");
    out.sort();
    out
}

/// 번들 pet.json 그대로의 설정 (게임이 쓰는 PetConfig::from_manifest, 청소년, 화면 배율 1)
pub(crate) fn bundled() -> Vec<(String, PetConfig)> {
    species()
        .into_iter()
        .map(|id| {
            let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../ui/pets").join(&id).join("pet.json");
            let json = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path:?}: {e}"));
            let m = crate::manifest::parse_manifest(&json).unwrap_or_else(|e| panic!("{id}: {e:?}"));
            let c = PetConfig::from_manifest(&m, 1.0, m.size_mult(Stage::Teenager), Stage::Teenager.name());
            (id, c)
        })
        .collect()
}

/// `while`·`loop` 테스트의 안전장치: 회귀가 나면 테스트가 멈춰 버리지 않고 실패하게 한다.
/// 넉넉하게 시뮬레이션 10분(60fps)까지만 돈다.
pub(crate) struct LoopLimit(usize);

impl LoopLimit {
    pub(crate) fn new() -> Self {
        LoopLimit(60 * 600)
    }

    #[track_caller]
    pub(crate) fn step(&mut self) {
        assert!(self.0 > 0, "loop did not finish within the step limit");
        self.0 -= 1;
    }
}

/// 가짜 세상의 하루: 시각·유휴 시간을 테스트가 정한다. 1초에 한 번 Rhythm::tick, 그 사이 60 틱
pub(crate) struct Sim {
    pub(crate) rhythm: Rhythm,
    pub(crate) care: Care,
    pub(crate) marks: DailyMarks,
    pub(crate) secs: f64,
    pub(crate) day: i32,
    /// 지금 시각 (시, 분) — clock_runs면 start 분에서 secs만큼 흐른다
    pub(crate) time: (u32, u32),
    pub(crate) clock_runs: bool,
    pub(crate) idle: Option<f64>,
    pub(crate) visible: bool,
    pub(crate) said: Vec<Speech>,
    pub(crate) house_waves: u32,
    /// 틱마다 커서를 동물 근처로 움직인다
    pub(crate) hover_near: bool,
}

impl Sim {
    /// 멈춘 시계, 오늘 아침 인사는 이미 했다 (따로 시험하는 테스트만 지운다)
    pub(crate) fn fixed_at(time: (u32, u32)) -> Self {
        Self {
            rhythm: Rhythm::default(),
            care: Care::default(),
            marks: DailyMarks { morning: Some(1), lunch: Some(1) },
            secs: 0.0,
            day: 1,
            time,
            clock_runs: false,
            idle: Some(0.0),
            visible: true,
            said: Vec::new(),
            house_waves: 0,
            hover_near: false,
        }
    }

    /// hour:minute부터 흐르는 시계, 하루의 인사도 아직, 커서는 동물 곁을 맴돈다
    pub(crate) fn from(hour: u32, minute: u32) -> Self {
        Self { marks: DailyMarks::default(), time: (hour, minute), clock_runs: true, hover_near: true, ..Self::fixed_at((hour, minute)) }
    }

    pub(crate) fn input(&self) -> RhythmInput {
        let (h, m) = if self.clock_runs {
            let minute = self.time.0 * 60 + self.time.1 + (self.secs / 60.0) as u32;
            (minute / 60, minute % 60)
        } else {
            self.time
        };
        RhythmInput { secs: self.secs, moment: LocalMoment::new(self.day, h, m), idle: self.idle, visible: self.visible }
    }

    pub(crate) fn tick_rhythm(&mut self, p: &mut Pet, w: &World) {
        let input = self.input();
        let out = self.rhythm.tick(&input, p, w, &mut self.care, &mut self.marks);
        self.house_waves += out.house_wave as u32;
    }

    /// 1초. each는 틱마다 (세상을 흔들거나 살펴본다)
    pub(crate) fn second_with(&mut self, p: &mut Pet, w: &World, mut each: impl FnMut(&Pet)) {
        self.tick_rhythm(p, w);
        for _ in 0..60 {
            if self.hover_near {
                p.hover(p.body.x + 90.0, p.body.y - 200.0, false);
            }
            p.update(DT, w);
            if let Some(s) = p.take_speech() {
                self.said.push(s);
            }
            each(p);
        }
        self.secs += 1.0;
        if let Some(i) = self.idle.as_mut() {
            *i += 1.0;
        }
    }

    pub(crate) fn second(&mut self, p: &mut Pet, w: &World) {
        self.second_with(p, w, |_| {});
    }

    pub(crate) fn run(&mut self, p: &mut Pet, w: &World, secs: u32) {
        for _ in 0..secs {
            self.second(p, w);
        }
    }
}
