use std::cell::RefCell;
use std::fmt;
use std::path::Path;
use std::rc::Rc;

use agb_save::{SaveSlotManager, StorageInfo, StorageMedium};
use eb_agb_psg_controller::{Sfx, Track};
use serde::{Deserialize, Serialize};

//must match game values
pub const SAVE_MAGIC: [u8; 32] = *b"AGB PSG Tracker - EB - SaveVer 1";
pub const SAVE_SLOTS: usize = 16;
pub const MAX_FILENAME_LEN: usize = 20;
pub const SAV_SIZE: usize = 32 * 1024;

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
pub enum SaveKind {
    Sfx,
    Track,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum SavePayload {
    Sfx(Sfx),
    Track(Track),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Metadata {
    pub kind: SaveKind,
    pub name: Vec<u8>,
    pub instruments: u8,
    pub rows: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SavError {
    ReadOnly,
    OutOfBounds,
}

impl fmt::Display for SavError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SavError::ReadOnly => f.write_str("write attempted on read-only storage"),
            SavError::OutOfBounds => f.write_str("access outside the 32 KiB save"),
        }
    }
}

#[derive(Clone)]
pub struct SavBytes(Rc<RefCell<Vec<u8>>>);

pub struct SavStorage {
    bytes: Rc<RefCell<Vec<u8>>>,
    writable: bool,
}

impl StorageMedium for SavStorage {
    type Error = SavError;

    fn info(&self) -> StorageInfo {
        StorageInfo {
            size: SAV_SIZE,
            erase_size: None,
            write_size: 1.try_into().unwrap(),
        }
    }

    fn read(&mut self, offset: usize, buf: &mut [u8]) -> Result<(), SavError> {
        let bytes = self.bytes.borrow();
        let end = offset + buf.len();
        if end > bytes.len() {
            return Err(SavError::OutOfBounds);
        }
        buf.copy_from_slice(&bytes[offset..end]);
        Ok(())
    }

    fn erase(&mut self, _offset: usize, _len: usize) -> Result<(), SavError> {
        if self.writable {
            Ok(())
        } else {
            Err(SavError::ReadOnly)
        }
    }

    fn write(&mut self, offset: usize, data: &[u8]) -> Result<(), SavError> {
        if !self.writable {
            return Err(SavError::ReadOnly);
        }
        let mut bytes = self.bytes.borrow_mut();
        let end = offset + data.len();
        if end > bytes.len() {
            return Err(SavError::OutOfBounds);
        }
        bytes[offset..end].copy_from_slice(data);
        Ok(())
    }
}

pub type Manager = SaveSlotManager<SavStorage, Metadata>;

fn manager_over(bytes: &SavBytes, writable: bool) -> Result<Manager, String> {
    SaveSlotManager::new(
        SavStorage {
            bytes: bytes.0.clone(),
            writable,
        },
        SAVE_SLOTS,
        SAVE_MAGIC,
    )
    .map_err(|e| match e {
        agb_save::SaveError::Storage(SavError::ReadOnly) => {
            "not an AGB PSG Tracker save (bad or missing save header); refusing to touch it \
             — use `insert --init` to format a new save file"
                .to_string()
        }
        e => format!("failed to open save: {e:?}"),
    })
}

fn load_bytes(path: &Path) -> Result<SavBytes, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    if bytes.len() != SAV_SIZE {
        return Err(format!(
            "{}: expected a raw {SAV_SIZE}-byte SRAM save, got {} bytes",
            path.display(),
            bytes.len()
        ));
    }
    Ok(SavBytes(Rc::new(RefCell::new(bytes))))
}

pub fn open_read(path: &Path) -> Result<Manager, String> {
    manager_over(&load_bytes(path)?, false)
}

pub fn open_write(path: &Path) -> Result<(Manager, SavBytes), String> {
    let bytes = load_bytes(path)?;
    manager_over(&bytes, false)?;
    Ok((manager_over(&bytes, true)?, bytes))
}

pub fn create_new() -> Result<(Manager, SavBytes), String> {
    let bytes = SavBytes(Rc::new(RefCell::new(vec![0u8; SAV_SIZE])));
    Ok((manager_over(&bytes, true)?, bytes))
}

pub fn persist(path: &Path, bytes: &SavBytes) -> Result<(), String> {
    let tmp = path.with_extension("sav.tmp");
    std::fs::write(&tmp, &*bytes.0.borrow()).map_err(|e| format!("{}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, path).map_err(|e| format!("{}: {e}", path.display()))
}
