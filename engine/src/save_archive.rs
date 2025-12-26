//! # SaveArchive Module
//!
//! This module provides a custom archive format for storing a game's state (`GameData`)
//! together with associated turn images in a single file. The format is designed for
//! efficient appending of images while keeping the game data easily readable and writable.
//!
//! ## File Layout
//!
//! The archive file is structured as follows:
//!
//! ```text
//! +----------------------+
//! | Header               |  fixed-size `SaveHeader`
//! +----------------------+
//! | GameData JSON region |  Fixed-size (or growable) space for JSON-serialized `GameData`
//! +----------------------+
//! | Image Data Chunks    |  Arbitrary-length sequence of image bytes, appended as needed
//! +----------------------+
//! | Image Index          |  Serialized `BTreeMap<ImageId, (offset, length)>` pointing to image chunks
//! +----------------------+
//! ```
//!
//! ## Key Features
//! - The JSON region for `GameData` is pre-allocated and can grow if necessary by rewriting the file.
//! - Each appended image is stored sequentially in the file. The index at the end allows random access to any image by its `ImageId`.
//! - The header is updated whenever the JSON region or index changes, keeping the archive consistent.
//! - Supports reading and writing of both `GameData` and images via `read_game_data`, `write_game_data`, `append_image`, and `read_image`.

use color_eyre::{
    Result,
    eyre::{ensure, eyre},
};
use log::debug;
use serde_binary::binary_stream::Endian;
use std::{
    fs::{File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    mem::transmute,
    path::Path,
};

use crate::game::GameData;

const MAGIC: &[u8; 8] = b"WOWEAVER";

#[derive(Debug)]
pub struct SaveArchive {
    file: File,
    header: SaveHeader,
    /// (offset, length)
    image_index: Vec<(u64, u64)>,
}

#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct SaveHeader {
    /// marker for files
    magic: [u8; 8],
    /// 8bytes so there is no padding
    version: u64,
    game_data_region_size: u64,
    game_data_size: u64,
    game_data_region_offset: u64,
    index_offset: u64,
    index_size: u64,
}

impl SaveArchive {
    pub const DEFAULT_GAME_DATA_SIZE: u64 = 20 * 1024 * 1024; // 20 MB
    pub const HEADER_SIZE: u64 = size_of::<SaveHeader>() as u64;

    pub fn create<P: AsRef<Path>>(path: P) -> Result<Self> {
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(&path)?;

        let header = SaveHeader {
            magic: *MAGIC,
            version: 1,
            game_data_region_size: Self::DEFAULT_GAME_DATA_SIZE,
            game_data_region_offset: Self::HEADER_SIZE,
            index_offset: Self::HEADER_SIZE + Self::DEFAULT_GAME_DATA_SIZE,
            index_size: 0,
            game_data_size: 0,
        };

        file.set_len(header.index_offset)?;
        write_header(&mut file, &header)?;

        Ok(Self {
            file,
            header,
            image_index: vec![],
        })
    }

    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let mut file = OpenOptions::new().read(true).write(true).open(&path)?;
        let header = read_header(&mut file)?;
        debug!("Read header:\n{header:#?}");
        ensure!(&header.magic == MAGIC, "Invalid save file");

        let mut index_bytes = vec![0u8; header.index_size as usize];
        file.seek(SeekFrom::Start(header.index_offset))?;
        file.read_exact(&mut index_bytes)?;
        let image_index: Vec<(u64, u64)> = if header.index_size > 0 {
            serde_binary::from_slice(&index_bytes, Endian::Little)?
        } else {
            vec![]
        };

        Ok(Self {
            file,
            header,
            image_index,
        })
    }

    pub fn write_game_data(&mut self, data: &GameData) -> Result<()> {
        let serde_str = serde_json::to_string(data)?;
        let json_bytes = serde_str.as_bytes();
        ensure!(
            (json_bytes.len() as u64) < self.header.game_data_region_size,
            "The json region in the save archive is not large enough, it needs to be grown"
        );

        self.file
            .seek(SeekFrom::Start(self.header.game_data_region_offset))?;
        self.file.write_all(&json_bytes)?;

        self.header.game_data_size = json_bytes.len() as u64;
        write_header(&mut self.file, &self.header)?;

        Ok(())
    }

    pub fn append_image(&mut self, image_bytes: &[u8]) -> Result<()> {
        let offset = self.header.index_offset;
        let length = image_bytes.len() as u64;
        self.file.set_len(offset)?;
        self.file.seek(SeekFrom::End(0))?;
        self.file.write_all(image_bytes)?;

        self.image_index.push((offset, length));
        self.header.index_offset += length;
        let serialized_index = serde_binary::to_vec(&self.image_index, Endian::Little)?;
        self.file.write_all(&serialized_index)?;
        self.header.index_size = serialized_index.len() as u64;
        write_header(&mut self.file, &self.header)?;

        Ok(())
    }

    pub fn read_game_data(&mut self) -> Result<GameData> {
        ensure!(self.header.game_data_size > 0, "No game data");
        self.file
            .seek(SeekFrom::Start(self.header.game_data_region_offset))?;
        let mut buf = vec![0u8; self.header.game_data_size as usize];
        self.file.read_exact(&mut buf)?;

        let data: GameData = serde_json::from_str(std::str::from_utf8(&buf)?)?;
        Ok(data)
    }

    pub fn read_image(&mut self, id: usize) -> Result<Vec<u8>> {
        let (offset, length) = self
            .image_index
            .get(id)
            .ok_or_else(|| eyre!("Image ID not found"))?;

        self.file.seek(SeekFrom::Start(*offset))?;
        let mut buf = vec![0u8; *length as usize];
        self.file.read_exact(&mut buf)?;
        Ok(buf)
    }
}

fn read_header(file: &mut File) -> Result<SaveHeader> {
    let mut res = SaveHeader::default();
    let buf: &mut [u8; size_of::<SaveHeader>()] = unsafe { transmute(&mut res) };
    file.read_exact(buf)?;
    Ok(res)
}

fn write_header(file: &mut File, header: &SaveHeader) -> Result<()> {
    let buf: &[u8; size_of::<SaveHeader>()] = unsafe { transmute(header) };
    file.seek(SeekFrom::Start(0))?;
    file.write_all(buf)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use tempfile::NamedTempFile;

    fn make_sample_game_data(turns: usize) -> GameData {
        let mut pc_descriptions = BTreeMap::new();
        pc_descriptions.insert("Alice".to_string(), "A brave warrior".to_string());

        let world_description = crate::game::WorldDescription {
            main_description: "A fantasy world with dragons".to_string(),
            pc_descriptions,
            init_action: "Look around".to_string(),
        };

        let mut summaries = vec![];
        for i in 0..(turns / 8) {
            summaries.push(crate::game::Summary {
                content: format!("Summary at turn {}", i * 10),
                age: i * 10,
            });
        }

        let mut turn_data = vec![];
        for i in 0..turns {
            let input = crate::game::TurnInput::PlayerAction(format!("Do action {}", i));
            let output = crate::game::TurnOutput {
                text: format!("Result of action {}", i),
                secret_info: format!("Secret info {}", i),
                proposed_next_actions: [
                    format!("Action A{}", i),
                    format!("Action B{}", i),
                    format!("Action C{}", i),
                ],
                input_tokens: 5,
                output_tokens: 10,
            };
            turn_data.push(crate::game::TurnData {
                summary_before_input: if i == 0 { None } else { Some(i - 1) },
                input,
                output,
            });
        }

        GameData {
            world_description,
            pc: "Alice".to_string(),
            summaries,
            turn_data,
        }
    }

    #[test]
    fn create_and_write_game_data() -> Result<()> {
        let tmpfile = NamedTempFile::new()?;
        let mut archive = SaveArchive::create(tmpfile.path())?;

        let game_data = make_sample_game_data(15);
        archive.write_game_data(&game_data)?;

        let read_data = archive.read_game_data()?;
        assert_eq!(read_data.pc, "Alice");
        assert_eq!(read_data.turn_data.len(), 15);
        assert_eq!(read_data.summaries.len(), 1);
        Ok(())
    }

    #[test]
    fn append_and_read_image() -> Result<()> {
        let tmpfile = NamedTempFile::new()?;
        let mut archive = SaveArchive::create(tmpfile.path())?;

        let img1 = vec![0u8, 1, 2, 3, 4, 5];
        let img2 = vec![10u8, 11, 12];

        archive.append_image(&img1)?;
        archive.append_image(&img2)?;

        let read1 = archive.read_image(0)?;
        let read2 = archive.read_image(1)?;

        assert_eq!(img1, read1);
        assert_eq!(img2, read2);

        Ok(())
    }

    #[test]
    fn reopen_archive() -> Result<()> {
        let tmpfile = NamedTempFile::new()?;
        let path = tmpfile.path().to_path_buf();

        {
            let mut archive = SaveArchive::create(&path)?;
            let game_data = make_sample_game_data(5);
            archive.write_game_data(&game_data)?;
        }

        {
            let mut archive = SaveArchive::open(&path)?;
            let game_data = archive.read_game_data()?;
            assert_eq!(game_data.turn_data.len(), 5);
            assert_eq!(game_data.pc, "Alice");

            let img = vec![42u8, 43, 44];
            archive.append_image(&img)?;
        }

        let mut archive = SaveArchive::open(&path)?;
        let img = archive.read_image(0)?;
        assert_eq!(img, vec![42u8, 43, 44]);

        Ok(())
    }

    #[test]
    fn image_not_found() -> Result<()> {
        let tmpfile = NamedTempFile::new()?;
        let mut archive = SaveArchive::create(tmpfile.path())?;

        let err = archive.read_image(999).unwrap_err();
        assert!(err.to_string().contains("Image ID not found"));

        Ok(())
    }
}
