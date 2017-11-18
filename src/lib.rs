extern crate futures;
extern crate hyper;
extern crate hyper_tls;
extern crate serde;
#[macro_use]
extern crate serde_json;
#[macro_use]
extern crate serde_derive;
extern crate tokio_core;
extern crate uuid;
extern crate zip;

pub mod launcher;
pub mod parsing;
pub mod requests;
pub mod versions;
pub mod yggdrasil;

#[cfg(test)]
mod tests {
    use std::env;
    use std::process;

    #[test]
    fn start_minecraft() {
        use launcher;
        use yggdrasil::{self, Authenticator};
        let game_version_id = "1.10.2";
        let game_dir = env::home_dir().unwrap().join(".minecraft");
        let game_auth_info = yggdrasil::offline("zzzz").auth().unwrap();
        let launcher = launcher::create(game_dir, game_version_id, game_auth_info);
        let process: process::Child = launcher.to_launch_arguments().unwrap().start().unwrap();
        let exit_code = process.wait_with_output().unwrap().status.code().unwrap();
        println!("\nMinecraft client finished with exit code {}", exit_code);
    }
}
