mod ext {
    use bytes::{Buf, BufMut};
    use super::game::Error;

    type Result<T> = std::result::Result<T, Error>;

    pub trait ReadExt {
        fn read_u8(&mut self) -> Result<u8>;
        fn read_u16(&mut self) -> Result<u16>;
        fn read_u16_le(&mut self) -> Result<u16>;
        fn read_i32_le(&mut self) -> Result<i32>;
        fn read_u32_le(&mut self) -> Result<u32>;
        fn read_f32_le(&mut self) -> Result<f32>;

        fn read_string(&mut self) -> Result<String>;
    }

    macro_rules! read_buf {
        ($name:ident, $buf_name:ident, $type:ty) => {
            fn $name(&mut self) -> Result<$type> {
                if Buf::remaining(self) >= std::mem::size_of::<$type>() {
                    Ok(Buf::$buf_name(self))
                } else {
                    Err(Error::Incomplete)
                }
            }
        };
    }

    impl<R: Buf + ?Sized> ReadExt for R {
        read_buf!(read_u8, get_u8, u8);
        read_buf!(read_u16, get_u16, u16);
        read_buf!(read_u16_le, get_u16_le, u16);
        read_buf!(read_i32_le, get_i32_le, i32);
        read_buf!(read_u32_le, get_u32_le, u32);
        read_buf!(read_f32_le, get_f32_le, f32);

        fn read_string(&mut self) -> Result<String> {
            let len = self.read_u16_le()? as usize;
            if Buf::remaining(self) >= len {
                let mut buf = vec![0u8; len];
                Buf::copy_to_slice(self, &mut buf);
                Ok(String::from_utf8_lossy(&buf).into())
            } else {
                Err(Error::Incomplete)
            }
        }
    }

    pub trait WriteExt {
        fn write_u8(&mut self, n: u8);
        fn write_u16(&mut self, n: u16);
        fn write_u16_le(&mut self, n: u16);
        fn write_i32_le(&mut self, n: i32);
        fn write_u32_le(&mut self, n: u32);
        fn write_f32_le(&mut self, n: f32);

        fn write_string(&mut self, n: &str);
    }

    macro_rules! write_buf {
        ($name:ident, $name_buf:ident, $type:ty) => {
            fn $name(&mut self, n: $type) {
                BufMut::$name_buf(self, n);
            }
        };
    }

    impl<W: BufMut + ?Sized> WriteExt for W {
        write_buf!(write_u8, put_u8, u8);
        write_buf!(write_u16, put_u16, u16);
        write_buf!(write_u16_le, put_u16_le, u16);
        write_buf!(write_i32_le, put_i32_le, i32);
        write_buf!(write_u32_le, put_u32_le, u32);
        write_buf!(write_f32_le, put_f32_le, f32);

        fn write_string(&mut self, n: &str) {
            let buf = n.as_bytes();
            self.write_u16_le(buf.len() as u16);
            BufMut::put_slice(self, buf);
        }
    }
}

pub mod game {
    use std::fmt::Debug;
    use std::sync::Arc;
    use parking_lot::Mutex;
    use tokio::sync::mpsc;
    use thiserror::Error;
    use Direction::{Client, Server};
    use Packet::*;
    use Error::*;
    use super::ext::{ReadExt, WriteExt};

    #[derive(Error, Debug)]
    pub enum Error {
        #[error("Invalid packet")]
        Invalid,
        #[error("There aren't enough bytes yet")]
        Incomplete,
    }

    type Result<T> = std::result::Result<T, Error>;

    #[derive(Debug, Copy, Clone, Eq, PartialEq)]
    pub enum Direction {
        Server, Client
    }

    #[derive(Debug, Clone)]
    pub enum Packet {
    }

    impl Packet {
        pub fn parse<R>(origin: Direction, stream: &mut R) -> Result<Packet>
        where R: ReadExt {
            Ok(match id {
                _ => return Err(Invalid)
            })
        }

        pub fn write_to<W>(self, target: Direction, stream: &mut W)
        where W: WriteExt {
            match self {
            };
        }

        pub async fn handle(self, origin: Direction, server_queue: &mpsc::Sender<Packet>, client_queue: &mpsc::Sender<Packet>, _state: Arc<Mutex<State>>) {
            match origin {
                Server => {
                    match &self {
                        _ => {}
                    }
                    let _ = client_queue.send(self).await;
                }
                Client => {
                    match &self {
                        _ => {}
                    }
                    let _ = server_queue.send(self).await;
                }
            }
        }
    }

    #[derive(Debug, Default)]
    pub struct State {}
}
