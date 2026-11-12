//! 📌 작업표시줄(Dock)에서 앱 아이콘 끌어내 놀기 (pet.json `taskbar_play` — 흐물이): 숫자와 앱과 주고받는 것. 동물의 상태는 brain_taskbar.rs,
//! 아이콘 그림은 platform (Windows 창 아이콘 · macOS 앱 아이콘), 창은 src-tauri objects.rs.
//!
//! 흉내일 뿐이다: 진짜 작업표시줄·Dock 아이콘은 옮기지도 숨기지도 덮지도 않는다. 장난감은 지금 열린 앱 하나의 아이콘을 베낀 그림이다.
//! - 끌어내기: 화면 아래 바닥을 걷거나 쉬다가 PULL_EVERY_SECS마다 한 번 PULL_CHANCE로 — 쪼그려 앉아 바닥(작업표시줄 쪽)을 잡고 끙차 당기다가
//!   앱이 장난감을 만들어 주면 번쩍 들어 올린다 (평소의 들고 던지기: 던지기·들고 돌아다니기. 친구에게 배달은 하지 않는다).
//!   앱이 YANK_GIVE_UP_SECS 안에 못 만들면 (아이콘을 못 읽는 OS 등) 그냥 일어난다.
//! - 도로 넣기: 내려앉은 지 RETURN_SECS (가끔 QUICK_RETURN_SECS) 뒤에 가서 주워 바닥 쪽으로 조금 걷다가 쪼그려 밀어 넣으면 뿅.
//!   가끔(PUT_BACK_CHANCE)은 꺼내자마자 들여다보고 바로 도로 넣는다.
//! - 흐물이가 없거나 자거나 친구에게 가 있으면 장난감은 TOY_LIFE_SECS 뒤 저절로 스르륵 사라진다 — 쌓이지 않는다. 한 번에 하나만.
//! - 🔒 장난감은 이 컴퓨터를 떠나지 않는다: 친구에게 던져지지도 들려 가지도 않고 (연결된 화면 끝에선 안 받아 준 물건처럼 튕겨 돌아온다),
//!   설정에 저장되지 않고, 기록(로그)에 남지 않는다. 앱 이름·창 제목은 읽지도 않는다 — 아이콘 그림 칸만.
//!
//! 주사위는 이 동물만 자기 주사위(ToyMood.rng)로 굴린다 — 다른 동물과 흐물이의 들고 던지기 주사위 순서는 그대로.

/// 끌어내기 주사위를 굴리는 간격 (초) — 굴릴 때가 됐어도 바닥에서 걷거나 쉬고 있을 때만 굴린다
pub const PULL_EVERY_SECS: (f64, f64) = (150.0, 330.0);
/// 굴릴 때마다 끌어낼 확률 (평균 8분에 한 번쯤, 짧아도 2분 반 간격)
pub const PULL_CHANCE: f64 = 0.5;
/// 끙차 당기는 최소 시간 — 장난감이 더 빨리 와도 이만큼은 당긴다 (그림 yank 4칸 6fps 한 바퀴 남짓)
pub const YANK_MIN_SECS: f64 = 1.0;
/// 이만큼 당겨도 장난감이 안 오면 그만둔다 (앱이 아이콘을 못 읽었다)
pub const YANK_GIVE_UP_SECS: f64 = 3.0;
/// 꺼내자마자 들여다보고 바로 도로 넣을 확률
pub const PUT_BACK_CHANCE: f64 = 0.15;
/// 꺼낸 뒤 던지기 전에 들고 돌아다닐 확률 (아니면 바로 던진다)
pub const TOY_WANDER_CHANCE: f64 = 0.45;
/// 장난감이 손을 떠난 뒤 도로 넣으러 가기까지 (초)
pub const RETURN_SECS: (f64, f64) = (20.0, 60.0);
/// 가끔은 금방 도로 넣으러 간다
pub const QUICK_RETURN_CHANCE: f64 = 0.2;
pub const QUICK_RETURN_SECS: (f64, f64) = (2.0, 5.0);
/// 도로 넣으러 갈 때가 됐는데 장난감에 갈 수 없으면 (다른 디딤판·날아가는 중) 이만큼 뒤에 다시 본다
pub const RETRY_SECS: f64 = 5.0;
/// 도로 넣기 전에 들고 바닥을 조금 걷는다 (초)
pub const STUFF_WALK_SECS: (f64, f64) = (0.8, 2.0);
/// 도로 밀어 넣기: 내려 쪼그려 앉아 (STUFF_RELEASE초에) 밀어 넣으면 뿅, STUFF_SECS에 일어난다 (그림 stuff 4칸 6fps 한 번 — 넷째 칸이 0.5초)
pub const STUFF_SECS: f64 = 1.0;
pub const STUFF_RELEASE: f64 = 0.5;
/// 장난감은 만든 지 이만큼 뒤 저절로 스르륵 사라진다 (초 — 누가 들고 있으면 기다린다)
pub const TOY_LIFE_SECS: f64 = 240.0;
/// 장난감 아이콘 그림 한 변 (px — 앱 아이콘을 이 크기로 읽는다). 장난감은 화면에 51 논리 px쯤 (object::AppIcon 칸 68/128 × 96 × 0.75)이라
/// 2배 화면(Retina)에서도 또렷하게 그 두 배쯤. 웹뷰에 base64로 ~49 KB
pub const TOY_ICON_PX: u32 = 96;
/// 장난감 아이콘을 못 읽었으면 이만큼은 다시 묻지 않는다 (초 — 앱)
pub const CAPTURE_BACKOFF_SECS: f64 = 600.0;

/// 앱이 매 틱 알려 주는 장난감 사정
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Toys {
    /// 지금 장난감을 만들 수 있다 (아이콘을 읽는 OS, 화면에 장난감이 없고 만드는 중도 아니다, 모두 숨기지 않았다)
    pub ready: bool,
    /// 화면에 있는 장난감 (물건 id) — 한 번에 하나
    pub live: Option<u64>,
}
