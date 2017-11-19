# RMCLL - Rust MineCraft Launcher Library

**Still work in progress!**

An example for launching a minecraft 1.8.9 client in your home directory:

```rust
fn main() {
    use std::env;
    use rmcll::launcher;
    use rmcll::yggdrasil::{self, Authenticator};
    // prepare for starting minecraft client process
    let game_dir = env::home_dir().unwrap().join(".minecraft/");
    let game_auth_info = yggdrasil::offline("zzzz").auth().unwrap();
    let launcher = launcher::create(game_dir, game_auth_info);
    // start the 1.8.9 client now
    let process = launcher.to_arguments("1.8.9").unwrap().start().unwrap();
    let exit_code = process.wait_with_output().unwrap().status.code().unwrap();
    println!("\nMinecraft client finished with exit code {}", exit_code);
}
```

License: [Apache 2.0](LICENSE)
