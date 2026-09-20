//! A connection to a server: a blocking login phase, then reader and writer threads.
//!
//! Handshake, login and configuration run synchronously on the calling thread, because
//! they are a short, strictly ordered conversation (and the only place where compression
//! and later encryption get switched on). Once the server has finished the configuration,
//! the socket is split: one thread reads, decompresses and frames incoming packets, another
//! compresses and writes outgoing ones. The game talks to both through channels and is
//! never blocked by network I/O or (de)compression.

use std::io::BufReader;
use std::net::{Shutdown, TcpStream};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender, TryRecvError};
use std::thread;
use std::time::Duration;

use fulmen_protocol::PROTOCOL_VERSION;
use fulmen_protocol::codec::{write_bool, write_string, write_u8};
use fulmen_protocol::handshake::{Handshake, NextState};
use fulmen_protocol::login::{
    LoginAcknowledged, LoginDisconnect, LoginStart, LoginSuccess, SetCompression,
};
use fulmen_protocol::packet::Packet;
use fulmen_protocol::varint::{read_varint, write_varint};

use crate::framing::{RawPacket, Threshold, read_packet, threshold_from_packet, write_packet};
use crate::offline::offline_uuid;
use crate::status::connect;
use crate::{Error, Result};

// Packet ids, protocol 776 (26.2) per the Minecraft Wiki, not yet verified for 777. They are
// meant to be generated from the server data reports (`--reports`) instead of typed by hand.
const LOGIN_ENCRYPTION_REQUEST: i32 = 0x01;
const LOGIN_PLUGIN_REQUEST: i32 = 0x04;
const LOGIN_COOKIE_REQUEST: i32 = 0x05;
const LOGIN_PLUGIN_RESPONSE: i32 = 0x02;
const LOGIN_COOKIE_RESPONSE: i32 = 0x04;

const CONFIG_COOKIE_REQUEST: i32 = 0x00;
const CONFIG_DISCONNECT: i32 = 0x02;
const CONFIG_FINISH: i32 = 0x03;
const CONFIG_KEEP_ALIVE: i32 = 0x04;
const CONFIG_PING: i32 = 0x05;
const CONFIG_KNOWN_PACKS: i32 = 0x0F;
const CONFIG_CODE_OF_CONDUCT: i32 = 0x13;

const CONFIG_COOKIE_RESPONSE: i32 = 0x01;
const CONFIG_ACK_FINISH: i32 = 0x03;
const CONFIG_KEEP_ALIVE_REPLY: i32 = 0x04;
const CONFIG_PONG: i32 = 0x05;
const CONFIG_KNOWN_PACKS_REPLY: i32 = 0x07;

const CONFIG_CLIENT_INFORMATION: i32 = 0x00;
const CONFIG_PLUGIN_MESSAGE: i32 = 0x02;

/// Where to connect and who to be.
#[derive(Debug, Clone)]
pub struct ConnectOptions {
    pub host: String,
    pub port: u16,
    /// Player name (offline mode: the server trusts it).
    pub username: String,
    /// Timeout for connecting and for every step of the login phase.
    pub timeout: Duration,
}

/// An established connection in the play state.
pub struct Connection {
    /// UUID the server assigned to us.
    pub uuid: u128,
    /// Name the server assigned to us.
    pub username: String,
    incoming: Receiver<Result<RawPacket>>,
    outgoing: Sender<RawPacket>,
    stream: TcpStream,
}

struct Session {
    reader: BufReader<TcpStream>,
    writer: TcpStream,
    threshold: Threshold,
    /// Prints every packet of the login and configuration phase to stderr
    /// (enabled by setting the environment variable `FULMEN_TRACE`).
    trace: bool,
}

impl Session {
    fn send(&mut self, packet: &RawPacket) -> Result<()> {
        if self.trace {
            eprintln!("-> id {:#04x}, {} bytes", packet.id, packet.body.len());
        }
        write_packet(&mut self.writer, packet, self.threshold)
    }

    fn recv(&mut self) -> Result<RawPacket> {
        let packet = read_packet(&mut self.reader, self.threshold.is_some())?;
        if self.trace {
            eprintln!("<- id {:#04x}, {} bytes", packet.id, packet.body.len());
        }
        Ok(packet)
    }
}

impl Connection {
    /// Connects, logs in (offline mode) and completes the configuration phase.
    pub fn connect(opts: &ConnectOptions) -> Result<Self> {
        let stream = connect((opts.host.as_str(), opts.port), opts.timeout)?;
        stream.set_read_timeout(Some(opts.timeout))?;
        stream.set_write_timeout(Some(opts.timeout))?;
        stream.set_nodelay(true)?;

        let mut session = Session {
            reader: BufReader::new(stream.try_clone()?),
            writer: stream.try_clone()?,
            threshold: None,
            trace: std::env::var_os("FULMEN_TRACE").is_some(),
        };
        let profile = login(&mut session, opts)?;
        configure(&mut session)?;

        // From here on the game may wait as long as it likes for the next packet.
        stream.set_read_timeout(None)?;

        let Session {
            mut reader,
            mut writer,
            threshold,
            ..
        } = session;
        let (in_tx, incoming) = mpsc::channel();
        let (outgoing, out_rx) = mpsc::channel::<RawPacket>();

        thread::Builder::new()
            .name("fulmen-net-read".into())
            .spawn(move || {
                loop {
                    match read_packet(&mut reader, threshold.is_some()) {
                        Ok(packet) => {
                            if in_tx.send(Ok(packet)).is_err() {
                                break;
                            }
                        }
                        Err(e) => {
                            let _ = in_tx.send(Err(e));
                            break;
                        }
                    }
                }
            })?;
        thread::Builder::new()
            .name("fulmen-net-write".into())
            .spawn(move || {
                while let Ok(packet) = out_rx.recv() {
                    if write_packet(&mut writer, &packet, threshold).is_err() {
                        break;
                    }
                }
            })?;

        Ok(Self {
            uuid: profile.uuid,
            username: profile.username,
            incoming,
            outgoing,
            stream,
        })
    }

    /// Queues a packet for the writer thread.
    pub fn send(&self, packet: RawPacket) -> Result<()> {
        self.outgoing.send(packet).map_err(|_| Error::Closed)
    }

    /// Queues a typed packet for the writer thread.
    pub fn send_typed<P: Packet>(&self, packet: &P) -> Result<()> {
        self.send(RawPacket::of(packet)?)
    }

    /// Waits for the next packet. An error means the connection is over.
    pub fn recv(&self) -> Result<RawPacket> {
        self.incoming.recv().unwrap_or(Err(Error::Closed))
    }

    /// Waits up to `timeout` for the next packet; `Ok(None)` means nothing arrived in time.
    pub fn recv_timeout(&self, timeout: Duration) -> Result<Option<RawPacket>> {
        match self.incoming.recv_timeout(timeout) {
            Ok(packet) => packet.map(Some),
            Err(RecvTimeoutError::Timeout) => Ok(None),
            Err(RecvTimeoutError::Disconnected) => Err(Error::Closed),
        }
    }

    /// Returns the next packet if one is already waiting, without blocking.
    pub fn try_recv(&self) -> Result<Option<RawPacket>> {
        match self.incoming.try_recv() {
            Ok(packet) => packet.map(Some),
            Err(TryRecvError::Empty) => Ok(None),
            Err(TryRecvError::Disconnected) => Err(Error::Closed),
        }
    }
}

impl Drop for Connection {
    fn drop(&mut self) {
        // Unblocks the reader thread; the writer ends when the sender is dropped.
        let _ = self.stream.shutdown(Shutdown::Both);
    }
}

fn login(s: &mut Session, opts: &ConnectOptions) -> Result<LoginSuccess> {
    s.send(&RawPacket::of(&Handshake {
        protocol_version: PROTOCOL_VERSION,
        server_address: opts.host.clone(),
        server_port: opts.port,
        next_state: NextState::Login,
    })?)?;
    s.send(&RawPacket::of(&LoginStart {
        name: opts.username.clone(),
        uuid: offline_uuid(&opts.username),
    })?)?;

    loop {
        let packet = s.recv()?;
        match packet.id {
            LoginDisconnect::ID => {
                return Err(Error::Disconnected(
                    packet.decode::<LoginDisconnect>()?.reason,
                ));
            }
            LOGIN_ENCRYPTION_REQUEST => return Err(Error::EncryptionRequired),
            SetCompression::ID => {
                s.threshold = threshold_from_packet(packet.decode::<SetCompression>()?.threshold);
            }
            LOGIN_PLUGIN_REQUEST => {
                // We understand no login plugin channels: answer "not successful".
                let message_id = read_varint(&mut &packet.body[..])?;
                let mut body = Vec::new();
                write_varint(&mut body, message_id)?;
                body.push(0);
                s.send(&RawPacket::new(LOGIN_PLUGIN_RESPONSE, body))?;
            }
            LOGIN_COOKIE_REQUEST => {
                s.send(&cookie_response(LOGIN_COOKIE_RESPONSE, &packet)?)?;
            }
            LoginSuccess::ID => {
                eprintln!(
                    "login success body ({} bytes): {:02x?}",
                    packet.body.len(),
                    packet.body
                );
                let profile = packet.decode::<LoginSuccess>()?;
                s.send(&RawPacket::of(&LoginAcknowledged)?)?;
                return Ok(profile);
            }
            other => {
                return Err(fulmen_protocol::Error::UnexpectedPacketId {
                    expected: LoginSuccess::ID,
                    got: other,
                }
                .into());
            }
        }
    }
}

fn configure(s: &mut Session) -> Result<()> {
    s.send(&brand()?)?;
    s.send(&client_information()?)?;

    loop {
        let packet = s.recv()?;
        match packet.id {
            CONFIG_DISCONNECT => {
                // The reason is an NBT text component, which we do not decode yet.
                return Err(Error::Disconnected("(reason not decoded)".into()));
            }
            CONFIG_FINISH => {
                s.send(&RawPacket::new(CONFIG_ACK_FINISH, Vec::new()))?;
                return Ok(());
            }
            CONFIG_KEEP_ALIVE => s.send(&RawPacket::new(CONFIG_KEEP_ALIVE_REPLY, packet.body))?,
            CONFIG_PING => s.send(&RawPacket::new(CONFIG_PONG, packet.body))?,
            CONFIG_KNOWN_PACKS => {
                // We have no data packs: an empty list makes the server send everything.
                s.send(&RawPacket::new(CONFIG_KNOWN_PACKS_REPLY, vec![0]))?;
            }
            CONFIG_COOKIE_REQUEST => {
                s.send(&cookie_response(CONFIG_COOKIE_RESPONSE, &packet)?)?;
            }
            CONFIG_CODE_OF_CONDUCT => return Err(Error::Unsupported("code of conduct")),
            // Registry data, tags, feature flags, ...: not needed for a headless client yet.
            _ => {}
        }
    }
}

/// Answers a cookie request with "no cookie stored".
fn cookie_response(id: i32, request: &RawPacket) -> Result<RawPacket> {
    let key = fulmen_protocol::codec::read_string(&mut &request.body[..], 32_767)?;
    let mut body = Vec::new();
    write_string(&mut body, &key, 32_767)?;
    body.push(0);
    Ok(RawPacket::new(id, body))
}

/// Plugin message on the `minecraft:brand` channel.
fn brand() -> Result<RawPacket> {
    let mut body = Vec::new();
    write_string(&mut body, "minecraft:brand", 32_767)?;
    write_string(&mut body, "fulmen", 32_767)?;
    Ok(RawPacket::new(CONFIG_PLUGIN_MESSAGE, body))
}

/// Client Information with plain defaults (field order taken from third-party protocol
/// crates, not yet verified against 777).
fn client_information() -> Result<RawPacket> {
    let mut body = Vec::new();
    write_string(&mut body, "en_us", 16)?; // locale
    write_u8(&mut body, 8)?; // view distance
    write_varint(&mut body, 0)?; // chat mode: enabled
    write_bool(&mut body, true)?; // chat colors
    write_u8(&mut body, 0x7f)?; // displayed skin parts: all
    write_varint(&mut body, 1)?; // main hand: right
    write_bool(&mut body, false)?; // text filtering
    write_bool(&mut body, true)?; // allow server listings
    write_varint(&mut body, 0)?; // particles: all
    Ok(RawPacket::new(CONFIG_CLIENT_INFORMATION, body))
}
