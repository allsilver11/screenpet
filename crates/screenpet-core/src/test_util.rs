//! 여러 테스트 파일이 같이 쓰는 도우미 (테스트 빌드에만 들어간다).

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
