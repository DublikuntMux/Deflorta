//! Shared media probing for `WebM` video and soundtracks.

use std::fs::File;
use std::path::Path;

use kira::sound::FromFileError;
use symphonia::core::formats::{FormatOptions, FormatReader, probe::Hint};
use symphonia::core::io::{MediaSourceStream, MediaSourceStreamOptions};
use symphonia::core::meta::MetadataOptions;

pub fn open(path: &Path) -> Result<Box<dyn FormatReader>, FromFileError> {
    let source = MediaSourceStream::new(
        Box::new(File::open(path)?),
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
