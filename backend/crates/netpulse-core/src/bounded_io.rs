//! Bound allocations before reading local metadata and recovery records.
use crate::{Error, Result};
use std::{io::Read, path::Path};

pub(crate) fn read(path: &Path, limit: u64, oversized: Error) -> Result<Vec<u8>> {
    let file=std::fs::File::open(path).map_err(|e|Error::io(&oversized.stage,e))?;
    if file.metadata().map_err(|e|Error::io(&oversized.stage,e))?.len()>limit {
        return Err(oversized);
    }
    // The file can grow after metadata was read. Never let read_to_end allocate
    // for the complete changed file before checking the limit.
    let mut bytes=Vec::new();
    file.take(limit.saturating_add(1)).read_to_end(&mut bytes)
        .map_err(|e|Error::io(&oversized.stage,e))?;
    if bytes.len() as u64>limit {return Err(oversized)}
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_limit_is_read_but_oversized_and_missing_files_are_distinguished() {
        let dir=tempfile::tempdir().unwrap();let path=dir.path().join("record.json");
        let limit=||Error::new("RecordTooLarge","record-read","Record exceeds limit");
        std::fs::write(&path,b"1234").unwrap();
        assert_eq!(read(&path,4,limit()).unwrap(),b"1234");
        assert_eq!(read(&path,3,limit()).unwrap_err().code,"RecordTooLarge");
        assert_eq!(read(&dir.path().join("missing"),4,limit()).unwrap_err().code,"IoError");
    }
}
