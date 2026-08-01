use crate::UserStr;
use agb::eprintln;
use agb::save::{SaveSlotManager, Slot};
use eb_agb_psg_controller::{Sfx, Track};
use serde::{Deserialize, Serialize};

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
    pub name: UserStr,
    pub instruments: u8,
    pub rows: u8,
}

pub struct SaveController {
    manager: SaveSlotManager<Metadata>,
}

impl SaveController {
    pub fn new(manager: SaveSlotManager<Metadata>) -> Self {
        Self { manager }
    }

    pub fn slot_count(&self) -> usize {
        self.manager.num_slots()
    }

    pub fn slot(&self, slot: usize) -> Slot<'_, Metadata> {
        self.manager.slot(slot)
    }

    /// returns true when the effect was written
    pub fn save_sfx(&mut self, slot: usize, name: &UserStr, sfx: &Sfx) -> bool {
        let metadata = Metadata {
            kind: SaveKind::Sfx,
            name: name.clone(),
            instruments: sfx.instruments.len().min(255) as u8,
            rows: sfx.rows.len().min(255) as u8,
        };
        if let Err(e) = self
            .manager
            .write(slot, &SavePayload::Sfx(sfx.clone()), &metadata)
        {
            eprintln!("Error writing save: {e:?}");
            return false;
        }
        true
    }

    pub fn load(&mut self, slot: usize) -> Option<(UserStr, SavePayload)> {
        let name = self.manager.metadata(slot)?.name.clone();
        match self.manager.read::<SavePayload>(slot) {
            Ok(payload) => Some((name, payload)),
            Err(e) => {
                eprintln!("Error reading save: {e:?}");
                None
            }
        }
    }

    pub fn delete_save(&mut self, slot: usize) -> bool {
        if slot >= self.manager.num_slots() {
            return false;
        }
        if let Err(e) = self.manager.erase(slot) {
            eprintln!("Error deleting saving: {e:?}");
            return false;
        }
        true
    }
}
