//! 건의 편지함(💌 건의하기)용 새 개발자 키 쌍을 만듦.
//! 공개키는 `letter::DEV_KEY`에, 비밀키는 개발자 자신의 `config.json`(`dev_key`)에만 넣음. 저장소에는 절대 안 넣음.
//!
//!     cargo run -p screenpet-net --example dev_key

/// 키 쌍을 만들어 공개키와 비밀키를 찍음
fn main() {
    let (secret, public) = screenpet_net::letter::new_dev_keypair();
    println!("public {public}");
    println!("secret {secret}");
}
