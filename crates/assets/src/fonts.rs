use crate::GameFiles;

#[must_use]
pub fn load_fonts(files: &GameFiles) -> fontdb::Database {
    let mut db = fontdb::Database::new();
    for path in files.list("fonts") {
        let is_font = std::path::Path::new(&path)
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| {
                ["ttf", "otf", "ttc", "otc"]
                    .iter()
                    .any(|font| ext.eq_ignore_ascii_case(font))
            });
        if is_font {
            match files.read(&path) {
                Ok(data) => db.load_font_data(data),
                Err(err) => log::warn!("Cannot read font '{path}': {err}"),
            }
        }
    }
    db
}

#[must_use]
pub fn font_families(files: &GameFiles) -> Vec<String> {
    let db = load_fonts(files);
    let mut families: Vec<String> = db
        .faces()
        .filter_map(|face| face.families.first().map(|(name, _)| name.clone()))
        .collect();
    families.sort_unstable();
    families.dedup();
    families
}
