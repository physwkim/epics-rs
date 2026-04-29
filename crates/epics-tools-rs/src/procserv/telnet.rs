//! Minimal telnet IAC parser/encoder.
//!
//! C procServ uses libtelnet but only exercises a tiny subset:
//! - 5 functions: `telnet_init` / `negotiate` / `recv` / `send` / `free`
//! - 3 events: DATA, SEND, ERROR
//! - 2 telnet options: `WILL ECHO`, `DO LINEMODE`
//!
//! Vendoring libtelnet just for this is overkill, so we hand-roll a
//! ~100 LOC IAC state machine.
//!
//! ## Wire format reminder
//!
//! ```text
//! IAC = 0xFF
//! IAC IAC          → literal 0xFF byte (escape)
//! IAC <cmd>        → 2-byte command (e.g. NOP, AYT, BRK)
//! IAC <neg> <opt>  → 3-byte negotiation (WILL/WONT/DO/DONT + option)
//! IAC SB <opt> ... IAC SE  → subnegotiation (we only need to skip these)
//! ```

/// Telnet protocol bytes we care about.
#[allow(dead_code)]
pub mod codes {
    pub const IAC: u8 = 0xFF;
    pub const DONT: u8 = 0xFE;
    pub const DO: u8 = 0xFD;
    pub const WONT: u8 = 0xFC;
    pub const WILL: u8 = 0xFB;
    pub const SB: u8 = 0xFA;
    pub const SE: u8 = 0xF0;

    pub const TELOPT_ECHO: u8 = 0x01;
    pub const TELOPT_SGA: u8 = 0x03; // suppress-go-ahead
    pub const TELOPT_LINEMODE: u8 = 0x22;
}

/// Output of one feed into [`TelnetParser::feed`]. The supervisor
/// task forwards `Data` to [`super::client::ClientConnection`]'s
/// input handler and writes `Reply` back to the socket.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TelnetEvent {
    /// Plain user data (IAC sequences stripped, IAC-IAC unescaped).
    Data(Vec<u8>),
    /// Bytes to write back to the peer (responses to negotiations).
    Reply(Vec<u8>),
}

/// Streaming IAC parser. Hold one per client socket.
#[derive(Debug, Default)]
pub struct TelnetParser {
    state: ParseState,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
enum ParseState {
    #[default]
    Data,
    Iac,
    Negotiate(u8), // command (WILL/WONT/DO/DONT) recorded
    Subneg,
    SubnegIac,
}

impl TelnetParser {
    pub fn new() -> Self {
        Self::default()
    }

    /// Feed raw bytes from the socket; returns the events produced.
    pub fn feed(&mut self, input: &[u8]) -> Vec<TelnetEvent> {
        let mut events = Vec::new();
        let mut data_buf = Vec::with_capacity(input.len());
        let mut reply_buf = Vec::new();

        for &b in input {
            self.state = match self.state {
                ParseState::Data => {
                    if b == codes::IAC {
                        ParseState::Iac
                    } else {
                        data_buf.push(b);
                        ParseState::Data
                    }
                }
                ParseState::Iac => match b {
                    codes::IAC => {
                        // Escaped 0xFF byte.
                        data_buf.push(0xFF);
                        ParseState::Data
                    }
                    codes::WILL | codes::WONT | codes::DO | codes::DONT => ParseState::Negotiate(b),
                    codes::SB => ParseState::Subneg,
                    _ => {
                        // Single-byte command (NOP, AYT, EC, EL, …).
                        // We ignore them per procServ semantics.
                        ParseState::Data
                    }
                },
                ParseState::Negotiate(cmd) => {
                    // Auto-respond conservatively: refuse all options
                    // we didn't actively offer. C procServ negotiates
                    // WILL ECHO + DO LINEMODE at startup; further
                    // requests get refused. Mirroring that here.
                    let response = match cmd {
                        codes::WILL => codes::DONT, // they offer; we refuse
                        codes::DO => codes::WONT,   // they ask; we refuse
                        _ => 0,
                    };
                    if response != 0 {
                        reply_buf.extend_from_slice(&[codes::IAC, response, b]);
                    }
                    ParseState::Data
                }
                ParseState::Subneg => {
                    if b == codes::IAC {
                        ParseState::SubnegIac
                    } else {
                        // Discard subnegotiation payload — we don't
                        // care about the specifics.
                        ParseState::Subneg
                    }
                }
                ParseState::SubnegIac => match b {
                    codes::SE => ParseState::Data,
                    codes::IAC => ParseState::Subneg,
                    _ => ParseState::Subneg,
                },
            };
        }

        if !data_buf.is_empty() {
            events.push(TelnetEvent::Data(data_buf));
        }
        if !reply_buf.is_empty() {
            events.push(TelnetEvent::Reply(reply_buf));
        }
        events
    }
}

/// Build the initial negotiation handshake to send when a client
/// connects. Mirrors C procServ's `telnet_negotiate` calls in
/// `clientItem::clientItem`: announce `WILL ECHO` + `DO LINEMODE`.
pub fn initial_negotiation() -> Vec<u8> {
    vec![
        codes::IAC,
        codes::WILL,
        codes::TELOPT_ECHO,
        codes::IAC,
        codes::DO,
        codes::TELOPT_LINEMODE,
    ]
}

/// IAC-encode an outgoing data buffer: any literal `0xFF` is
/// doubled. Other bytes pass through. Equivalent to libtelnet's
/// `telnet_send` for the raw-data path.
pub fn iac_escape(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len());
    for &b in data {
        if b == codes::IAC {
            out.push(codes::IAC);
            out.push(codes::IAC);
        } else {
            out.push(b);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn passes_plain_data_through() {
        let mut p = TelnetParser::new();
        let evs = p.feed(b"hello");
        assert_eq!(evs, vec![TelnetEvent::Data(b"hello".to_vec())]);
    }

    #[test]
    fn unescapes_iac_iac() {
        let mut p = TelnetParser::new();
        let evs = p.feed(&[b'a', codes::IAC, codes::IAC, b'b']);
        assert_eq!(evs, vec![TelnetEvent::Data(vec![b'a', 0xFF, b'b'])]);
    }

    #[test]
    fn refuses_unknown_will() {
        let mut p = TelnetParser::new();
        // Peer offers WILL ECHO — we refuse with DONT ECHO (procServ
        // always sends its own WILL ECHO, doesn't accept theirs).
        let evs = p.feed(&[codes::IAC, codes::WILL, codes::TELOPT_ECHO]);
        assert_eq!(
            evs,
            vec![TelnetEvent::Reply(vec![
                codes::IAC,
                codes::DONT,
                codes::TELOPT_ECHO,
            ])]
        );
    }

    #[test]
    fn skips_subnegotiation_block() {
        let mut p = TelnetParser::new();
        let evs = p.feed(&[
            b'a',
            codes::IAC,
            codes::SB,
            0x18,
            0x01,
            0x02,
            codes::IAC,
            codes::SE,
            b'b',
        ]);
        assert_eq!(evs, vec![TelnetEvent::Data(vec![b'a', b'b'])]);
    }

    #[test]
    fn iac_escape_doubles_ff() {
        assert_eq!(iac_escape(&[1, 0xFF, 2]), vec![1, 0xFF, 0xFF, 2]);
    }
}
