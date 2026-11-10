//! A new developer key pair for the feedback mailbox (💌 건의하기).
//! The public key goes into `letter::DEV_KEY`; the secret goes only into the
//! developer's own `config.json` (`dev_key`), never into the repository.
//!
//!     cargo run -p screenpet-net --example dev_key

fn main() {
    let (secret, public) = screenpet_net::letter::new_dev_keypair();
    println!("public {public}");
    println!("secret {secret}");
}
