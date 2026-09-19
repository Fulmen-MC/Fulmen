//! Usage: `cargo run -p fulmen-net --example ping -- [host[:port]]`
//!
//! Defaults to `localhost:25565`. IPv6 literals are not supported by this example.

use std::time::Duration;

fn main() {
    let target = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "localhost:25565".into());
    let (host, port) = match target.rsplit_once(':') {
        Some((h, p)) => match p.parse::<u16>() {
            Ok(p) => (h.to_owned(), p),
            Err(_) => (target.clone(), 25565),
        },
        None => (target.clone(), 25565),
    };

    match fulmen_net::status::ping(&host, port, Duration::from_secs(5)) {
        Ok(result) => {
            println!("{:#?}", result.status);
            println!("latency: {} ms", result.latency.as_millis());
        }
        Err(e) => {
            eprintln!("ping to {host}:{port} failed: {e}");
            std::process::exit(1);
        }
    }
}
