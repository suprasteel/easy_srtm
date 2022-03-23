use byteorder;
use std::fs;

// http://dwtkns.com/srtm30m/

const FILEPATH: &str = "/home/aurelien/Documents/data/hgt/N48W002.hgt";

fn main() {
    let result = Tile::from_file(FILEPATH);
    if let Ok(tile) = result {
        let Tile {
            latitude,
            longitude,
            resolution,
            ..
        } = tile;
        println!("tile : {}, {}, {:?}", latitude, longitude, resolution);

        for x in 3000..3002 {
            for y in 1800..1802 {
                let ll =
                    file_index_to_lat_lng((latitude as f32, longitude as f32), x, y, resolution);
                println!("lat={}, lng={} -> height={}", ll.0, ll.1, tile.get(x, y));
            }
        }
    } else {
        dbg!("{:?}", result.err());
    }
}

/// generate srtm file name containing elevation for the given geoposition
fn srtm_file_name(lat: f32, lng: f32) -> String {
    let clean = |v: f32| v.floor().abs();
    let ns = if lat >= 0.0 { "N" } else { "S" };
    let ew = if lng >= 0.0 && lng < 180.0 { "E" } else { "W" };
    format!("{}{:02}{}{:03}", ns, clean(lat), ew, clean(lng)).to_string()
}

/// generate srtm pixel coordinates for the given geoposition
fn srtm_file_coord(lat: f32, lng: f32, resolution: Resolution) -> (u32, u32) {
    let pixel_index = |v: f32| ((v - v.floor()) * resolution.size() as f32).round() as u32;
    (pixel_index(lat), pixel_index(lng))
}

fn elevation<P: AsRef<Path>>(srtm_directory: P, lat: f32, lng: f32) -> i16 {
    let srtm_file = srtm_file_name(lat, lng); // .push_str(".hgt");
    srtm_file.push_str(".hgt");
    let meta = fs::metadata(srtm_directory.as_ref().join(srtm_file))
        .ok()
        .and_then(|m: fs::Metadata| Some(m.len()));
    // let pixel_coord = srtm_file_coord(lat, lng, )
}

// fn elevations : read a set of point around position

#[test]
fn test_srtm_file_name() {
    let check = |lat, lng, expect| {
        let result = srtm_file_name(lat, lng);
        assert!(result == expect, "failed for (l={:?}, g={:?})", lat, lng)
    };

    // some cases
    check(49.0, -2.0, "N49W002");
    check(49.4, -1.3, "N49W002");
    check(50.9, 1.7, "N50E001");
    check(-50.9, 1.7, "S51E001");
    // check l,g around 0,0
    check(0.0, -0.1, "N00W001");
    check(-0.0, 0.1, "N00E000");
    check(0.1, -0.0, "N00E000");
    check(-0.1, 0.0, "S01E000");
    check(0.0, -0.0, "N00E000");
    check(-0.0, 0.0, "N00E000");
    // check around g=180
    check(45.0, 179.0, "N45E179");
    check(45.0, 180.0, "N45W180");
    check(45.0, 179.9, "N45E179");
    check(45.0, -180.0, "N45W180");
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

fn file_index_to_lat_lng(base: (f32, f32), x: u32, y: u32, res: Resolution) -> (f32, f32) {
    let adjust = |v| v / res.size() as f32;
    println!(
        "(base.0 + {}, base.1 + {})",
        adjust(x as f32),
        adjust(y as f32)
    );
    println!("x = {}, y = {}", x, y);
    (base.0 + adjust(x as f32), base.1 + adjust(y as f32))
}

use std::fs::File;

use byteorder::{BigEndian, ReadBytesExt};
use std::io;
use std::io::{BufReader, Read};
use std::path::Path;

#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub enum Resolution {
    SRTM1,
    SRTM3,
}

impl Resolution {
    fn size(&self) -> u16 {
        if self == &Resolution::SRTM1 {
            3601
        } else {
            1201
        }
    }
}

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
        self.data[self.idx(x, y)]
    }

    fn idx(&self, x: u32, y: u32) -> usize {
        assert!(x < self.extent() && y < self.extent());
        (y * self.extent() + x) as usize
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
