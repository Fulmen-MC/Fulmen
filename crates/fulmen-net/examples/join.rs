//! Usage: `cargo run -p fulmen-net --example join -- [host[:port]] [name]`
//!
//! Joins an offline-mode server, prints what arrives for ten seconds and leaves. There is
//! no keep-alive handling yet, so a real server will kick us after a while.

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use fulmen_net::{ConnectOptions, Connection};

fn main() {
    let mut args = std::env::args().skip(1);
    let target = args.next().unwrap_or_else(|| "127.0.0.1:25565".into());
    let username = args.next().unwrap_or_else(|| "Fulmen".into());
    let (host, port) = match target.rsplit_once(':') {
        Some((h, p)) => match p.parse::<u16>() {
            Ok(p) => (h.to_owned(), p),
            Err(_) => (target.clone(), 25565),
        },
        None => (target.clone(), 25565),
    };

    let opts = ConnectOptions {
        host,
        port,
        username,
        timeout: Duration::from_secs(5),
    };
    let conn = match Connection::connect(&opts) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("join failed: {e}");
            std::process::exit(1);
        }
    };
    println!("logged in as {} ({:032x})", conn.username, conn.uuid);

    let deadline = Instant::now() + Duration::from_secs(10);
    let mut seen: BTreeMap<i32, (usize, usize)> = BTreeMap::new();
    while Instant::now() < deadline {
        match conn.recv_timeout(Duration::from_millis(200)) {
            Ok(Some(p)) => {
                let entry = seen.entry(p.id).or_default();
                entry.0 += 1;
                entry.1 += p.body.len();
            }
            Ok(None) => {}
            Err(e) => {
                println!("connection ended: {e}");
                break;
            }
        }
    }
    println!("play packets by id (count, bytes):");
    for (id, (count, bytes)) in &seen {
        println!("  {id:#04x}: {count} x, {bytes} bytes");
    }
}
