extern crate chrono;
extern crate chrono_tz;
extern crate regex;

use self::chrono_tz::Tz;
use self::regex::Regex;
use chrono::{DateTime, NaiveDateTime, TimeZone};
use std::collections::HashMap;
use std::io::BufReader;
use std::process::Command;
use std::str::FromStr;

// TODO: Remove hardcoded timezone and location
const LOCAL_TZ: &str = "America/Los_Angeles";
const RSS_URL: &str =
    "https://spotthestation.nasa.gov/sightings/xml_files/United_States_California_Redwood_City.xml";

#[derive(Debug)]
pub enum CurlError {
    Code(i32),
    TerminatedBySignal,
}

#[derive(Debug)]
pub struct Sighting {
    pub datetime: DateTime<Tz>,
    pub approach: SkyLocation,
    pub departure: SkyLocation,
    pub duration: i32,      // time for which ISS is visible in minutes
    pub max_elevation: i32, // max elevation in degrees
}

#[derive(Debug)]
pub struct SkyLocation {
    pub direction: String, // compass direction
    pub elevation: i32,    // angle in degrees measured from horizon
}

fn parse_datetime(s: &str) -> NaiveDateTime {
    NaiveDateTime::parse_from_str(s, "%A %b %d, %Y %l:%M %p").unwrap()
}

fn parse_skylocation(s: &str) -> SkyLocation {
    let regex = Regex::new(r"(?P<elevation>[0-9]+)° above (?P<direction>[NSEW]+)").unwrap();
    let caps = regex.captures(s).unwrap();
    SkyLocation {
        direction: caps["direction"].to_string(),
        elevation: caps["elevation"].parse().unwrap(),
    }
}

fn parse_duration(s: &str) -> i32 {
    let regex = Regex::new(r"(?P<duration>[0-9]+) minute").unwrap();
    let caps = regex.captures(s).unwrap();
    caps["duration"].parse().unwrap()
}

fn parse_elevation(s: &str) -> i32 {
    s.trim_end_matches("°").parse().unwrap()
}

fn curl(url: &str) -> Result<Vec<u8>, CurlError> {
    let out = Command::new("curl")
        .arg(url)
        .output()
        .expect("Error invoking curl");
    match out.status.code() {
        Some(0) => Ok(out.stdout),
        Some(i) => Err(CurlError::Code(i)),
        None => Err(CurlError::TerminatedBySignal),
    }
}

fn rss_item_to_map(item: &rss::Item) -> HashMap<String, String> {
    let desc = item.description().unwrap();
    desc.split('\n')
        .map(|s| s.trim_start_matches('\t'))
        .map(|s| s.trim_end_matches(" <br/>"))
        .map(|s| s.splitn(2, ": ").collect())
        .map(|s: Vec<_>| (s[0].to_string(), s[1].to_string()))
        .collect()
}

fn map_to_sighting(map: &HashMap<String, String>) -> Sighting {
    let datetime_str = format!("{} {}", map["Date"], map["Time"]);
    let datetime = Tz::from_str(LOCAL_TZ)
        .unwrap()
        .from_local_datetime(&parse_datetime(&datetime_str))
        .unwrap();
    Sighting {
        datetime,
        approach: parse_skylocation(&map["Approach"]),
        departure: parse_skylocation(&map["Departure"]),
        duration: parse_duration(&map["Duration"]),
        max_elevation: parse_elevation(&map["Maximum Elevation"]),
    }
}

pub fn get_sightings() -> Vec<Sighting> {
    let result = curl(RSS_URL).unwrap();
    let channel = rss::Channel::read_from(BufReader::new(result.as_slice())).unwrap();
    let iter = channel.items().iter();
    let mut vec: Vec<_> = iter
        .filter(|item| item.title().unwrap().contains("ISS Sighting"))
        .map(|item| rss_item_to_map(&item))
        .map(|map| map_to_sighting(&map))
        .collect();
    vec.sort_by(|a, b| a.datetime.cmp(&b.datetime));
    vec
}
