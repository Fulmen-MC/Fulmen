//! Runs the ping against a tiny fake server on localhost, so no real server is needed.

use std::io::Write;
use std::net::TcpListener;
use std::thread;
use std::time::Duration;

use fulmen_protocol::handshake::{Handshake, NextState};
use fulmen_protocol::packet::{decode, encode, read_frame};
use fulmen_protocol::status::{PingRequest, PongResponse, StatusRequest, StatusResponse};

const JSON: &str = r#"{"version":{"name":"fake","protocol":1},"players":{"max":5,"online":0},"description":"test"}"#;

#[test]
fn ping_against_fake_server() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();

    let server = thread::spawn(move || {
        let (mut conn, _) = listener.accept().unwrap();
        let hs: Handshake = decode(&read_frame(&mut conn).unwrap()).unwrap();
        assert_eq!(hs.next_state, NextState::Status);
        assert_eq!(hs.server_address, "127.0.0.1");
        let _: StatusRequest = decode(&read_frame(&mut conn).unwrap()).unwrap();
        conn.write_all(&encode(&StatusResponse { json: JSON.into() }).unwrap())
            .unwrap();
        let ping: PingRequest = decode(&read_frame(&mut conn).unwrap()).unwrap();
        conn.write_all(
            &encode(&PongResponse {
                payload: ping.payload,
            })
            .unwrap(),
        )
        .unwrap();
    });

    let result = fulmen_net::status::ping("127.0.0.1", port, Duration::from_secs(2)).unwrap();
    assert_eq!(result.json, JSON);
    server.join().unwrap();
}

#[test]
fn ping_to_closed_port_fails() {
    // Bind and drop to obtain a port that is (very likely) closed.
    let port = TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    assert!(fulmen_net::status::ping("127.0.0.1", port, Duration::from_millis(500)).is_err());
}
