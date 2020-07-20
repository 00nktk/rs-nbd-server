use std::io::{Read};
use std::rc::Rc;
use std::iter::Iterator;

use crate::protocol::message::Message;
use crate::export::Export;

use num_derive::{FromPrimitive, ToPrimitive};
use num_traits::{ToPrimitive};

pub const SIMPLE_REPLY_MAGIC: u32 = 0x67446698;
const STRUCTURED_REPLY_MAGIC: u32 = 0x668e33ef;

pub enum Reply {
    Simple(SimpleReply),
    Structured(StructuredReply),
    Disconnect
}

#[derive(Debug)]
pub struct SimpleReply {
    header: [u8; 16],
    pub error: u32,
    pub handle: u64,
    pub data: Option<Vec<u8>>
}

impl SimpleReply {
    pub fn new(error: u32, handle: u64, data: Option<Vec<u8>>) -> Self {
        let mut header = [0_u8; 16];
        
        SIMPLE_REPLY_MAGIC.to_be_bytes().iter().enumerate()
            .for_each(|(i, b)| header[i] = *b);
        error.to_be_bytes().iter().enumerate()
            .for_each(|(i, b)| header[i + 4] = *b);
        handle.to_be_bytes().iter().enumerate()
            .for_each(|(i, b)| header[i + 8] = *b);
        
        Self { header, error, handle, data }
    }
}

impl Message for SimpleReply {
    fn get_header(&self) -> &[u8] {
        &self.header
    }

    fn get_data(&self) -> Option<&[u8]> {
        self.data.as_ref().map(Vec::as_slice)
    }
}

pub struct StructuredReply{
    handle: u64,
    count: usize,
    gen_func: Box<dyn FnMut((u64, u32)) -> Vec<u8>>,
    ranges: Vec<(u64, u32)>
}

impl StructuredReply {
    pub fn read_from_offset(source: Rc<Export>, handle: u64, offset: u64, len: u32, chunk_size: u32) -> Self
    {
        let end: u64 = offset + len as u64;

        let ranges: Vec<(u64, u32)> = (0..).take_while(|i| i * chunk_size < len)
            .map(|i| {
                let start: u64 = offset + (i * chunk_size) as u64;
                let size: u64 = if start + chunk_size as u64 > end {
                        end - offset 
                    } else { chunk_size.into() };
                (start, size as u32)
            }).collect();

        let gen_func = Box::new(
            move |arg: (u64, u32)| {
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
                self.count == self.ranges.len()             
            ))
        } else {
            None
        }
    }
}

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
    header: [u8; 20],
    pub flags: u16,            //  !: only NBD_REPLY_FLAG_DONE on bit 0
    pub type_: ChunkType,      //  u16
    pub handle: u64,
    pub len: u32,
    data: Option<Vec<u8>>  //  ?: consider using unwrapped Vec since 
                           //  ?: simple reply can be used for msgs without payload
}

impl StructuredReplyChunk {
    fn new(type_: ChunkType, handle: u64, data: Option<Vec<u8>>, done: bool) -> Self {
        let flags: u16 = if done { 1 } else { 0 };
        let len: u32 = data.as_ref().map(Vec::len).unwrap_or(0) as u32;

        let mut header = [0_u8; 20];

        STRUCTURED_REPLY_MAGIC.to_be_bytes().iter().enumerate()
            .for_each(|(i, b)| header[i] = *b);
        flags.to_be_bytes().iter().enumerate()
            .for_each(|(i, b)| header[i + 4] = *b);
        type_.to_u16().unwrap().to_be_bytes().iter().enumerate()
            .for_each(|(i, b)| header[i + 6] = *b);
        handle.to_be_bytes().iter().enumerate()
            .for_each(|(i, b)| header[i + 8] = *b);
        len.to_be_bytes().iter().enumerate()
            .for_each(|(i, b)| header[i + 16] = *b);

        Self { header, flags, type_, handle, len, data }
    }
}

impl Message for StructuredReplyChunk {
    fn get_header(&self) -> &[u8] {
        &self.header
    }

    fn get_data(&self) -> Option<&[u8]> {
        self.data.as_ref().map(Vec::as_slice)
    }
}
