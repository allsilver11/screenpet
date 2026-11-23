//! ✨ 덮개 창 (trail.html의 ui/magic.js)이 그리는 마법 효과 — 펫이 매 틱 `Pet::magic_fx`로 내놓고 앱 (src-tauri trail.rs)이
//! 그 자리 모니터의 덮개 창 좌표 (논리 px)로 바꿔 보냄. 좌표는 모두 물리 px.
//!
//! - ⚡ Bolt: 볼드모트 Stupefy의 초록 빛줄기 (지팡이 끝 → 맞은 자리)
//! - ⚔️ Duel: 해리 (빨강)·볼드모트 (초록) 빛줄기가 가운데 clash에서 맞부딪힘 (reach = 뻗어 나간 몫, 1이면 맞부딪혀 반짝)
//! - 🐍 Snake: 볼드모트의 나기니 (발밑 y를 따라 기어 다님)
//! - 🦌 Stag: 해리의 은빛 사슴 (dir 쪽으로 달림)
//! - 🦦 Otter: 헤르미온느의 은빛 수달 (dir 쪽으로 물결치듯 헤엄침 — tilt = 몸 기울기, t = 나온 뒤 초)
//! - 👻 Soul: 디멘터가 빼앗는 펫의 영혼 줄기 (펫 from → 디멘터 to, level = 짙기)
//! - 🥶 Frost: 디멘터 가까운 화면 구석의 서리 (in = 구석에서 화면 안쪽 방향 부호)
//! - 🔥 Campfire: 해그리드가 피운 모닥불 (바닥 위 장작 + 일렁이는 불꽃 + 튀는 불티, level = 타오른 몫)
//! - 🪄 Wand: ⚔️ Expelliarmus에 맞아 날아간 지팡이 (손 → 땅, k = 날아간 몫 — 일어날 때 손으로 돌아옴)
//! - 💡 Glow: "Lumos!" 지팡이 끝의 밝은 불빛 (level = 밝기 0…1 — 덮개 창이 빛 번짐을 그림)
//! - 🌑 Puff: "Nox!" 불이 꺼진 지팡이 끝에서 피어오르는 작은 검은 연기 (k = 퍼진 몫 0…1)
//! - 🥚 Egg: 해그리드가 품은 용알 (crack = 금 간 몫) · 🐉 Hatchling: 알에서 깨어난 아기 용 (알 자리 둘레를 파닥파닥 날다 불 퐁 뿜고 사라짐)

use serde::Serialize;

/// 빛줄기 색
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BeamColor {
    /// 해리·Expelliarmus
    Red,
    /// 볼드모트·Stupefy
    Green,
    /// 헤르미온느 (결투의 밝은 쪽)
    Gold,
}

/// 덮개 창에 그릴 마법 효과 하나 (물리 px — 앱이 덮개 창 논리 px로 바꿈, mapped)
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MagicFx {
    /// ⚡ 빛줄기 하나 (from → to, width = 굵기)
    Bolt { from: (f64, f64), to: (f64, f64), color: BeamColor, width: f64 },
    /// ⚔️ 결투: red (밝은 쪽 — 해리 빨강·헤르미온느 금빛 = light)·green (볼드모트) 지팡이 끝에서 clash까지 두 빛줄기 (reach = 뻗은 몫 0…1, width = 굵기)
    Duel { red: (f64, f64), green: (f64, f64), clash: (f64, f64), reach: f64, width: f64, light: BeamColor },
    /// 🐍 나기니: 머리 자리 x, 바닥 y, 길이 size, dir = 머리 쪽 (±1), t = 나온 뒤 초 (꿈틀), alpha = 보이는 몫
    Snake { x: f64, y: f64, size: f64, dir: f64, t: f64, alpha: f64 },
    /// 🦌 은빛 사슴: 가운데 (x, y), 크기 size, dir = 달리는 쪽 (±1), alpha = 보이는 몫
    Stag { x: f64, y: f64, size: f64, dir: f64, alpha: f64 },
    /// 🦦 은빛 수달: 가운데 (x, y), 크기 size, dir = 헤엄치는 쪽 (±1), alpha = 보이는 몫, tilt = 몸 기울기 (라디안, dir 쪽 기준 — 머리를 들면 −), t = 나온 뒤 초 (발·꼬리 젓기)
    Otter { x: f64, y: f64, size: f64, dir: f64, alpha: f64, tilt: f64, t: f64 },
    /// 👻 영혼 줄기: 펫 가슴 (from)에서 디멘터 (to)로 빨려 올라가는 은빛 줄기, 굵기 width, level = 짙기 0…1, t = 시작한 뒤 초 (일렁임)
    Soul { from: (f64, f64), to: (f64, f64), width: f64, level: f64, t: f64 },
    /// 🥶 서리: 구석 (x, y), 크기 size, inward = 화면 안쪽 부호 (±1, ±1), level = 낀 몫 0…1
    Frost { x: f64, y: f64, size: f64, inward: (f64, f64), level: f64 },
    /// 🥚 용알: 바닥 가운데 (x, y — 해그리드 손), 높이 size, crack = 금 간 몫 0…1
    Egg { x: f64, y: f64, size: f64, crack: f64 },
    /// 🐉 아기 용: 깨어난 알 자리 (x, y), 크기 size, t = 깨어난 뒤 초 (len초 — 덮개 창이 둘레를 날다 불 퐁 뿜고 사라지게 그림), dir = 처음 날아가는 쪽
    Hatchling { x: f64, y: f64, size: f64, t: f64, len: f64, dir: f64 },
    /// 🪄 날아간 지팡이: 손 (from)에서 땅 (to)까지 k (0 = 손 … 1 = 땅) 만큼 빙글 날아간 자리, 길이 size
    Wand { from: (f64, f64), to: (f64, f64), k: f64, size: f64 },
    /// 💡 루모스 불빛: 지팡이 끝 (x, y), 빛 번짐 반지름 size, level = 밝기 0…1
    Glow { x: f64, y: f64, size: f64, level: f64 },
    /// 🌑 녹스 연기: 지팡이 끝 (x, y), 크기 size, k = 퍼진 몫 0…1 (덮개 창이 피어오르며 흐려지는 검은 연기 몇 덩이를 그림)
    Puff { x: f64, y: f64, size: f64, k: f64 },
    /// 🔥 모닥불: 바닥 가운데 (x, y), 장작 폭 size, level = 타오른 몫 0…1 (피울 때 커지고 끌 때 줄어듦 — 덮개 창이 일렁이는 불꽃·튀는 불티를 그림)
    Campfire { x: f64, y: f64, size: f64, level: f64 },
}

impl MagicFx {
    /// 어느 모니터 덮개 창에 그릴지 정하는 점 (구석의 서리는 화면 안쪽으로 1px)
    pub fn anchor(&self) -> (f64, f64) {
        match *self {
            MagicFx::Bolt { from, .. } | MagicFx::Wand { from, .. } | MagicFx::Soul { from, .. } => from,
            MagicFx::Glow { x, y, .. } | MagicFx::Puff { x, y, .. } => (x, y),
            MagicFx::Duel { clash, .. } => clash,
            MagicFx::Snake { x, y, .. } | MagicFx::Stag { x, y, .. } | MagicFx::Otter { x, y, .. } | MagicFx::Egg { x, y, .. } | MagicFx::Hatchling { x, y, .. } | MagicFx::Campfire { x, y, .. } => (x, y - 1.0),
            MagicFx::Frost { x, y, inward, .. } => (x + inward.0, y + inward.1),
        }
    }

    /// 점은 at으로, 길이는 scale을 곱해 바꾼 것 (덮개 창 논리 px로 — 앱 trail.rs)
    pub fn mapped(&self, at: impl Fn((f64, f64)) -> (f64, f64), scale: f64) -> MagicFx {
        match *self {
            MagicFx::Bolt { from, to, color, width } => MagicFx::Bolt { from: at(from), to: at(to), color, width: width * scale },
            MagicFx::Duel { red, green, clash, reach, width, light } => MagicFx::Duel { red: at(red), green: at(green), clash: at(clash), reach, width: width * scale, light },
            MagicFx::Snake { x, y, size, dir, t, alpha } => {
                let (x, y) = at((x, y));
                MagicFx::Snake { x, y, size: size * scale, dir, t, alpha }
            }
            MagicFx::Stag { x, y, size, dir, alpha } => {
                let (x, y) = at((x, y));
                MagicFx::Stag { x, y, size: size * scale, dir, alpha }
            }
            MagicFx::Otter { x, y, size, dir, alpha, tilt, t } => {
                let (x, y) = at((x, y));
                MagicFx::Otter { x, y, size: size * scale, dir, alpha, tilt, t }
            }
            MagicFx::Soul { from, to, width, level, t } => MagicFx::Soul { from: at(from), to: at(to), width: width * scale, level, t },
            MagicFx::Frost { x, y, size, inward, level } => {
                let (x, y) = at((x, y));
                MagicFx::Frost { x, y, size: size * scale, inward, level }
            }
            MagicFx::Egg { x, y, size, crack } => {
                let (x, y) = at((x, y));
                MagicFx::Egg { x, y, size: size * scale, crack }
            }
            MagicFx::Hatchling { x, y, size, t, len, dir } => {
                let (x, y) = at((x, y));
                MagicFx::Hatchling { x, y, size: size * scale, t, len, dir }
            }
            MagicFx::Campfire { x, y, size, level } => {
                let (x, y) = at((x, y));
                MagicFx::Campfire { x, y, size: size * scale, level }
            }
            MagicFx::Wand { from, to, k, size } => MagicFx::Wand { from: at(from), to: at(to), k, size: size * scale },
            MagicFx::Glow { x, y, size, level } => {
                let (x, y) = at((x, y));
                MagicFx::Glow { x, y, size: size * scale, level }
            }
            MagicFx::Puff { x, y, size, k } => {
                let (x, y) = at((x, y));
                MagicFx::Puff { x, y, size: size * scale, k }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effects_serialize_with_a_kind_tag_for_the_webview() {
        let fx = MagicFx::Bolt { from: (1.0, 2.0), to: (3.0, 4.0), color: BeamColor::Green, width: 5.0 };
        let v: serde_json::Value = serde_json::to_value(fx).unwrap();
        assert_eq!(v["kind"], "bolt");
        assert_eq!(v["color"], "green");
        assert_eq!(v["from"], serde_json::json!([1.0, 2.0]));
        let s = serde_json::to_value(MagicFx::Frost { x: 0.0, y: 0.0, size: 1.0, inward: (1.0, 1.0), level: 0.5 }).unwrap();
        assert_eq!(s["kind"], "frost");
        let h = serde_json::to_value(MagicFx::Hatchling { x: 0.0, y: 0.0, size: 1.0, t: 0.5, len: 5.0, dir: -1.0 }).unwrap();
        assert_eq!((h["kind"].as_str(), h["len"].as_f64()), (Some("hatchling"), Some(5.0)));
        let w = serde_json::to_value(MagicFx::Wand { from: (0.0, 0.0), to: (5.0, 5.0), k: 0.5, size: 3.0 }).unwrap();
        assert_eq!((w["kind"].as_str(), w["k"].as_f64()), (Some("wand"), Some(0.5)));
        let g = MagicFx::Glow { x: 10.0, y: 20.0, size: 8.0, level: 1.0 };
        assert_eq!(serde_json::to_value(g).unwrap()["kind"], "glow");
        assert_eq!(g.mapped(|(x, y)| (x / 2.0, y / 2.0), 0.5), MagicFx::Glow { x: 5.0, y: 10.0, size: 4.0, level: 1.0 });
        let n = MagicFx::Puff { x: 10.0, y: 20.0, size: 8.0, k: 0.5 };
        assert_eq!(serde_json::to_value(n).unwrap()["kind"], "puff");
        assert_eq!(n.anchor(), (10.0, 20.0));
        assert_eq!(n.mapped(|(x, y)| (x / 2.0, y / 2.0), 0.5), MagicFx::Puff { x: 5.0, y: 10.0, size: 4.0, k: 0.5 });
        let o = MagicFx::Otter { x: 10.0, y: 20.0, size: 8.0, dir: 1.0, alpha: 1.0, tilt: 0.2, t: 0.5 };
        assert_eq!(serde_json::to_value(o).unwrap()["kind"], "otter");
        assert_eq!(o.mapped(|(x, y)| (x / 2.0, y / 2.0), 0.5), MagicFx::Otter { x: 5.0, y: 10.0, size: 4.0, dir: 1.0, alpha: 1.0, tilt: 0.2, t: 0.5 });
        let soul = MagicFx::Soul { from: (10.0, 40.0), to: (10.0, 0.0), width: 8.0, level: 0.5, t: 1.0 };
        assert_eq!(serde_json::to_value(soul).unwrap()["kind"], "soul");
        assert_eq!(soul.anchor(), (10.0, 40.0));
        assert_eq!(soul.mapped(|(x, y)| (x / 2.0, y / 2.0), 0.5), MagicFx::Soul { from: (5.0, 20.0), to: (5.0, 0.0), width: 4.0, level: 0.5, t: 1.0 });
        let egg = MagicFx::Egg { x: 10.0, y: 20.0, size: 8.0, crack: 0.5 };
        assert_eq!(egg.mapped(|(x, y)| (x / 2.0, y / 2.0), 0.5), MagicFx::Egg { x: 5.0, y: 10.0, size: 4.0, crack: 0.5 });
    }

    #[test]
    fn mapping_moves_points_and_scales_sizes() {
        let at = |(x, y): (f64, f64)| ((x - 100.0) / 2.0, (y - 50.0) / 2.0);
        let fx = MagicFx::Duel { red: (100.0, 50.0), green: (300.0, 50.0), clash: (200.0, 50.0), reach: 1.0, width: 8.0, light: BeamColor::Gold };
        assert_eq!(fx.mapped(at, 0.5), MagicFx::Duel { red: (0.0, 0.0), green: (100.0, 0.0), clash: (50.0, 0.0), reach: 1.0, width: 4.0, light: BeamColor::Gold });
        let frost = MagicFx::Frost { x: 1920.0, y: 0.0, size: 200.0, inward: (-1.0, 1.0), level: 1.0 };
        assert_eq!(frost.anchor(), (1919.0, 1.0), "구석은 화면 안쪽 1px로 모니터를 고름");
    }
}
