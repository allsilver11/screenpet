//! `SecureLink`: 핸드셰이크 뒤의 메시지 통로임.
//! 메시지는 큐에 넣기만 하고(호출자를 막지 않음) writer 스레드가 Noise 전송 상태로 암호화해 프레임으로 보냄.
//! `peers` 세션이 만들고 씀. 아래로는 `transport::FrameLink`와 이야기함.

use crate::protocol::{Message, MAX_MESSAGE};
use crate::transport::FrameLink;
use crate::Shared;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{sync_channel, Receiver, SyncSender, TrySendError};
use std::sync::{Arc, Mutex};

/// 링크당 쌓아 두는 메시지 수. 넘치면 막힌 것으로 보고 닫음
const QUEUE: usize = 64;

/// writer 스레드에 넘기는 일감임
enum Out {
    /// 평문 메시지 하나
    Msg(Vec<u8>),
    /// 더 보내지 않음. 쌓인 것을 다 쓴 뒤 반쯤 닫음
    Finish,
}

/// 친구 한 명과의 암호화된 메시지 통로임 (writer 스레드 하나를 둠)
pub(crate) struct SecureLink {
    tx: SyncSender<Out>,
    frames: Arc<dyn FrameLink>,
    closing: AtomicBool,
}

impl SecureLink {
    /// writer 스레드를 띄움. 띄우지 못하면 None임
    pub fn start(
        shared: &Shared,
        frames: Arc<dyn FrameLink>,
        transport: Arc<Mutex<snow::TransportState>>,
    ) -> Option<Arc<SecureLink>> {
        let (tx, rx) = sync_channel(QUEUE);
        let f = frames.clone();
        if !shared.spawn(move || writer(rx, f, transport)) {
            return None;
        }
        Some(Arc::new(SecureLink { tx, frames, closing: AtomicBool::new(false) }))
    }

    /// 막지 않고 메시지를 큐에 넣음. 닫는 중·끊김·큐가 꽉 참(이때 링크를 닫음)이면 `false`임
    pub fn send(&self, msg: &Message) -> bool {
        if self.closing() {
            return false;
        }
        let json = msg.to_json();
        if json.len() > MAX_MESSAGE {
            return false;
        }
        match self.tx.try_send(Out::Msg(json)) {
            Ok(()) => true,
            Err(TrySendError::Full(_)) => {
                self.close();
                false
            }
            Err(TrySendError::Disconnected(_)) => false,
        }
    }

    /// 우아한 닫기임: 이미 쌓인 것은 다 보내고 우리 쪽을 반쯤 닫음.
    /// 세션은 상대가 닫을 때까지 계속 읽음
    pub fn finish(&self) {
        if !self.closing.swap(true, Ordering::SeqCst) && self.tx.try_send(Out::Finish).is_err() {
            self.frames.close();
        }
    }

    /// 즉시 끊음
    pub fn close(&self) {
        self.closing.store(true, Ordering::SeqCst);
        self.frames.close();
    }

    /// 닫는 중이거나 닫혔는지 돌려줌
    pub fn closing(&self) -> bool {
        self.closing.load(Ordering::SeqCst)
    }
}

/// writer 스레드 본체임: 큐에서 꺼내 암호화해 프레임으로 보내고, 실패하면 링크를 닫음
fn writer(rx: Receiver<Out>, frames: Arc<dyn FrameLink>, transport: Arc<Mutex<snow::TransportState>>) {
    let mut buf = vec![0u8; MAX_MESSAGE + 16];
    for out in rx {
        match out {
            Out::Msg(plain) => {
                let n = {
                    let mut t = transport.lock().unwrap_or_else(|e| e.into_inner());
                    t.write_message(&plain, &mut buf)
                };
                match n {
                    Ok(n) if frames.send_frame(&buf[..n]) => {}
                    _ => {
                        frames.close();
                        return;
                    }
                }
            }
            Out::Finish => {
                frames.finish();
                return;
            }
        }
    }
}
