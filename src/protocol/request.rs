use std::convert::{TryInto, TryFrom};

use num_derive::{FromPrimitive, ToPrimitive};
use num_traits::{FromPrimitive};

pub const REQMAGIC: u32 = 0x25609513;

#[derive(Debug)]
pub struct Flags {
    fua: bool,
    no_hole: bool,
    dont_fragment: bool,
    request_one: bool,
    fast_zero: bool,
}

impl From<u16> for Flags {
    fn from(flags: u16) -> Self {
        Self {
            fua:           (flags >> 0 & 1) != 0,
            no_hole:       (flags >> 1 & 1) != 0,
            dont_fragment: (flags >> 2 & 1) != 0,
            request_one:   (flags >> 3 & 1) != 0,
            fast_zero:     (flags >> 4 & 1) != 0,
        }
    }
}

#[derive(FromPrimitive, ToPrimitive, Debug)]
pub enum RequestType {
    Read = 0,
    Write,
    Disc,
    Flush,
    Trim,
    Cache,
    WriteZeroes,
    BlockStatus,
    Resize,
}

#[derive(Debug)]
pub struct Request {
    pub type_: RequestType,
    pub flags: Flags,
    pub handle: u64,
    pub offset: u64,
    pub len: u32,
    pub data: Option<Vec<u8>>,
}

// TODO: come up with better names
#[derive(Debug)]
pub enum RequestError {
    UnknownType,
    BufferTooShort,
    Parse,
}

impl From<std::array::TryFromSliceError> for RequestError {
    // ?: consider wrapping base error 
    fn from(_: std::array::TryFromSliceError) -> RequestError {
        RequestError::Parse
    }
}


impl TryFrom<&[u8]> for Request {
    type Error = RequestError;

    fn try_from(slice: &[u8]) -> Result<Self, Self::Error> {
        
        if slice.len() < 24 {
            Err(Self::Error::BufferTooShort)
        } else {
            println!("{:?}", slice);
            let type_ = slice[2..4].try_into()
                .map(u16::from_be_bytes)
                .map(RequestType::from_u16)?
                .ok_or(Self::Error::UnknownType)?; 

            let flags =  slice[0..2].try_into()
                .map(u16::from_be_bytes)?.into(); 

            let handle = slice[4..12].try_into().map(u64::from_be_bytes)?;
            let offset = slice[12..20].try_into().map(u64::from_be_bytes)?; 
            let len =    slice[20..24].try_into().map(u32::from_be_bytes)?;

            let data = if slice.len() > 24 { Some(slice[24..].into()) } else { None };

            Ok( Self { type_, flags, handle, offset, len, data })
        }
    }
}