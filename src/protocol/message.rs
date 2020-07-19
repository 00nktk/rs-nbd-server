use std::io;
use std::io::{Write};
use std::fmt::Debug;

pub trait Message: Debug {
    fn get_header(&self) -> &[u8];
    fn get_data(&self) -> Option<&[u8]>;
}

pub fn send_msg<W, M>(stream: &mut W, msg: M) -> io::Result<()>
    where W: Write,
          M: Message
{
    stream.write_all(msg.get_header())?;
    msg.get_data().map(|buf| stream.write_all(buf).unwrap());
    stream.flush()
}