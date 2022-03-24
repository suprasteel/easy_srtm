use anyhow::Result;
use byteorder;
use dotenv::Result;
use std::fs;
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

const DIRPATH: &str = "/home/aurelien/Documents/data/hgt/";
const FILEPATH: &str = "/home/aurelien/Documents/data/hgt/N49W001.hgt";

fn main() {
    dotenv::from_filename(".env").ok();
    env_logger::init();
    let positions = vec![
        // (lat,lng,x,y,h)
        (49.99972, -0.99972224, 1, 1, 0, 3602),
        (49.444443, -0.99972224, 1, 2000, 0, 7202001),
        (49.02778, -0.99972224, 1, 3500, 118, 12603501),
        (49.99972, -0.027777791, 3500, 1, 0, 7101),
        (49.444443, -0.027777791, 3500, 2000, 0, 7205500),
        (49.02778, -0.027777791, 3500, 3500, 67, 12607000),
        (49.02778, -0.027777791, 3600, 3600, 67, 12607000),
    ];

    let positions2 = positions.clone();

    /*    let result = Tile::from_file(FILEPATH);
    if let Ok(tile) = result {
    let Tile {
    latitude,
    longitude,
    resolution,
    ..
    } = tile;
    println!(
    "tile : {:?},  {}, {}, {:?}",
    FILEPATH.split("/").last(),
    latitude,
    longitude,
    resolution
    );

    log::debug!(target: "srtm", "(lat,lng,x,y,h)");
    positions
    .into_iter()
    .for_each(|(_, _, x, y, _, index): (f32, f32, u32, u32, u16, u32)| {
    let (lat, lng) = resulting_lg((latitude as f32, longitude as f32), x, y, resolution);
    log::debug!(target: "srtm", "({},{},{},{},{},{})",lat, lng, x, y, tile.get(x, y), index);
    });
    } else {
    dbg!("{:?}", result.err());
    }*/

    println!(" --- ");

    positions2
        .into_iter()
        .for_each(|(lat, lng, x, y, h, idx): (f32, f32, u32, u32, u16, u32)| {
            log::debug!(target: "srtm", "expect ({},{},{}, index={})", x, y, h, idx);
            let res = elevation(DIRPATH, lat, lng);
            let height = res.unwrap();
            log::debug!(target: "srtm", "({},{},{},{},{})",lat, lng, x, y, height);
        });
}

#[derive(Error, Debug)]
enum SrtmError {
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

impl Resolution {
    /// SRTM files are squares.
    /// The side size depends on the subformat:
    /// - 3601 values for one earth arc degree (and an overlapped value) with SRTM1
    /// - 1201 values for one earth arc degree (and an overlapped value) with SRTM3
    fn side(&self) -> u32 {
        match &self {
            Resolution::SRTM1 => 3601,
            Resolution::SRTM3 => 1201,
        }
    }
    /// Get the number of values available with this format
    fn count(&self) -> u32 {
        self.side() * self.side()
    }
    /// Get the file size of this resolution
    fn file_size(&self) -> u32 {
        self.count() * 2
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

/// get elevation from lat/lng using srtm files
fn elevation<P: AsRef<Path>>(srtm_directory: P, lat: f32, lng: f32) -> Result<i16> {
    let resolution = Resolution::SRTM1;
    let filename = srtm_file_name(lat, lng);
    let mut file = File::open(srtm_directory.as_ref().join(filename))?;
    let resolution = Resolution::try_from(file.metadata()?.len())?;
    let (x, y) = srtm_file_coord(lat, lng, resolution);
    let index = x + y * resolution.side();
    dbg!(index);
    file.seek(SeekFrom::Start((index * 2) as u64))?;
    Ok(file.read_i16::<BigEndian>()?)
}

// fn bounded_elevations(from: (f32, f32), to: (f32, f32)) -> Vec<(f32, f32, i16)> {}

// fn elevations : read a set of point around position

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
    let check = |lat, lng, expect| {
        let result = srtm_file_coord(lat, lng, Resolution::SRTM1);
        assert!(result == expect, "failed for (l={:?}, g={:?})", lat, lng)
    };

    check(48.833103, -1.5001389, (3000, 1800));
}

#[test]
fn test_elevation() {
    let h = elevation(DIRPATH, 48.833103, -1.5001389);
    let h = h.unwrap();
    println!("ELEVATION IS {}", h); //160
    assert_eq!(h, 160);
}

fn resulting_lg(
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

use std::fs::File;

use byteorder::{BigEndian, ReadBytesExt};
use std::io::{self, Seek, SeekFrom};
use std::io::{BufReader, Read};
use std::path::Path;

#[derive(Debug)]
pub struct Tile {
    pub latitude: i32,
    pub longitude: i32,
    pub resolution: Resolution,
    data: Vec<i16>,
}

#[derive(Debug)]
pub enum Error {
    ParseLatLong,
    Filesize,
    Read,
}

impl Tile {
    fn new_empty(lat: i32, lng: i32, res: Resolution) -> Tile {
        Tile {
            latitude: lat,
            longitude: lng,
            resolution: res,
            data: Vec::new(),
        }
    }

    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Tile, Error> {
        let (lat, lng) = get_lat_long(&path)?;
        let res = get_resolution(&path).ok_or(Error::Filesize)?;
        let file = File::open(&path).map_err(|_| Error::Read)?;
        let reader = BufReader::new(file);
        let mut tile = Tile::new_empty(lat, lng, res);
        tile.data = parse(reader, tile.resolution).map_err(|_| Error::Read)?;
        Ok(tile)
    }

    pub fn extent(&self) -> u32 {
        match self.resolution {
            Resolution::SRTM1 => 3601,
            Resolution::SRTM3 => 1201,
        }
    }

    pub fn max_height(&self) -> i16 {
        *(self.data.iter().max().unwrap())
    }

    pub fn get(&self, x: u32, y: u32) -> i16 {
        log::warn!("lib found index = {}", self.idx(x, y));
        self.data[self.idx(x, y)]
    }

    fn idx(&self, x: u32, y: u32) -> usize {
        assert!(x < self.extent() && y < self.extent());
        (y * (self.extent()) + x) as usize
    }
}

fn get_resolution<P: AsRef<Path>>(path: P) -> Option<Resolution> {
    let from_metadata = |m: fs::Metadata| match m.len() {
        25934402 => Some(Resolution::SRTM1),
        2884802 => Some(Resolution::SRTM3),
        _ => None,
    };
    fs::metadata(path).ok().and_then(from_metadata)
}

// FIXME Better error handling.
fn get_lat_long<P: AsRef<Path>>(path: P) -> Result<(i32, i32), Error> {
    let stem = path.as_ref().file_stem().ok_or(Error::ParseLatLong)?;
    let desc = stem.to_str().ok_or(Error::ParseLatLong)?;
    if desc.len() != 7 {
        return Err(Error::ParseLatLong);
    }

    let get_char = |n| desc.chars().nth(n).ok_or(Error::ParseLatLong);
    let lat_sign = if get_char(0)? == 'N' { 1 } else { -1 };
    let lat: i32 = desc[1..3].parse().map_err(|_| Error::ParseLatLong)?;

    let lng_sign = if get_char(3)? == 'E' { 1 } else { -1 };
    let lng: i32 = desc[4..7].parse().map_err(|_| Error::ParseLatLong)?;
    Ok((lat_sign * lat, lng_sign * lng))
}

fn total_size(res: Resolution) -> u32 {
    match res {
        Resolution::SRTM1 => 3601 * 3601,
        Resolution::SRTM3 => 1201 * 1201,
    }
}

fn parse<R: Read>(reader: R, res: Resolution) -> io::Result<Vec<i16>> {
    let mut reader = reader;
    let mut data = Vec::new();
    for _ in 0..total_size(res) {
        let h = reader.read_i16::<BigEndian>()?;
        data.push(h);
    }
    Ok(data)
}

#[cfg(test)]
mod tests {
    use super::get_lat_long;
    use std::path::Path;

    #[test]
    fn parse_latitute_and_longitude() {
        let ne = Path::new("N35E138.hgt");
        assert_eq!(get_lat_long(&ne).unwrap(), (35, 138));

        let nw = Path::new("N35W138.hgt");
        assert_eq!(get_lat_long(&nw).unwrap(), (35, -138));

        let se = Path::new("S35E138.hgt");
        assert_eq!(get_lat_long(&se).unwrap(), (-35, 138));

        let sw = Path::new("S35W138.hgt");
        assert_eq!(get_lat_long(&sw).unwrap(), (-35, -138));
    }
}
