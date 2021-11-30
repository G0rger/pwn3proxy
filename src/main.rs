#[macro_use]
extern crate log;

use std::io::Write;
use std::net::SocketAddr;
use tokio::io::{AsyncBufReadExt, BufReader, stdin};
use tokio::select;
use crate::server::{game, master, server};
use crate::shutdown::Shutdown;

mod shutdown;
mod server;
mod packet;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let remote = std::env::args().nth(1).expect("No remote found").parse().expect("Invalid remote passed");

    let mut stdout = std::io::stdout();
    print!("$ ");
    let _ = stdout.flush();

    log4rs::init_file("log4rs.yml", Default::default()).unwrap();
    let (shutdown_notify, mut shutdown_await, shutdown) = Shutdown::create();

    let localhost = "127.0.0.1".parse().unwrap();
    
    tokio::spawn(server(SocketAddr::new(localhost, 3333), shutdown.clone(), master(SocketAddr::new(remote, 3333))));

    for port in 3000..3011 {
        tokio::spawn(server(SocketAddr::new(localhost, port), shutdown.clone(), game(SocketAddr::new(remote, port))));
    }

    let mut stdin = BufReader::new(stdin()).lines();

    loop {
        select! {
            _ = shutdown_await.recv() => break,
            _ = tokio::signal::ctrl_c() => break,
            Ok(line) = stdin.next_line() => {
                if let Some(line) = line {
                    print!("$ ");
                    let _ = stdout.flush();
                    debug!("{}", line);
                    match &line[..] {
                        "quit" => {
                            println!();
                            break;
                        }
                        _ => {}
                    }
                } else {
                    debug!("Encountered EOF, exiting!");
                    println!();
                    break;
                }
            }
        }
    }

    drop(shutdown);

    let _ = shutdown_notify.send(());
    let _ = shutdown_await.recv().await;

    Ok(())
}
