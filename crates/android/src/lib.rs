//! `GameActivity` entry point. Kotlin loads this library before creating the
//! activity so the tts crate's `JNI_OnLoad` can resolve `rs.tts.Bridge`.

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
extern "Rust" fn android_main(app: winit::platform::android::activity::AndroidApp) {
    android_logger::init_once(
        android_logger::Config::default()
            .with_tag("Deflorta")
            .with_max_level(log::LevelFilter::Info),
    );
    if let Err(error) = start(app) {
        log::error!("Cannot start the game: {error:#}");
    }
}

#[cfg(target_os = "android")]
fn start(app: winit::platform::android::activity::AndroidApp) -> anyhow::Result<()> {
    use anyhow::Context;
    use std::io::Write;

    let directory = app.internal_data_path().context("no app data directory")?;
    std::fs::create_dir_all(&directory)?;
    let mut asset = app
        .asset_manager()
        .open(c"game.dm")
        .context("missing game.dm asset")?;
    // Copy in bounded chunks to retain the archive's seekable filesystem I/O.
    // Always replace it so installing an update cannot reuse old game data.
    let temporary = directory.join("game.dm.partial");
    let archive = directory.join("game.dm");
    {
        let mut file = std::io::BufWriter::new(std::fs::File::create(&temporary)?);
        std::io::copy(&mut asset, &mut file)?;
        file.flush()?;
    }
    std::fs::rename(temporary, &archive)?;
    deflorta::run_android(deflorta::GameFiles::open(&archive)?, app)
}
