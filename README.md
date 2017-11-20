# RMCLL - Rust MineCraft Launcher Library

**Still work in progress!**

An example for launching a minecraft 1.12.2 client in your home directory:

```rust
fn main() {
    use std::env;
    use rmcll::launcher;
    use rmcll::yggdrasil::{self, Authenticator};
    // prepare for starting minecraft client process
    let game_dir = env::home_dir().unwrap().join(".minecraft/");
    let game_auth_info = yggdrasil::offline("zzzz").auth().unwrap();
    let launcher = launcher::create(game_dir, game_auth_info);
    let args = launcher.to_arguments("1.12.2").unwrap();
    // start the 1.12.2 client now
    println!("\nStarting minecraft with: {} {:?}", args.program(), args.args());
    let minecraft_process = args.start().unwrap();
    let output = minecraft_process.wait_with_output().unwrap();
    let exit_code = output.status.code().unwrap();
    println!("\nMinecraft client finished with exit code {}", exit_code);
}
```

License: [Apache 2.0](LICENSE)
