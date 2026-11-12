//! 전송 이음매임: 친구 한 명에게 가는 순서 보장·신뢰성 있는 불투명 프레임 통로와 전송 중립 다이얼링.
//!
//! 이 위 계층은 전송과 무관함: `peers`가 프레임 위에서 Noise 핸드셰이크를 돌린 뒤
//! `secure::SecureLink`로 암호화된 JSON 메시지를 보냄. 새 전송(예: LAN 밖 친구용 릴레이)은
//! `FrameLink` + `FrameReader`를 구현하고 `Candidate` 종류와 `dial` 분기를 더한 뒤
//! 받은 링크를 `peers::run_session`에 넘기면 됨. Noise가 끝에서 끝까지 돌아 릴레이는 암호문만 봄.
//! 친구 신원은 고정된 Noise 정적 키라서 어느 전송에서든 같은 친구 목록이 통함.

use crate::{relay, tcp, Shared};
use std::net::SocketAddr;
use std::sync::Arc;

/// 보내는 쪽 반쪽임. 세션 스레드와 writer 스레드가 같이 씀
pub(crate) trait FrameLink: Send + Sync {
    /// 프레임 하나를 보냄 (최대 `MAX_FRAME`바이트). `false`면 링크가 끊긴 것임
    fn send_frame(&self, frame: &[u8]) -> bool;
    /// 우아한 닫기임: 우리 쪽은 더 안 보내지만 받기는 계속함
    fn finish(&self);
    /// 링크를 끊음. 짝인 `FrameReader`가 그다음 `Closed`를 알림
    fn close(&self);
    /// 상대가 누구인지 돌려줌 (속도 제한용, 예: 원격 IP)
    fn peer_addr(&self) -> String;
}

/// 한 번 받기 시도의 결과임
pub(crate) enum Recv {
    /// 프레임 하나가 옴
    Frame(Vec<u8>),
    /// 짧은 틱 안에 온 게 없음. 세션이 타이머를 볼 틈을 줌
    Idle,
    /// 프레임 모양이 잘못됨: 상대를 끊음
    Invalid,
    /// 링크가 사라짐
    Closed,
}

/// 받는 쪽 반쪽임. 세션 스레드가 가짐
pub(crate) trait FrameReader: Send {
    /// 최대 약 `TICK` 하나만큼만 기다림
    fn recv(&mut self) -> Recv;
}

/// 친구에게 닿을 수 있는 곳 하나임. 알림은 후보만 만들고,
/// Noise 핸드셰이크가 신원을 증명해야 링크로 침
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Candidate {
    /// LAN TCP 주소로
    Lan(SocketAddr),
    /// 릴레이를 거쳐 이 정적 공개키에게로
    Relay([u8; 32]),
}

/// `candidate`로 `peer_id`에게 워커 스레드에서 다이얼함.
/// 호출자가 이미 `peer_id`를 `State::dialing`에 넣어 두었어야 함
pub(crate) fn dial(shared: &Arc<Shared>, peer_id: String, candidate: Candidate) {
    match candidate {
        Candidate::Lan(addr) => tcp::dial(shared, peer_id, addr),
        Candidate::Relay(key) => relay::dial(shared, peer_id, key),
    }
}
