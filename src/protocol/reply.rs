use crate::protocol::message::Message;

pub const SIMPLE_REPLY_MAGIC: u32 = 0x67446698;

pub enum Reply {
    Simple(SimpleReply),
    Structured(StructuredReply),
    Disconnect
}

impl Message for Reply {
    fn get_header(&self) -> &[u8] {
        match self {
            Reply::Simple(x) => x.get_header(),
            Reply::Structured(x) => x.get_header(),
            Reply::Disconnect => &[]
        }
    }

    fn get_data(&self) -> Option<&[u8]> {
        match self {
            Reply::Simple(x) => x.get_data(),
            Reply::Structured(x) => x.get_data(),
            Reply::Disconnect => None
        }
    }
}

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

}

impl Message for StructuredReply {
    fn get_header(&self) -> &[u8] {
        unimplemented!()
    }

    fn get_data(&self) -> Option<&[u8]> {
        unimplemented!()
    }
}
