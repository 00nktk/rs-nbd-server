use std::convert::{TryFrom, TryInto};

use crate::protocol::message::Message;

use num_derive::{FromPrimitive, ToPrimitive};
use num_traits::{FromPrimitive, ToPrimitive};

const REPLY_MAGIC: u64 = 0x3e889045565a9;

#[derive(Debug)]
pub enum NbdOption {
    ExportName,
    Abort,
    List,
    PeekExport,
    Starttls,
    Info(Option<String>, Option<Vec<Info>>),
    Go(Option<String>, Option<Vec<Info>>),
    StructuredReply(bool),  // boot whether to allow or not
    ListMetaContext(Option<String>, Option<Vec<String>>),
    SetMetaContext,
}

#[derive(Debug)]
pub enum OptionError {
    TooShort,
    UnknownOption(u32),
    RequireData(usize),
    Parse,
}

impl TryFrom<&[u8]> for NbdOption {
    type Error = OptionError;

    fn try_from(slice: &[u8]) -> Result<Self, Self::Error> {
        if slice.len() < 8 { Err(Self::Error::TooShort) }
        else {
            let (type_, rest) = slice.split_at(4);
            let (data_len, data) = rest.split_at(4);

            let type_ = type_.try_into()
                .map(u32::from_be_bytes).map_err(|_| OptionError::Parse)?;
             
            let data_len: usize = data_len.try_into()
                .map(u32::from_be_bytes)
                .map_err(|_| OptionError::Parse)? as usize;

            if data.len() < data_len { return Err(Self::Error::RequireData(data_len - data.len())); }
            //  ?: maybe check for data excess (though client should not send options before recieving ack)

            Ok( match type_ {
                // ExportName
                1 => NbdOption::ExportName,
                // Abort
                2 => NbdOption::Abort,
                // List
                3 => NbdOption::List,
                // PeekExport
                4 => NbdOption::PeekExport,
                // Starttls
                5 => NbdOption::Starttls,
                // Info & Go
                6 | 7 => {
                    let (name, info_requests) = parse_info_go(data)?;
                        if type_ == 7 {
                            NbdOption::Go(name, info_requests)
                        } else {
                            NbdOption::Info(name, info_requests)
                        }
                },
                // StructuredReply
                8 => NbdOption::StructuredReply(data_len == 0),
                // ListMetaContext
                9 => {
                    let (name, queries) = parse_list_meta_context(data)?;
                    NbdOption::ListMetaContext(name, queries)
                }, 
                // SetMetaContext 
                10 => NbdOption::SetMetaContext,

                n => return Err(Self::Error::UnknownOption(n))
            })
        }
    }
}

fn parse_info_go(data: &[u8]) -> Result<(Option<String>, Option<Vec<Info>>), OptionError> {
    let (name_len, data) = data.split_at(4);
    let name_len: usize = name_len.try_into()
        .map(u32::from_be_bytes)
        .map_err(|_| OptionError::Parse)? as usize;

    
    let (name, data) = if name_len > 0 {
        let (name, data) = data.split_at(name_len);
        (Some( String::from_utf8(name.into()).unwrap() ), data)  // !: Fix unwrap
    } else { (None, data) };

    let (n_info_requests, requests) = data.split_at(2);
    let n_info_requests = n_info_requests.try_into()
        .map(u16::from_be_bytes)
        .map_err(|_| OptionError::Parse)? as usize;

    let info_requests: Option<Vec<Info>> = if n_info_requests > 0 {
        Some( 
            requests.chunks(2).filter_map(|s| s.try_into().ok())
                .map(u16::from_be_bytes)
                .filter_map(Info::from_u16)
                .collect() 
        )
    } else { None };

    Ok((name, info_requests))
}

fn parse_list_meta_context(data: &[u8]) -> Result<(Option<String>, Option<Vec<String>>), OptionError> {
    let (name_len, data) = data.split_at(4);
    let name_len: usize = name_len.try_into()
        .map(u32::from_be_bytes)
        .map_err(|_| OptionError::Parse)? as usize;

    
    let (name, data) = if name_len > 0 {
        let (name, data) = data.split_at(name_len);
        (Some( String::from_utf8(name.into()).unwrap() ), data)  // !: Fix unwrap
    } else { (None, data) };

    let (n_queries, queries) = data.split_at(4);
    let n_queries = n_queries.try_into()
        .map(u32::from_be_bytes)
        .map_err(|_| OptionError::Parse)? as usize;

    let (_, queries): (_, Option<Vec<String>>) = if n_queries > 0 {
        (0..n_queries)
            .try_fold((queries, Vec::with_capacity(n_queries)), |(data, mut acc), _| {
                let (len, data) = data.split_at(4);
                let len = len.try_into()
                    .map(u32::from_be_bytes)
                    .map_err(|_| OptionError::Parse)? as usize;
        
                if data.len() < len { return Err(OptionError::RequireData(len - data.len())) }
                
                let (qry, data) = data.split_at(len);
                let qry = String::from_utf8_lossy(&qry).into_owned();
                acc.push(qry);
        
                Ok((data, acc))
            }
        ).map(|(a, b)| (a, Some(b)))?
    } else { (&[], None) };
    Ok((name, queries))
}

impl Into<u32> for NbdOption {
    fn into(self) -> u32 {
        match self {
            NbdOption::ExportName => 1,
            NbdOption::Abort => 2,
            NbdOption::List => 3,
            NbdOption::PeekExport => 4,
            NbdOption::Starttls => 5,
            NbdOption::Info(_,_) => 6,
            NbdOption::Go(_,_) => 7,
            NbdOption::StructuredReply(_) => 8,
            NbdOption::ListMetaContext(_,_) => 9, 
            NbdOption::SetMetaContext => 10,
        }
    }
}

#[derive(FromPrimitive, ToPrimitive, Debug)]
pub enum OptionReplyType {
    Ack = 1,
    Server,
    Info,
    MetaContext,
    ErrUnsup = (1 << 31) + 1,
    ErrPolicy,
    ErrInvalid,
    ErrPlatform,
    ErrTlsReqd,
    ErrUnknown,
    ErrShutdown,
    ErrBlockSizeReqd,
    ErrTooBig,
}

#[derive(Debug)]
pub struct Reply {
    header: Vec<u8>,
    pub option_type: u32,
    pub reply_type: OptionReplyType,
    pub data: Option<Vec<u8>>
}

impl Reply {
    pub fn new(option_type: u32, reply_type: OptionReplyType, data: Option<Vec<u8>>) -> Self {
        let len: u32 = data.as_ref().map_or(0, Vec::len) as u32;
        let header = REPLY_MAGIC.to_be_bytes().iter()
            .chain(option_type.to_u32().unwrap().to_be_bytes().iter())  // !: REMOVE UNWRAP 
            .chain(reply_type.to_u32().unwrap().to_be_bytes().iter())
            .chain(len.to_be_bytes().iter())
            .copied()
            .collect();

        Self { header, option_type, reply_type, data }
    }
}

impl Message for Reply {
    fn get_header(&self) -> &[u8] {
        self.header.as_slice()
    }

    fn get_data(&self) -> Option<&[u8]> {
        self.data.as_ref().map(Vec::as_slice)
    }
}

#[derive(FromPrimitive, ToPrimitive, Debug, PartialEq)]
pub enum Info {
    Export,
    Name,
    Description,
    BlockSize
}