use std::future::Future;
use crate::shutdown::Shutdown;
use tokio::io::{AsyncReadExt, copy, Result};
use std::net::{SocketAddr};
use std::pin::Pin;
use std::sync::Arc;
use bytes::{Buf, BufMut, BytesMut};
use parking_lot::Mutex;
use tokio::io::AsyncWriteExt;
use tokio::net::{TcpListener, TcpStream};
use tokio::select;
use tokio::sync::mpsc;
use crate::packet::game::Direction;
use crate::packet::game::Error::*;

pub async fn server<F, Fut>(address: SocketAddr, mut shutdown: Shutdown, handler: F) -> Result<()>
where
    F: Fn(TcpStream, Shutdown) -> Fut,
    Fut: Future<Output=Result<()>> + Send + 'static
{
    let server = TcpListener::bind(address).await?;
    loop {
        select! {
            _ = shutdown.wait() => break,
            stream = server.accept() => {
                match stream {
                    Ok((stream, _)) => {
                        info!("Client connected at {}", address);
                        tokio::spawn(handler(stream, shutdown.clone()));
                    }
                    Err(err) => {
                        return Err(err);
                    }
                }
            }
        }
    }
    Ok(())
}

pub fn master(remote: SocketAddr) -> impl Fn(TcpStream, Shutdown) -> Pin<Box<dyn Future<Output=Result<()>> + Send + 'static>> {
    move |mut stream: TcpStream, mut shutdown: Shutdown| Box::pin(async move {
        let mut remote = TcpStream::connect(remote).await?;

        {
            let (mut cr, mut cw) = stream.split();
            let (mut rr, mut rw) = remote.split();

            select! {
                _ = shutdown.wait() => {},
                _ = tokio::io::copy(&mut cr, &mut rw) => {},
                _ = tokio::io::copy(&mut rr, &mut cw) => {},
            }
        }

        remote.shutdown().await?;
        stream.shutdown().await?;

        Ok(())
    })
}

pub fn game(remote: SocketAddr) -> impl Fn(TcpStream, Shutdown) -> Pin<Box<dyn Future<Output=Result<()>> + Send + 'static>> {
    move |mut stream: TcpStream, mut shutdown: Shutdown| Box::pin(async move {
        use crate::packet::game::Packet;

        let mut remote = TcpStream::connect(remote).await?;

        {
            let (mut cr, mut cw) = stream.split();
            let (mut rr, mut rw) = remote.split();

            let (server_send, mut server_recv) = mpsc::channel::<Packet>(64);
            let (client_send, mut client_recv) = mpsc::channel::<Packet>(64);

            let mut client_recv_buffer = BytesMut::with_capacity(2048);
            let mut client_send_buffer = BytesMut::with_capacity(2048);
            let mut server_recv_buffer = BytesMut::with_capacity(2048);
            let mut server_send_buffer = BytesMut::with_capacity(2048);

            let state = Arc::new(Mutex::new(Default::default()));

            let stop: Option<Direction> = select! {
                _ = shutdown.wait() => None,
                err = async {
                    while let Some(packet) = server_recv.recv().await {
                        packet.write_to(Direction::Server, &mut server_send_buffer);
                        rw.write_buf(&mut server_send_buffer).await?;
                    }

                    std::io::Result::Ok(None)
                } => match err {
                    Err(err) => match err.kind() {
                        std::io::ErrorKind::UnexpectedEof => None,
                        _ => return Err(err),
                    },
                    Ok(ok) => ok
                },
                err = async {
                    while let Some(packet) = client_recv.recv().await {
                        packet.write_to(Direction::Client, &mut client_send_buffer);
                        cw.write_buf(&mut client_send_buffer).await?;
                    }

                    std::io::Result::Ok(None)
                } => match err {
                    Err(err) => match err.kind() {
                        std::io::ErrorKind::UnexpectedEof => None,
                        _ => return Err(err),
                    },
                    Ok(ok) => ok
                },
                err = async {
                    let client = client_send.clone();
                    let server = server_send.clone();
                    let packet = loop {
                        let mut buf = &server_recv_buffer[..];
                        match Packet::parse(Direction::Server, &mut buf) {
                            Ok(packet) => {
                                let len = server_recv_buffer.remaining() - buf.remaining();
                                drop(buf);
                                server_recv_buffer.advance(len);
                                packet.handle(Direction::Server, &server, &client, state.clone()).await;
                            }
                            Err(err) => match err {
                                Incomplete => {
                                    let remaining = server_recv_buffer.has_remaining_mut();
                                    let amount = rr.read_buf(&mut server_recv_buffer).await?;
                                    if remaining && amount == 0 {
                                        break None;
                                    }
                                }
                                Invalid => break Some(Direction::Server)
                            }
                        }
                    };

                    std::io::Result::Ok(packet)
                } => match err {
                    Err(err) => match err.kind() {
                        std::io::ErrorKind::UnexpectedEof => None,
                        _ => return Err(err),
                    },
                    Ok(ok) => ok
                },
                err = async {
                    let client = client_send.clone();
                    let server = server_send.clone();
                    let packet = loop {
                        let mut buf = &client_recv_buffer[..];
                        match Packet::parse(Direction::Client, &mut buf) {
                            Ok(packet) => {
                                let len = client_recv_buffer.remaining() - buf.remaining();
                                drop(buf);
                                client_recv_buffer.advance(len);
                                packet.handle(Direction::Client, &server, &client, state.clone()).await;
                            }
                            Err(err) => match err {
                                Incomplete => {
                                    let remaining = client_recv_buffer.has_remaining_mut();
                                    let amount = cr.read_buf(&mut client_recv_buffer).await?;
                                    if remaining && amount == 0 {
                                        break None;
                                    }
                                }
                                Invalid => break Some(Direction::Client)
                            }
                        }
                    };

                    std::io::Result::Ok(packet)
                } => match err {
                    Err(err) => match err.kind() {
                        std::io::ErrorKind::UnexpectedEof => None,
                        _ => return Err(err),
                    },
                    Ok(ok) => ok
                },
            };

            drop(server_send);
            drop(client_send);

            if let Some(origin) = stop {
                rw.write_all_buf(&mut server_send_buffer).await?;
                cw.write_all_buf(&mut client_send_buffer).await?;

                while let Some(packet) = server_recv.recv().await {
                    packet.write_to(Direction::Server, &mut server_send_buffer);
                    cw.write_all_buf(&mut client_send_buffer).await?;
                }
                while let Some(packet) = client_recv.recv().await {
                    packet.write_to(Direction::Client, &mut client_send_buffer);
                    rw.write_all_buf(&mut server_send_buffer).await?;
                }

                drop(server_recv);
                drop(client_recv);
                drop(server_send_buffer);
                drop(client_send_buffer);

                match origin {
                    Direction::Server => {
                        let buf = &server_recv_buffer[..];
                        info!("Encountered invalid packet from Server!");
                        info!("{} bytes for packet: {:02x?}", buf.len(), buf);
                        cw.write_all_buf(&mut server_recv_buffer).await?;
                        cr.read_buf(&mut client_recv_buffer).await?;
                        info!("Client responded with {:02x?}", &client_recv_buffer[..])
                    }
                    Direction::Client => {
                        let buf = &client_recv_buffer[..];
                        info!("Encountered invalid packet from Client!");
                        info!("{} bytes for packet: {:02x?}", buf.len(), buf);
                        rw.write_all_buf(&mut client_recv_buffer).await?;
                        rr.read_buf(&mut server_recv_buffer).await?;
                        info!("Server responded with {:02x?}", &server_recv_buffer[..])
                    }
                }

                rw.write_all_buf(&mut client_recv_buffer).await?;
                cw.write_all_buf(&mut server_recv_buffer).await?;

                drop(client_recv_buffer);
                drop(server_recv_buffer);

                select! {
                    _ = shutdown.wait() => {},
                    _ = copy(&mut cr, &mut rw) => {},
                    _ = copy(&mut rr, &mut cw) => {},
                }
            }
        }

        remote.shutdown().await?;
        stream.shutdown().await?;

        info!("Client disconnected!");

        Ok(())
    })
}
