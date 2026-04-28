//! Boots the proxy with auth on an ephemeral port and prints
//! `<base_url> <api_key>` so a verifier script can curl it.
//! Exits cleanly when stdin closes (script kills the child).

use std::time::Duration;

use ccuse_desktop_lib::proxy::ProxyRuntime;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let runtime = ProxyRuntime::new(0, 1);
    let config = runtime.start().await?;
    println!("{} {}", config.base_url, config.api_key);
    // Keep the runtime alive until the parent process closes stdin
    // (or kills us). 60s ceiling is a belt-and-braces exit.
    tokio::time::sleep(Duration::from_secs(60)).await;
    runtime.stop().await?;
    Ok(())
}
