extern crate num_derive;
extern crate num_traits;

mod server;
mod protocol;
mod export;

use std::net::{TcpListener};

use server::Server;

fn main() -> std::io::Result<()> {
    let listener = TcpListener::bind("127.0.0.1:10809")?;

    
    for stream in listener.incoming() {
        Server::handshake(stream?)?.option_haggle()?.serve()?;
    }

    println!("Hello, world!");
    Ok(())
}
