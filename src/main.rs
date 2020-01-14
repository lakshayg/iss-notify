#[macro_use]
extern crate log;
extern crate blinkt;
extern crate chrono;
extern crate chrono_tz;
extern crate ctrlc;
extern crate fern;
extern crate regex;
extern crate rss;

use blinkt::Blinkt;
use chrono::{DateTime, NaiveDateTime, TimeZone, Utc};
use regex::Regex;
use std::cmp;
use std::collections::HashMap;
use std::io::BufReader;
use std::process::Command;
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError, Sender};
use std::thread;
use std::time::Duration;

static NOTIFY_DURATION: i64 = 300; // 5min to seconds
static BLINKT_REFRESH_DURATION: Duration = Duration::from_millis(1000);
static BLINKT_BRIGHTNESS: f32 = 0.05;

#[derive(Debug)]
enum CurlError {
    Code(i32),
    TerminatedBySignal,
}

#[derive(Debug)]
struct Sighting {
    datetime: DateTime<chrono_tz::Tz>,
    approach: SkyLocation,
    departure: SkyLocation,
    duration: i32,      // time for which ISS is visible in minutes
    max_elevation: i32, // max elevation in degrees
}

#[derive(Debug)]
struct SkyLocation {
    direction: String, // compass direction
    elevation: i32,    // angle in degrees measured from horizon
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
    let datetime = chrono_tz::Tz::America__Los_Angeles
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

fn get_sightings() -> Vec<Sighting> {
    let result = curl("https://spotthestation.nasa.gov/sightings/xml_files/United_States_California_Redwood_City.xml").unwrap();
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

enum BlinktCommand {
    IssApproching(i64), // unix ts indicating the time of arrival
    Terminate,
}

fn terminate(blinkt: &mut Blinkt) {
    warn!("Blinkt received Terminate");
    blinkt.set_all_pixels(0, 0, 0);
    blinkt.set_pixel(0, 255, 0, 0);
    blinkt.show().unwrap();
}

fn heartbeat(blinkt: &mut Blinkt, heartbeat_state: &mut u8) {
    *heartbeat_state = 255 as u8 - *heartbeat_state;
    blinkt.set_all_pixels(0, 0, 0);
    blinkt.set_pixel(0, 0, *heartbeat_state, 0);
    blinkt.show().unwrap();
}

fn color(n: i32, i: usize) -> (u8, u8, u8) {
    let deg = n as f64;
    let idx = i as f64 * 30.0;
    let r = 127.0 * (1.0 + (deg + idx + 0.0).to_radians().sin());
    let g = 127.0 * (1.0 + (deg + idx + 120.0).to_radians().sin());
    let b = 127.0 * (1.0 + (deg + idx + 240.0).to_radians().sin());
    (r as u8, g as u8, b as u8)
}

fn iss_approaching(blinkt: &mut Blinkt, until: i64) {
    info!("Blinkt received IssApproaching");
    while Utc::now().timestamp() < until {
        for n in (0..360).step_by(3) {
            for i in 0..8 as usize {
                let (r, g, b) = color(n, i);
                blinkt.set_pixel(i, r, g, b);
            }
            blinkt.show().unwrap();
            thread::sleep(Duration::from_millis(10));
        }
    }
    blinkt.clear();
    blinkt.show().unwrap();
}

fn blinkt_mainloop(blinkt_rx: Receiver<BlinktCommand>) {
    let mut blinkt = blinkt::Blinkt::new().unwrap();
    blinkt.set_clear_on_drop(false);
    blinkt.set_all_pixels_brightness(BLINKT_BRIGHTNESS);
    let mut heartbeat_state: u8 = 0;
    loop {
        match blinkt_rx.recv_timeout(BLINKT_REFRESH_DURATION) {
            Err(RecvTimeoutError::Disconnected) => panic!("blinkt_rx closed unexpectedly"),
            Err(RecvTimeoutError::Timeout) => heartbeat(&mut blinkt, &mut heartbeat_state),
            Ok(BlinktCommand::IssApproching(when)) => iss_approaching(&mut blinkt, when),
            Ok(BlinktCommand::Terminate) => {
                terminate(&mut blinkt);
                return;
            }
        }
    }
}

fn sightings_mainloop(blinkt_tx: Sender<BlinktCommand>, sigint_rx: Receiver<()>) {
    loop {
        info!("Retrieving RSS feed from spotthestation.nasa.gov");
        let sightings = get_sightings();
        for sighting in sightings {
            let event_ts = sighting.datetime.timestamp();
            let time_to_event = event_ts - Utc::now().timestamp();
            if time_to_event < 0 {
                debug!("Ignoring past event {}", sighting.datetime);
                continue;
            }
            let wait_duration = cmp::max(time_to_event - NOTIFY_DURATION, 0) as u64;
            let wait_duration = Duration::from_secs(wait_duration);
            info!("Next sighting in {} sec, {:#?}", time_to_event, sighting);
            match sigint_rx.recv_timeout(wait_duration) {
                Err(RecvTimeoutError::Timeout) => {
                    info!("Sending ISS notification");
                    blinkt_tx
                        .send(BlinktCommand::IssApproching(event_ts))
                        .unwrap();
                }
                Err(RecvTimeoutError::Disconnected) => panic!("impossible"),
                Ok(()) => return, // signal received, exiting
            }
        }
    }
}

fn init_logger() {
    fern::Dispatch::new()
        .format(|out, message, record| {
            out.finish(format_args!(
                "{}:{} {}",
                chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
                record.level(),
                message
            ))
        })
        .level(log::LevelFilter::Info)
        .chain(std::io::stdout())
        .chain(fern::log_file("iss-notify.log").unwrap())
        .apply()
        .unwrap();
}

fn main() {
    init_logger();

    let (blinkt_tx, blinkt_rx) = channel();
    let blinkt_tx2 = blinkt_tx.clone();
    let (sigint_tx, sigint_rx) = channel();

    ctrlc::set_handler(move || {
        warn!("Signal received, exiting");
        blinkt_tx.send(BlinktCommand::Terminate).unwrap();
        sigint_tx.send(()).unwrap();
    })
    .unwrap();

    let blinkt_handle = thread::spawn(move || {
        blinkt_mainloop(blinkt_rx);
    });

    sightings_mainloop(blinkt_tx2, sigint_rx);

    blinkt_handle.join().unwrap();
}
