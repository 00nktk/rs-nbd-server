use std::io::{Read};
use std::rc::Rc;
use std::iter::Iterator;

use crate::protocol::message::Message;
use crate::export::Export;

use num_derive::{FromPrimitive, ToPrimitive};
use num_traits::{ToPrimitive};

pub const SIMPLE_REPLY_MAGIC: u32 = 0x67446698;

pub enum Reply {
    Simple(SimpleReply),
    Structured(StructuredReply),
    Disconnect
}

#[derive(Debug)]
pub struct SimpleReply {
    header: Vec<u8>,
    pub error: u32,
    pub handle: u64,
    pub data: Option<Vec<u8>>
}

impl SimpleReply {
    pub fn new(error: u32, handle: u64, data: Option<Vec<u8>>) -> Self {
        let header = SIMPLE_REPLY_MAGIC.to_be_bytes().iter()
            .chain(error.to_be_bytes().iter())
            .chain(handle.to_be_bytes().iter())
            .copied()
            .collect();
        
        Self { header, error, handle, data }
    }
}

impl Message for SimpleReply {
    fn get_header(&self) -> &[u8] {
        self.header.as_slice()
    }

    fn get_data(&self) -> Option<&[u8]> {
        self.data.as_ref().map(Vec::as_slice)
    }
}

pub struct StructuredReply{
    handle: u64,
    count: usize,
    gen_func: Box<dyn FnMut((u64, u64)) -> Vec<u8>>,
    ranges: Vec<(u64, u64)>
}

impl StructuredReply {
    pub fn read_from_offset(source: Rc<Export>, handle: u64, offset: u64, len: u64, chunk_size: u64) -> Self
    {
        let end = offset + len;

        let ranges: Vec<(u64, u64)> = (0..).take_while(|i| i * chunk_size < len)
            .map(|i| {
                let start = offset + i * chunk_size;
                let size = if start + chunk_size > end {
                        end - offset 
                    } else { chunk_size };
                (start, size)
            }).collect();

        let gen_func = Box::new(
            move |arg: (u64, u64)| {
                let (ofs, len) = arg;
                let mut data = vec![0; len as usize + 8];
                (&ofs.to_be_bytes()[0..]).read(&mut data);
                
                source.read_into(&mut data[8..], ofs, len as usize).unwrap();

                data
            }
        );

        Self { handle, count: 0, gen_func, ranges }
    } 
}

impl Iterator for StructuredReply {
    type Item = StructuredReplyChunk;

    fn next(&mut self) -> Option<Self::Item> {
        if self.count < self.ranges.len() {
            let data = (self.gen_func)(self.ranges[self.count]);
            self.count += 1;
            Some( Self::Item::new(
                ChunkType::OffsetData,
                self.handle,
                Some(data),
                self.count == self.ranges.len() - 1             
            ))
        } else {
            None
        }
    }
}

const STRUCTURED_REPLY_MAGIC: u32 = 0x668e33ef;

#[derive(FromPrimitive, ToPrimitive, Debug)]
pub enum ChunkType {
    None = 0,
    OffsetData,
    OffsetHole,
    BlockStatus = 5,
    Error = (1 << 15) + 1,
    ErrorOffset,
}

#[derive(Debug)]
pub struct StructuredReplyChunk {
    header: Vec<u8>,
    pub flags: u16,            //  !: only NBD_REPLY_FLAG_DONE on bit 0
    pub type_: ChunkType,      //  u16
    pub handle: u64,
    pub len: u64,
    data: Option<Vec<u8>>  //  ?: consider using unwrapped Vec since 
                           //  ?: simple reply can be used for msgs without payload
}

impl StructuredReplyChunk {
    fn new(type_: ChunkType, handle: u64, data: Option<Vec<u8>>, done: bool) -> Self {
        let flags: u16 = if done { 1 } else { 0 };
        let len: u64 = if let Some(ref vec) = data { vec.len() as u64 } else { 0 };

        let header = STRUCTURED_REPLY_MAGIC.to_be_bytes().iter()
            .chain(flags.to_be_bytes().iter())
            .chain(type_.to_u16().unwrap().to_be_bytes().iter())
            .chain(handle.to_be_bytes().iter())
            .chain(len.to_be_bytes().iter())
            .copied().collect();

        Self { header, flags, type_, handle, len, data }
    }
}

impl Message for StructuredReplyChunk {
    fn get_header(&self) -> &[u8] {
        self.header.as_slice()
    }

    fn get_data(&self) -> Option<&[u8]> {
        self.data.as_ref().map(Vec::as_slice)
    }
}
