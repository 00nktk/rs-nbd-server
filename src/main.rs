extern crate num_derive;
extern crate num_traits;

mod server;
mod protocol;
mod export;

use std::net::{TcpListener};

use clap::{Arg, App};

use server::{Server, ServerError};

fn main() -> Result<(), ServerError> {
    let matches = App::new("NBD Server")
        .arg(Arg::with_name("input file")
            .required(true)
            .takes_value(true)
            .about("path to export")
        ).arg(Arg::with_name("chunk size")
            .takes_value(true)
            .default_value("4096")
            .about("payload maximum size for chunk of structured reply")
        )
        .get_matches();

    let filename = matches.value_of("input file").unwrap();
    let chunk_size = matches.value_of("chunk size")
        .map(str::parse::<u32>)
        .unwrap()
        .expect("bad chunk size");

    let listener = TcpListener::bind("127.0.0.1:10809")?;
    
    for stream in listener.incoming() {
        match Server::handshake(filename, stream?, chunk_size)?.option_haggle() {
            Ok(mut server) => server.serve()?,
            Err(ServerError::Abort) => eprintln!("client aborted"),
            Err(err) => return Err(err)
        };
    }

    Ok(())
}
