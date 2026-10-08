use kira::sound::FromFileError;
use std::io::Read;
use symphonia::core::formats::{FormatOptions, FormatReader, probe::Hint};
use symphonia::core::io::{MediaSourceStream, MediaSourceStreamOptions};
use symphonia::core::meta::MetadataOptions;

use crate::files::GameFiles;

/// Archive entries retain source names; choose the audio path by container bytes.
pub fn is_webm(files: &GameFiles, path: &str) -> Result<bool, FromFileError> {
    let mut header = [0; 4];
    files.open_file(path)?.read_exact(&mut header)?;
    Ok(header == [0x1a, 0x45, 0xdf, 0xa3])
}

pub fn open(files: &GameFiles, path: &str) -> Result<Box<dyn FormatReader>, FromFileError> {
    let source = MediaSourceStream::new(
        Box::new(files.open_file(path)?),
        MediaSourceStreamOptions::default(),
    );
    Ok(symphonia::default::get_probe().probe(
        &Hint::default(),
        source,
        FormatOptions::default(),
        MetadataOptions::default(),
    )?)
}

pub fn duration(reader: &dyn FormatReader) -> Option<f64> {
    let info = reader.media_info();
    Some(info.time_base?.calc_duration(info.duration?)?.as_secs_f64())
}
