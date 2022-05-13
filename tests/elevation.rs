use dotenv;
use srtmtiles::Tiles;

/// Validates the elevetion retrieval.
///
/// Needs integration with files N49W001.hgt & N49W002.hgt.
///
/// This tests loads .env file and reads the `HGT_TILES_FOLDER`
/// to get the tiles directory in which N49W001.hgt & N49W002.hgt
/// should be provided.
#[test]
fn it_gets_the_right_elevation_from_file() {
    let key = "HGT_TILES_FOLDER";
    let folder = dotenv::var(key).unwrap();

    let positions = vec![
        (49.99972, -0.99972224, 0),
        (49.444443, -0.99972224, 0),
        (49.02778, -0.99972224, 118),
        (49.99972, -0.027777791, 0),
        (49.444443, -0.027777791, 0),
        (49.0302, -1.1916, 151),
        (49.28799, -1.47253, 122),
    ];

    positions
        .into_iter()
        .for_each(|(lat, lng, expect): (f32, f32, i16)| {
            let tiles = Tiles::new(folder.clone());
            let h = tiles.elevation(lat, lng).unwrap();
            assert!(expect == h, "Failed for lat:{}, lng:{})", lat, lng);
        });
}
