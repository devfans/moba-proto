
use bytes;
use bytes::BufMut;
use std::error::Error;
use std::{cmp, fmt, io};
use tokio::codec;


#[derive(Debug)]
struct CodecError;
impl fmt::Display for CodecError {
	fn fmt(&self, fmt: &mut fmt::Formatter) -> Result<(), fmt::Error> {
		fmt.write_str("Bad data")
	}
}
impl Error for CodecError {
	fn description(&self) -> &str {
		"Bad data"
	}
}

// =================
// Message Flags
//   0: BattleTest
//   1: BattleInit
//   2: BattleMeta
//   3: BattleStart
//   4: BattleStop
//   5: DataInput
//   6: DataFrame
// ==================
  
#[derive(Debug)]
#[derive(Clone)]
#[allow(dead_code)]
pub enum Message {
    BattleTest {
        raw: Vec<u8>,
    },
    BattleInit {
        player: u8,
        battle: u8,
    },
    BattleMeta {
        battle: u8,
        player: u8,
        raw: Vec<u8>,
    },
    BattleStart {
        battle: u8,
    },
    BattleStop {
        battle: u8,
    },
    DataInput {
        battle: u8,
        player: u8,
        actions: Vec<u8>,
    },
    DataFrame {
        battle: u8,
        frame: usize,
        actions: Vec<u8>,
    },
}

#[allow(dead_code)]
pub enum Status {
    Connected = 0,
    Init = 1,
    Wait = 2,
    Ready = 3,
    Start = 4,
    Stop = 5,
}

impl From<usize> for Status {
    fn from(s: usize) -> Status {
        match s {
            0 => Status::Connected,
            1 => Status::Init,
            2 => Status::Wait,
            3 => Status::Ready,
            4 => Status::Start,
            5 => Status::Stop,
            _ => unreachable!()
        }
    }
}


pub struct MessageFramer {
    cursor: usize,
}

impl MessageFramer {
    pub fn new() -> MessageFramer {
        MessageFramer {
            cursor: 0,
        }
    }
}

impl codec::Decoder for MessageFramer {
    type Item = Message;
	type Error = io::Error;

    #[allow(dead_code)]
    fn decode(&mut self, bytes: &mut bytes::BytesMut) -> Result<Option<Message>, io::Error> {
        if self.cursor != 0 {
            let cur = cmp::min(self.cursor, bytes.len());
            bytes.advance(cur);
            self.cursor -= cur;
            if self.cursor != 0 { return Ok(None); }
        }

        if bytes.len() < 4 { return Ok(None); }
        let len = ((((bytes[3] as usize) << 8) | (bytes[2] as usize)) << 8) | bytes[1] as usize;
        if len + 4 > bytes.len() { return Ok(None); } 
        let mut pos = 4;
        macro_rules! get_slice {
            ( $size: expr ) => {
                {
                    if pos + $size as usize > len + 4 { return Err(io::Error::new(io::ErrorKind::InvalidData, CodecError)); }
                    pos += $size as usize;
                    &bytes[pos - ($size as usize)..pos]
                }
            }
        }

        macro_rules! advance_bytes {
            () => {
                {
                    if pos != len + 4 {
                        return Err(io::Error::new(io::ErrorKind::InvalidData, CodecError));
                    }
                    bytes.advance(pos);
                }
            }
        }

        match bytes[0] {
            0 => {
                let raw_len = get_slice!(1)[0];
                let raw = get_slice!(raw_len).to_vec();
                advance_bytes!();
                Ok(Some(Message::BattleTest { raw }))
            },
            1 => {
                let battle = get_slice!(1)[0];
                let player = get_slice!(1)[0];
                advance_bytes!();
                Ok(Some(Message::BattleInit{ battle, player }))
            },
            2 => {
                let battle = get_slice!(1)[0];
                let player = get_slice!(1)[0];
                let raw_len = get_slice!(1)[0]; 
                let raw = get_slice!(raw_len).to_vec();
                advance_bytes!();
                Ok(Some(Message::BattleMeta { battle, player, raw }))
            },
            3 => {
                let battle = get_slice!(1)[0];
                advance_bytes!();
                Ok(Some(Message::BattleStart{ battle }))
            },
            4 => {
                let battle = get_slice!(1)[0];
                advance_bytes!();
                Ok(Some(Message::BattleStop{ battle }))
            },
            5 => {
                let battle = get_slice!(1)[0];
                let player = get_slice!(1)[0];
                let actions = get_slice!(len - 2).to_vec();
                advance_bytes!();
                Ok(Some(Message::DataInput{ battle, player, actions }))
            },
            6 => {
                let battle = get_slice!(1)[0];
                advance_bytes!();
                Ok(Some(Message::DataFrame{ battle, actions: Vec::new(), frame: 0 as usize }))
            },
            _ => {
				return Err(io::Error::new(io::ErrorKind::InvalidData, CodecError))
			},
        }
    }
}

impl codec::Encoder for MessageFramer {
    type Item = Message;
	type Error = io::Error;
    fn encode(&mut self, msg: Message, res: &mut bytes::BytesMut) -> Result<(), io::Error> {
        println!("TX: {:?}", msg.clone());
        match msg {
            Message::BattleTest { raw } => {
                res.reserve(4 + 1 + raw.len());
                res.put_u8(0);
                res.put_u16_le(1 + raw.len() as u16);
                res.put_u8(0);
                res.put_u8(raw.len() as u8);
                res.put_slice(&raw);
                println!("TX: {:?}", res.clone());
            },
            Message::BattleInit { battle, player } => {
                res.reserve(6);
                res.put_u8(1);
                res.put_u16_le(2 as u16);
                res.put_u8(0);
                res.put_u8(battle);
                res.put_u8(player);
            },
            Message::BattleMeta { battle, player, raw } => {
                res.reserve(7 + raw.len());
                res.put_u8(2);
                res.put_u16_le(3 + raw.len() as u16);
                res.put_u8(0);
                res.put_u8(battle);
                res.put_u8(player);
                res.put_u8(raw.len() as u8);
                res.put_slice(&raw);
            },
            Message::BattleStart { battle } => {
                res.reserve(5);
                res.put_u8(3);
                res.put_u16_le(1 as u16);
                res.put_u8(0);
                res.put_u8(battle);
            },
            Message::BattleStop { battle } => {
                res.reserve(5);
                res.put_u8(4);
                res.put_u16_le(1 as u16);
                res.put_u8(0);
                res.put_u8(battle);
            },
            Message::DataInput { battle, player: _, actions: _ } => {
                res.reserve(5);
                res.put_u8(5);
                res.put_u16_le(1 as u16);
                res.put_u8(0);
                res.put_u8(battle);
            },
            Message::DataFrame { battle, frame, actions } => {
                res.reserve(5 + 4 + actions.len());
                res.put_u8(6);
                res.put_u16_le(1 + 4 + actions.len() as u16);
                res.put_u8(0);
                res.put_u8(battle);
                res.put_u32_le(frame as u32);
                res.put_slice(&actions);
            },
        }
        println!("TX: {:?}", res.clone());
        Ok(())
    }

}
