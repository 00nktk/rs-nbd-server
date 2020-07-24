use std::fs::{File, metadata};
use std::os::unix::fs::*;
use std::io::{SeekFrom, Seek, Read};

pub struct Export {
    pub name: String,
    file: Option<File>,
    pub size: u64,
}

impl Export {
    pub fn new(filename: String) -> std::io::Result<Self> {
        let mtdt = metadata(&filename)?;
        let size = if mtdt.file_type().is_block_device() {
            File::open(&filename)?.seek(SeekFrom::End(0))?  // kinda hacky, but works
        } else { 
            mtdt.len() 
        };

        if size == 0 {
            panic!("size of the export is 0");
        }

        Ok( Self { name: filename, file: None, size } )
    }

    pub fn load(&mut self) -> std::io::Result<()> {
        if !self.loaded() {
            self.file = Some(File::open(&self.name)?);
        }
        Ok(())
    }

    pub fn loaded(&self) -> bool {
        self.file.is_some()
    }

    pub fn read(&self, offset: u64, len: usize) -> std::io::Result<Vec<u8>> {
        let mut buf = vec![0u8; len];  // !FIXME: can panic 
        self.read_into(&mut buf, offset, len)?;
        Ok(buf)
    }
    
    pub fn read_into(&self, buf: &mut [u8], offset: u64, len: usize) -> std::io::Result<()> {        
        if !self.loaded() { panic!("export not loaded"); }

        // self.file.as_ref().unwrap().seek(SeekFrom::Start(offset))?;
        // let _ = self.file.as_ref().unwrap().read(&mut buf[..len])?;
        let _ = self.file.as_ref().unwrap().read_at(&mut buf[..len], offset)?;

        Ok(())
    }
}
