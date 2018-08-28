
use bytes;
use bytes::BufMut;
use std::error::Error;
use std::{cmp, fmt, io};
use tokio_io::codec;


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

#[derive(Debug)]
#[derive(Clone)]
pub enum Message {
    BattleReady {
        raw: Vec<u8>,
    },
}

#[allow(dead_code)]
pub enum Status {
    Connected,
    Ready,
    Start,
    End,
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
        println!("RX: {:?}", bytes.clone());
        if self.cursor != 0 {
            let cur = cmp::min(self.cursor, bytes.len());
            bytes.advance(cur);
            self.cursor -= cur;
            if self.cursor != 0 { return Ok(None); }
        }

        if bytes.len() < 4 { return Ok(None); }
        let len = ((((bytes[3] as usize) << 8) | (bytes[2] as usize)) << 8) | bytes[1] as usize;

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
            11 => {
                let raw_len = get_slice!(1)[0];
                let raw = get_slice!(raw_len).to_vec();
                advance_bytes!();
                Ok(Some(Message::BattleReady { raw }))
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
        match msg {
            Message::BattleReady { raw } => {
                res.reserve(4 + 1 + raw.len());
                res.put_u8(11);
                res.put_u16_le(1 + raw.len() as u16);
                res.put_u8(0);
                res.put_u8(raw.len() as u8);
                res.put_slice(&raw);
                println!("TX: {:?}", res.clone());
            }
        }
        Ok(())
    }

}
