use std::io::prelude::*;
use std::net::TcpStream;
use std::convert::TryInto;
use std::rc::Rc;

use crate::export::{Export};
use crate::protocol::request as req;
use crate::protocol::reply as rpl;
use crate::protocol::option as opt;
use crate::protocol::send_msg;

use num_traits::{ToPrimitive};

const HS_FLAGS: u16 = 0b0000000000000011; // !NO_ZEROS && FIXED NEWSTYLE
const TRANSMISSION_FLAGS: u16 = 0b0000000000000011;

const NBDMAGIC: u64 = 0x4e42444d41474943;
const IHAVEOPT: u64 = 0x49484156454F5054;

// TODO: make generic over input_stream
pub struct Server {
    ready: bool,
    use_structured: bool,
    input_stream: TcpStream,
    export: Rc<Export>,
}

// ?: move this to protocol
fn handshake(stream: &mut TcpStream) -> std::io::Result<()> {
    let mut buf = [0; 4];

    stream.write_all(&NBDMAGIC.to_be_bytes())?;
    stream.write_all(&IHAVEOPT.to_be_bytes())?;
    stream.write_all(&HS_FLAGS.to_be_bytes())?;  // TODO: fix this
    stream.flush()?;
    stream.read_exact(&mut buf)?;

    // if u32::from_be_bytes(buf) != HS_FLAGS as u32 { panic!("wrong client flags"); }

    Ok(())
}

impl Server {
    pub fn handshake(mut stream: TcpStream) -> std::io::Result<Self> {
        
        handshake(&mut stream)?;
        let use_structured = false;

        let export = Rc::new(Export::new("test.img".to_owned())?);

        Ok ( Self {
            ready: false,
            use_structured,
            input_stream: stream,
            export
        })
    }

    pub fn option_haggle(mut self) -> std::io::Result<Self> {
        let mut input_buffer = vec![0; 1024];
        let (mut end, mut unparsed) = (0, false);
        
        loop {
            
            if !unparsed {
                let mut buf = [0; 8];
                self.input_stream.read_exact(&mut buf)?;
                assert_eq!(&buf, b"IHAVEOPT");

                let bytes_read = self.input_stream.read(&mut input_buffer)?;
                end = 0 + bytes_read;
            }
            unparsed = false;

            match input_buffer[..end].try_into() {
                Ok(option) => {
                    println!("got opt: {:?}", option);
                    match option {
                        opt::NbdOption::Info(_, info_requests) => {
                            let mut infos = Self::get_info(Rc::clone(&self.export), info_requests);

                            infos.try_for_each(|reply|
                                send_msg(&mut self.input_stream, reply)
                            )?;

                            let reply = opt::Reply::new(
                                opt::NbdOption::Info(None, None).into(), 
                                opt::OptionReplyType::Ack, 
                                None
                            );

                            send_msg(&mut self.input_stream, reply)?;
                        },

                        opt::NbdOption::Go(_name, _infos) => {
                            let info: Vec<u8> = opt::Info::Export.to_u16().unwrap()
                                .to_be_bytes().iter()
                                .chain(self.export.size.to_be_bytes().iter())
                                .chain(TRANSMISSION_FLAGS.to_be_bytes().iter())
                                .copied().collect();

                            let reply = opt::Reply::new(
                                opt::NbdOption::Go(None, None).into(), 
                                opt::OptionReplyType::Info, 
                                Some(info)
                            );

                            send_msg(&mut self.input_stream, reply)?;

                            let reply = opt::Reply::new(
                                opt::NbdOption::Go(None, None).into(), 
                                opt::OptionReplyType::Ack, 
                                None
                            );
                            
                            send_msg(&mut self.input_stream, reply)?;
                            
                            break;
                        },

                        //  TODO: fix this
                        opt::NbdOption::ExportName => {
                            let reply = opt::Reply::new(
                                opt::NbdOption::ExportName.into(), 
                                opt::OptionReplyType::ErrUnsup, 
                                None
                            );

                            send_msg(&mut self.input_stream, reply)?;
                        },

                        opt::NbdOption::StructuredReply(allow) => {             //  The client MUST NOT send any additional data
                            let reply = opt::Reply::new(                        //  with the option, and the server SHOULD reject
                                opt::NbdOption::StructuredReply(false).into(),  //  a request that includes data with 
                                if allow { opt::OptionReplyType::Ack }          //  `NBD_REP_ERR_INVALID`
                                else { opt::OptionReplyType::ErrInvalid },
                                None
                            );

                            send_msg(&mut self.input_stream, reply)?;
                            self.use_structured = allow;
                        }

                        opt::NbdOption::List => {                            
                            let data = (self.export.name.len() as u32).to_be_bytes().iter()
                                .chain(self.export.name.as_bytes())
                                .copied()
                                .collect();
                            
                            let reply = opt::Reply::new(
                                opt::NbdOption::List.into(),
                                opt::OptionReplyType::Server,
                                Some(data)
                            );

                            send_msg(&mut self.input_stream, reply)?;

                            let reply = opt::Reply::new(
                                opt::NbdOption::List.into(),
                                opt::OptionReplyType::Ack,
                                None
                            );

                            send_msg(&mut self.input_stream, reply)?;
                        }

                        opt::NbdOption::Abort => {
                            use std::io::{Error, ErrorKind};

                            let reply = opt::Reply::new(
                                opt::NbdOption::Abort.into(),
                                opt::OptionReplyType::Ack,
                                None
                            );

                            send_msg(&mut self.input_stream, reply)?;
                            return Err(Error::new(ErrorKind::Other, "client aborted"))
                        }

                        ot => {
                            let reply = opt::Reply::new(
                                ot.into(), 
                                opt::OptionReplyType::ErrUnsup, 
                                None
                            );

                            send_msg(&mut self.input_stream, reply)?;
                        },
                    }
                },

                Err(opt::OptionError::RequireData(how_much)) => {                    
                    let bytes_read = self.input_stream.read(&mut input_buffer[end..(end+how_much)])?;
                    unparsed = true;
                    end += bytes_read;
                }

                Err(e) => {
                    println!("option parse error: {:?}", e); 
                }
            }
        }

        self.ready = true;
        Rc::get_mut(&mut self.export).unwrap().load()?;  //  should not fail
        Ok(self)
    }

    pub fn serve(&mut self) -> std::io::Result<()> {
        use std::io::{Error, ErrorKind};

        if !self.ready { return Err(Error::new(ErrorKind::Other, "needs option haggling")) }
        
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
                        let reply_ = self.handle_request(r); 
                        match reply_ {
                            rpl::Reply::Disconnect => break,

                            rpl::Reply::Simple(reply) => {
                                send_msg(&mut self.input_stream, reply).unwrap()
                            },

                            rpl::Reply::Structured(replies) => {
                                let s = &mut self.input_stream;
                                replies.for_each(|msg| send_msg(s, msg).unwrap())
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

        Ok(())
    }

    fn handle_request(&self, request: req::Request) -> rpl::Reply {
        match request.type_ {
            req::RequestType::Read => 
                if self.use_structured {
                    rpl::Reply::Structured(
                        rpl::StructuredReply::read_from_offset(
                            Rc::clone(&self.export), request.handle, request.offset, request.len, 1024
                        )
                    )
                } else {
                    if request.offset + request.len as u64 > self.export.size {
                        panic!("bad file access");                  // !FIXME
                    }
                    rpl::Reply::Simple( 
                        rpl::SimpleReply::new(
                            0, request.handle, Some(self.export.read(request.offset, request.len as usize).unwrap())
                        )
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

    fn get_info(export: Rc<Export>, info_requests: Option<Vec<opt::Info>>) -> impl Iterator<Item = opt::Reply> {
        let export_size: u64 = export.size;
        let export_name: String = export.name.clone();

        let mut info_requests = info_requests.unwrap_or(Vec::new());
        if !info_requests.contains(&opt::Info::Export) { 
            info_requests.insert(0, opt::Info::Export); 
        }

        info_requests.into_iter()
                .map(move |info_type| match info_type {
                    opt::Info::Export => 
                        opt::Info::Export.to_u16().unwrap()
                            .to_be_bytes().iter()
                            .chain(export_size.to_be_bytes().iter())
                            .chain(TRANSMISSION_FLAGS.to_be_bytes().iter())
                            .copied().collect(),
                    
                    opt::Info::Name => 
                        opt::Info::Name.to_u16().unwrap()
                            .to_be_bytes().iter()
                            .chain(export_name.clone().into_bytes().iter())
                            .copied().collect(),

                    opt::Info::Description =>
                        opt::Info::Description.to_u16().unwrap()
                            .to_be_bytes().iter()
                            .copied().collect(),

                    opt::Info::BlockSize =>
                        opt::Info::BlockSize.to_u16().unwrap()
                            .to_be_bytes().iter()
                            .chain(512_u32.to_be_bytes().iter())
                            .chain(4096_u32.to_be_bytes().iter())
                            .chain(4096_u32.to_be_bytes().iter())
                            .copied().collect(),
                
                }).map(|data| opt::Reply::new(
                    opt::NbdOption::Info(None, None).into(), 
                    opt::OptionReplyType::Info, 
                    Some(data)
                ))
    }
}