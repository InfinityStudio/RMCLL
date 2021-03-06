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
    #[test]
    fn start_minecraft() {
        use std::env;
        use launcher;
        use yggdrasil::{self, Authenticator};
        let game_dir = env::home_dir().unwrap().join(".minecraft/");
        let game_auth_info = yggdrasil::offline("zzzz").auth().unwrap();
        let launcher = launcher::create(game_dir, game_auth_info);
        let args = launcher.to_arguments("1.12.2").unwrap();
        println!("\nStarting minecraft with: {} {:?}", args.program(), args.args());
        let minecraft_process = args.start().unwrap();
        let output = minecraft_process.wait_with_output().unwrap();
        let exit_code = output.status.code().unwrap();
        println!("\nMinecraft client finished with exit code {}", exit_code);
    }
}
