use anyhow::Result;
use byteorder::{self, BigEndian, ReadBytesExt};
use std::{
    collections::{HashMap, HashSet},
    fs::File,
    io::{Seek, SeekFrom},
    path::{Path, PathBuf},
};
use thiserror::Error;

///
/// # SRTM files reader.
///
/// This library provides simple function to access SRTM files elevation.
/// Its main purpose is the get the elevation directly from latitude and longitude values.
///
/// > - See SRTM [description](https://www.usgs.gov/centers/eros/science/usgs-eros-archive-digital-elevation-shuttle-radar-topography-mission-srtm-non)
/// > - See the [tiles downloader](http://dwtkns.com/srtm30m/)
/// > - See the account [login page](https://urs.earthdata.nasa.gov/) for data access
///
/// * General information *
/// Projection: Geographic
/// Horizontal Datum: WGS84
/// Vertical Datum: EGM96 (Earth Gravitational Model 1996)
/// Vertical Units: Meters
/// SRTM1: Spatial Resolution: 1 arc-second for global coverage (~30 meters)
/// SRTM3: Spatial Resolution: 3 arc-seconds for global coverage (~90 meters)
/// Raster Size:  1 degree tiles
/// C-band Wavelength: 5.6 cm
///

fn main() {}

#[derive(Error, Debug)]
pub enum SrtmError {
    #[error("File size is no STRM tile compatible format")]
    FileSize,
}

const SRTM1_FSIZE: u64 = 3601 * 3601 * 2;
const SRTM3_FSIZE: u64 = 1201 * 1201 * 2;

#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub enum Resolution {
    SRTM1,
    SRTM3,
}

impl TryFrom<u64> for Resolution {
    type Error = SrtmError;

    fn try_from(filesize: u64) -> Result<Self, Self::Error> {
        match filesize {
            SRTM1_FSIZE => Ok(Resolution::SRTM1),
            SRTM3_FSIZE => Ok(Resolution::SRTM3),
            _ => Err(Self::Error::FileSize),
        }
    }
}

/// SRTM files are squares.
/// The side size depends on the subformat:
/// - 3601 values for one earth arc degree (and an overlapped value) with SRTM1
/// - 1201 values for one earth arc degree (and an overlapped value) with SRTM3
impl Resolution {
    /// get the side size of this format.
    fn side(&self) -> u32 {
        match &self {
            Resolution::SRTM1 => 3601,
            Resolution::SRTM3 => 1201,
        }
    }
}

/// generate srtm file name containing elevation for the given geoposition
fn srtm_file_name(lat: f32, lng: f32) -> String {
    let clean = |v: f32| v.floor().abs();
    let ns = if lat >= 0.0 { "N" } else { "S" };
    let ew = if lng >= 0.0 && lng < 180.0 { "E" } else { "W" };
    format!("{}{:02}{}{:03}.hgt", ns, clean(lat), ew, clean(lng)).to_string()
}

/// generate srtm pixel coordinates for the given geoposition
fn srtm_file_coord(lat: f32, lng: f32, resolution: Resolution) -> (u32, u32) {
    let side = resolution.side() - 1;
    let pixel_index = |v: f32| ((v - v.floor()) * side as f32).round() as u32;
    (pixel_index(lng), side - pixel_index(lat))
}

pub struct Tiles<'a> {
    directory: PathBuf,
    handles: HashMap<String, &'a File>,
}
impl<'a> Tiles<'a> {
    pub fn new<P: AsRef<Path>>(directory: P) -> Self {
        Self {
            directory: directory.as_ref().to_path_buf(),
            handles: HashMap::default(),
        }
    }

    pub fn elevation(&self, lat: f32, lng: f32) -> Result<i16> {
        let filename = srtm_file_name(lat, lng);
        //let cachedfile = self.handles.get(&filename);
        //let mut file = cachedfile.unwrap_or(&File::open(self.directory.join(&filename))?);
        let mut file = File::open(self.directory.join(filename))?;
        let resolution = Resolution::try_from(file.metadata()?.len())?;
        let (x, y) = srtm_file_coord(lat, lng, resolution);
        let index = x + y * resolution.side();
        file.seek(SeekFrom::Start((index * 2) as u64))?;
        Ok(file.read_i16::<BigEndian>()?)
    }
}

// fn bounded_elevations(from: (f32, f32), to: (f32, f32)) -> Vec<(f32, f32, i16)> {}

// TESTS

#[test]
fn test_srtm_file_name() {
    let check = |lat, lng, expect| {
        let result = srtm_file_name(lat, lng);
        assert!(result == expect, "failed for (l={:?}, g={:?})", lat, lng)
    };

    // some cases
    check(49.0, -2.0, "N49W002.hgt");
    check(49.4, -1.3, "N49W002.hgt");
    check(50.9, 1.7, "N50E001.hgt");
    check(-50.9, 1.7, "S51E001.hgt");
    // check l,g around 0,0
    check(0.0, -0.1, "N00W001.hgt");
    check(-0.0, 0.1, "N00E000.hgt");
    check(0.1, -0.0, "N00E000.hgt");
    check(-0.1, 0.0, "S01E000.hgt");
    check(0.0, -0.0, "N00E000.hgt");
    check(-0.0, 0.0, "N00E000.hgt");
    // check around g=180
    check(45.0, 179.0, "N45E179.hgt");
    check(45.0, 180.0, "N45W180.hgt");
    check(45.0, 179.9, "N45E179.hgt");
    check(45.0, -180.0, "N45W180.hgt");
    // unsupported cases
    // check(0.0, -181.0, "N00E179");
    // check(0.0, 181.0, "N45W179");
    // check(91.0, 0.0, "N91E000");
    // check(-91.0, 0.0, "S91E000");
}

#[test]
fn test_srtm_file_coord() {
    let data = vec![
        (49.99972, -0.99972224, 1, 1),
        (49.444443, -0.99972224, 1, 2000),
        (49.02778, -0.99972224, 1, 3500),
        (49.99972, -0.027777791, 3500, 1),
        (49.444443, -0.027777791, 3500, 2000),
    ];

    data.into_iter().for_each(|(lat, lng, x, y)| {
        let result = srtm_file_coord(lat, lng, Resolution::SRTM1);
        assert!(result == (x, y), "failed for (l={:?}, g={:?})", lat, lng);
    });
}

#[test]
fn test_tiles_elevation() {
    let positions = vec![
        // (lat,lng,x_tile_offset,y_tile_offset,h)
        (49.99972, -0.99972224, 0),
        (49.444443, -0.99972224, 0),
        (49.02778, -0.99972224, 118),
        (49.99972, -0.027777791, 0),
        (49.444443, -0.027777791, 0),
    ];

    positions
        .into_iter()
        .for_each(|(lat, lng, h): (f32, f32, i16)| {
            // TODO: out var from build file
            const DIRPATH: &str = "/home/aurelien/Documents/data/hgt/";
            let tiles = Tiles::new(DIRPATH);
            let res = tiles.elevation(lat, lng);
            let height = res.unwrap();

            assert!(
                h == height,
                "Expected {}, got {} for ({}, {})",
                h,
                height,
                lat,
                lng
            );
        });
}

// may be useful to verify indexes
fn _resulting_lg(
    base: (f32, f32),
    offset_x_lng: u32,
    offset_y_lat: u32,
    res: Resolution,
) -> (f32, f32) {
    let adjust = |v| v / (res.side() - 1) as f32;
    (
        base.0 + 1.0 - adjust(offset_y_lat as f32),
        base.1 + adjust(offset_x_lng as f32),
    )
}
