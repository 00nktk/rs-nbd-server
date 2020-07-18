extern crate num_derive;
extern crate num_traits;

// mod request_message;
// mod reply_message;
// mod option;
// mod message;
mod server;
mod protocol;

use std::net::{TcpListener};

use server::Server;

fn main() -> std::io::Result<()> {
    let listener = TcpListener::bind("127.0.0.1:10809")?;

    
    for stream in listener.incoming() {
        Server::new(stream?)?.serve();
    }

    println!("Hello, world!");
    Ok(())
}
