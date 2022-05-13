//!
//! # EasySrtm - SRTM directory reader.
//!
//! This library provides simple function to access SRTM files elevation from tiles stored in a directory.
//! Its main purpose is the get the elevation directly from latitude and longitude information.
//!
//! Note: to read height directly from file coordinates, use another library: `<https://github.com/grtlr/srtm>`.
//!
//! ## Usage
//!
//! The (probably only) use case would be:
//!
//! 1. [download](http://dwtkns.com/srtm30m/) SRTM files with
//! an [account](https://urs.earthdata.nasa.gov/)
//!
//! 2. Put tiles files in a folder
//!
//! 3. Use those files in your code with:
//!
//! ```
//! use easy_srtm::Tiles;
//! // contains at least *N49W002.hgt* to retrieve (lat 49.1, lng -1.6)
//! let folder = "the_foler_path";
//! let (lat, lng) = (49.1, -1.6);
//! let tiles = Tiles::new(folder);
//! if let Ok(altitude) = tiles.elevation(lat, lng) {
//!   // ...
//! }
//! # Ok::<(), anyhow::Error>(())
//!
//! ```
//!
//! ## SRTM description
//!
//! See SRTM [description](https://www.usgs.gov/centers/eros/science/usgs-eros-archive-digital-elevation-shuttle-radar-topography-mission-srtm-non)
//!
//! - Projection: Geographic
//! - Horizontal Datum: WGS84
//! - Vertical Datum: EGM96 (Earth Gravitational Model 1996)
//! - Vertical Units: Meters
//! - SRTM1: Spatial Resolution: 1 arc-second for global coverage (~30 meters)
//! - SRTM3: Spatial Resolution: 3 arc-seconds for global coverage (~90 meters)
//! - Raster Size:  1 degree tiles
//! - C-band Wavelength: 5.6 cm
//!

use anyhow::Result;
use byteorder::{self, BigEndian, ReadBytesExt};
use std::{
    cell::RefCell,
    collections::HashMap,
    fs::File,
    io::{Seek, SeekFrom},
    path::{Path, PathBuf},
};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum SrtmError {
    #[error("File size is not STRM(1|3) compatible")]
    ResolutionError,
}

const SRTM1_FSIZE: u64 = 3601 * 3601 * 2;
const SRTM3_FSIZE: u64 = 1201 * 1201 * 2;

/// Tile resolution.
///
/// SRTM files are squares.
///
/// - SRTM1: Spatial Resolution: 1 arc-second for global coverage (~30 meters)
/// - SRTM3: Spatial Resolution: 3 arc-seconds for global coverage (~90 meters)
#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub enum Resolution {
    SRTM1,
    SRTM3,
}

impl TryFrom<u64> for Resolution {
    type Error = SrtmError;

    /// Instanciate the resolution from the filesize
    ///
    /// This call will fail if the file size is neither
    /// SRTM1 (3601 * 3601 * 2) nor
    /// SRTM3 (1201 * 1201 * 2).
    ///
    /// # Argument
    ///
    /// * `filesize` - the size of the file to interpret as a tile.
    ///
    /// # Error
    ///
    /// * `SrtmError::ResolutionError` in case of bad filesize.
    ///
    /// # Example
    ///
    /// ```
    /// use easy_srtm::Resolution;
    /// // get file size with `file.metadata()?.len()`
    /// let resolution = Resolution::try_from(3601 * 3601 * 2);
    /// assert_eq!(resolution.unwrap(), Resolution::SRTM1);
    /// ```
    ///
    fn try_from(filesize: u64) -> Result<Self, Self::Error> {
        match filesize {
            SRTM1_FSIZE => Ok(Resolution::SRTM1),
            SRTM3_FSIZE => Ok(Resolution::SRTM3),
            _ => Err(Self::Error::ResolutionError),
        }
    }
}

/// SRTM files are squares.
/// The side size depends on the subformat:
/// - 3601 values for one earth arc degree (and an overlapped value) with SRTM1
/// - 1201 values for one earth arc degree (and an overlapped value) with SRTM3
impl Resolution {
    /// Returns the side size of a tile having this resolution.
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

/// Generate srtm pixel coordinates for the given geoposition.
/// - x coordinate increase when the longitude goes east (except at 180°).
/// - y coordinate increase with the latitude goes north.
fn srtm_file_coord(lat: f32, lng: f32, resolution: Resolution) -> (u32, u32) {
    let side = resolution.side() - 1;
    let pixel_index = |v: f32| ((v - v.floor()) * side as f32).round() as u32;
    (pixel_index(lng), side - pixel_index(lat))
}

/// A **Tiles** structure retains the directory path of the tiles.
/// It works as a context the retrieve values by calling `tiles.elevation(lat, lng)`.
///
/// This structure also handles opened files to prevent reopening a file each call.
///
/// ## Methods
///
/// - `pub fn new<P: AsRef<Path>>(directory: P) -> Self`
/// - `pub fn elevation(&self, lat: f32, lng: f32) -> Result<i16>`
#[derive(Debug)]
pub struct Tiles {
    directory: PathBuf,
    handles: RefCell<HashMap<String, File>>,
}
impl Tiles {
    /// Returns a Tiles object referencing a directory as SRTM files source.
    ///
    /// This directory should contain all .hgt files needed for requested lat/lng elevation.
    /// Those files have to be present with their original names.
    ///
    /// # Arguments
    ///
    /// * `directory` - The path of the directory containing SRTM files.
    ///
    /// # Examples
    ///
    /// ```
    /// use easy_srtm::Tiles;
    /// let your_directory = "/dev/null";
    /// let tiles = Tiles::new(your_directory);
    ///
    /// ```
    pub fn new<P: AsRef<Path>>(directory: P) -> Self {
        Self {
            directory: directory.as_ref().to_path_buf(),
            handles: RefCell::new(HashMap::default()),
        }
    }

    /// Returns the elevation (height) from latitude and longitude.
    ///
    /// This method return the elevation of the nearest point without the elevation's true
    /// position.
    /// This means that the same height is returned for a square around the true geoposition for
    /// the height.
    pub fn elevation(&self, lat: f32, lng: f32) -> Result<i16> {
        let filename = srtm_file_name(lat, lng);
        let cachehit = self.handles.borrow().get(&filename).is_some();

        if !cachehit {
            let file = File::open(self.directory.join(filename.clone()))?;
            self.handles.borrow_mut().insert(filename.clone(), file);
        }

        let height = self
            .handles
            .borrow_mut()
            .get(&filename)
            .map(|mut f| -> Result<i16> {
                let resolution = Resolution::try_from(f.metadata()?.len())?;
                let (x, y) = srtm_file_coord(lat, lng, resolution);
                let index = x + y * resolution.side();
                f.seek(SeekFrom::Start((index * 2) as u64))?;
                Ok(f.read_i16::<BigEndian>()?)
            })
            .unwrap()?;

        Ok(height)
    }

    // TODO fn to return the interpolated (linear) height for this geoposition

    // TODO fn to return the nearest geoposition having data and its height
}

// UNIT TESTS

/// Validate the srtm file name generation from lat lng
#[test]
fn it_generates_hgt_file_name_from_latlng() {
    let check = |lat, lng, expect| {
        let result = srtm_file_name(lat, lng);
        assert!(result == expect, "failed for (l={:?}, g={:?})", lat, lng)
    };

    check(49.0, -2.0, "N49W002.hgt");
    check(49.4, -1.3, "N49W002.hgt");
    check(50.9, 1.7, "N50E001.hgt");
    check(-50.9, 1.7, "S51E001.hgt");

    // check (l,g) ~ (0,0)
    check(0.0, -0.1, "N00W001.hgt");
    check(-0.0, 0.1, "N00E000.hgt");
    check(0.1, -0.0, "N00E000.hgt");
    check(-0.1, 0.0, "S01E000.hgt");
    check(0.0, -0.0, "N00E000.hgt");
    check(-0.0, 0.0, "N00E000.hgt");

    // check longitude~180
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

/// Validate the mapping of lat lng to srtm file coordinates
#[test]
fn it_computes_hgt_elevation_coordinates_from_latlng() {
    // reference data
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
