use std::io::prelude::*;
use std::io::SeekFrom;
use std::fs::File;
use std::net::TcpStream;
use std::convert::TryInto;

use crate::protocol::request as req;
use crate::protocol::reply as rpl;
use crate::protocol::option as opt;
use crate::protocol::send_msg;

use num_traits::{ToPrimitive};

const HS_FLAGS: u16 = 0b0000000000000011; // !NO_ZEROS && FIXED NEWSTYLE

const NBDMAGIC: u64 = 0x4e42444d41474943;
const IHAVEOPT: u64 = 0x49484156454F5054;

// TODO: make generic over input_stream
pub struct Server {
    input_stream: TcpStream,
    export: File,
    export_len: u64,
}

// ?: move this to protocol
fn handshake(stream: &mut TcpStream) -> std::io::Result<()> {
    let mut buf = [0; 4];

    stream.write_all(&NBDMAGIC.to_be_bytes())?;
    stream.write_all(&IHAVEOPT.to_be_bytes())?;
    println!("{:#016b}", HS_FLAGS);
    stream.write_all(&HS_FLAGS.to_be_bytes())?;  // TODO: fix this
    stream.flush()?;
    stream.read_exact(&mut buf)?;

    println!("{:?}", buf);
    // if u32::from_be_bytes(buf) != HS_FLAGS as u32 { panic!("wrong client flags"); }

    Ok(())
}

impl Server {
    pub fn new(mut stream: TcpStream) -> std::io::Result<Self> {
        
        handshake(&mut stream)?;
        let mut input_buffer = vec![0; 1024];
        
        let file = File::open("test.img").unwrap();
        let flen = file.metadata().unwrap().len();
        
        loop {            
            let mut buf = [0; 8];
            stream.read_exact(&mut buf)?;
            assert_eq!(&buf, b"IHAVEOPT");
            
            let bytes_read = stream.read(&mut input_buffer)?;
            if bytes_read < 8 { panic!("too short"); }
            
            match input_buffer[..bytes_read].try_into() {
                Ok(option) => {
                    println!("got opt: {:?}", option);
                    match option {
                        opt::NbdOption::Go(_name, _infos) => {
                            let info: Vec<u8> = opt::Info::Export.to_u16().unwrap()
                                .to_be_bytes().iter()
                                .chain(flen.to_be_bytes().iter())
                                .chain(0b0000000000000011u16.to_be_bytes().iter())
                                .copied().collect();

                            let reply = opt::Reply::new(
                                opt::NbdOption::Go(None, None).into(), 
                                opt::OptionReplyType::Info, 
                                Some(info)
                            );

                            send_msg(&mut stream, reply)?;

                            let reply = opt::Reply::new(
                                opt::NbdOption::Go(None, None).into(), 
                                opt::OptionReplyType::Ack, 
                                None
                            );
                            
                            send_msg(&mut stream, reply)?;
                            
                            break;
                        },

                        //  TODO: fix this
                        opt::NbdOption::ExportName => {
                            let reply = opt::Reply::new(
                                opt::NbdOption::ExportName.into(), 
                                opt::OptionReplyType::ErrUnsup, 
                                None
                            );

                            send_msg(&mut stream, reply)?;
                        },
                        ot => {
                            let reply = opt::Reply::new(
                                ot.into(), 
                                opt::OptionReplyType::ErrUnsup, 
                                None
                            );

                            send_msg(&mut stream, reply)?;
                        },
                    }
                },
                Err(e) => {
                    println!("option parse error: {:?}", e); 
                }
            }
        }
        
        
        Ok ( Self {
            input_stream: stream,
            export: file,
            export_len: flen
        })
    }

    pub fn serve(&mut self) {
        let mut magic_buf = [0; 4];

        let mut header_buf = [0; 24];

        loop {

            // ?: consider moving magic check to request code   
            // TODO: remove panic and unwrap
            self.input_stream.read_exact(&mut magic_buf).unwrap();
            if u32::from_be_bytes(magic_buf) != req::REQMAGIC {
                panic!("wrong request magic");
            }
            
            let bytes_read = self.input_stream.read(&mut header_buf).unwrap();

            if bytes_read == 24 {

                // TODO: fix for a case when data > buffer 
                match header_buf[..bytes_read].try_into() {
                    Ok(r) => { 
                        match self.handle_request(r) {
                            rpl::Reply::Disconnect => break,

                            rpl::Reply::Simple(reply) => 
                                send_msg(&mut self.input_stream, reply).unwrap(),

                            rpl::Reply::Structured(replies) => {
                                replies.for_each(|msg| send_msg(&mut self.input_stream, msg).unwrap())
                            }
                        }
                    },
                    Err(e) => {
                        eprintln!("{:?}", e);
                        unimplemented!()
                    }
                }

            } else {
                eprintln!("request message too short");
            }
        }
    }

    fn handle_request(&mut self, request: req::Request) -> rpl::Reply {
        eprintln!("got request: {:?}", request);

        match request.type_ {
            req::RequestType::Read => {
                if request.offset + request.len as u64 > self.export_len {
                    panic!("bad file access");
                }

                let mut buf = vec![0u8; request.len as usize];  // !FIXME: can panic 

                self.export.seek(SeekFrom::Start(request.offset)).unwrap();
                let _ = self.export.read(&mut buf);

                rpl::Reply::Simple( 
                    rpl::SimpleReply::new(0, request.handle, Some(buf))
                )
            },
            req::RequestType::Write => unimplemented!(),
            req::RequestType::Disc => rpl::Reply::Disconnect,
            req::RequestType::Flush => unimplemented!(),
            req::RequestType::Trim => unimplemented!(),
            req::RequestType::Cache => unimplemented!(),
            req::RequestType::WriteZeroes => unimplemented!(),
            req::RequestType::BlockStatus => unimplemented!(),
            req::RequestType::Resize => unimplemented!(),
        }
    }
}