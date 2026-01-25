use std::{
    fs::{self, File},
    io::{BufReader, BufWriter},
    path::{Path, PathBuf},
};

use directories::ProjectDirs;
use serde::{Deserialize, Serialize};

use crate::synth_engine::Config;

const PRESET_EXT: &str = "adp";

#[derive(Serialize, Deserialize)]
pub struct PresetInfo {
    pub title: String,
}

#[derive(Serialize, Deserialize)]
pub struct PresetListItem {
    #[serde(flatten)]
    pub info: PresetInfo,
    #[serde(skip)]
    pub path: String,
}

#[derive(Serialize, Deserialize)]
pub struct Preset {
    #[serde(flatten)]
    pub info: PresetInfo,
    pub config: Config,
}

pub struct Presets {
    dirs: ProjectDirs,
}

impl Presets {
    pub fn new() -> Option<Self> {
        ProjectDirs::from("com", "Additizer", "Additizer").map(|dirs| Self { dirs })
    }

    fn read_preset_list_item(path: &Path) -> Option<PresetListItem> {
        let file = File::open(path).ok()?;
        let reader = BufReader::new(file);
        let mut item: PresetListItem = serde_json::from_reader(reader).ok()?;

        item.path = path.to_str()?.to_string();
        Some(item)
    }

    pub fn read_presets_list(&self) -> Vec<PresetListItem> {
        let presets_dir = self.dirs.data_dir();

        let Ok(entries) = presets_dir.read_dir() else {
            return Vec::new();
        };

        let mut list: Vec<PresetListItem> = entries
            .filter_map(|entry| entry.ok())
            .filter(|entry| {
                entry
                    .path()
                    .extension()
                    .map(|ext| ext == PRESET_EXT)
                    .unwrap_or(false)
            })
            .filter_map(|entry| Self::read_preset_list_item(entry.path().as_path()))
            .collect();

        list.sort_by_key(|item| item.info.title.to_lowercase());

        list
    }

    pub fn read_preset(path: &str) -> Option<Preset> {
        let file = File::open(path).ok()?;
        let reader = BufReader::new(file);

        serde_json::from_reader(reader).ok()?
    }

    pub fn write_preset(&self, preset: &Preset) -> Option<()> {
        let mut path = PathBuf::from(self.dirs.data_dir());

        fs::create_dir_all(path.as_path()).ok()?;
        path.push(&preset.info.title);
        path.set_extension(PRESET_EXT);

        let file = File::create(path).ok()?;
        let writer = BufWriter::new(file);

        serde_json::to_writer(writer, preset).ok()?;
        Some(())
    }
}
