#[macro_use]
extern crate log;
extern crate chrono;
extern crate ctrlc;
extern crate fern;

mod blinkt_compat;
mod iss_feed;

use blinkt_compat::{Blinkt, BlinktT};
use chrono::Utc;
use std::cmp;
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError, Sender};
use std::thread;
use std::time::Duration;

static NOTIFY_DURATION: i64 = 300; // 5min to seconds
static BLINKT_REFRESH_DURATION: Duration = Duration::from_millis(1000);
static BLINKT_BRIGHTNESS: f32 = 0.05;

enum BlinktCmd {
    IssApproching(i64), // unix ts indicating the time of arrival
    Terminate,
}

fn terminate(blinkt: &mut dyn BlinktT) {
    warn!("Blinkt received Terminate");
    blinkt.set_all_pixels(0, 0, 0);
    blinkt.set_pixel(0, 255, 0, 0);
    blinkt.show().unwrap();
}

fn heartbeat(blinkt: &mut dyn BlinktT, heartbeat_state: &mut u8) {
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

fn iss_approaching(blinkt: &mut dyn BlinktT, until: i64) {
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

fn blinkt_mainloop(blinkt_rx: Receiver<BlinktCmd>) {
    let mut blinkt = Blinkt::new().unwrap();
    blinkt.set_clear_on_drop(false);
    blinkt.set_all_pixels_brightness(BLINKT_BRIGHTNESS);
    let mut heartbeat_state: u8 = 0;
    loop {
        match blinkt_rx.recv_timeout(BLINKT_REFRESH_DURATION) {
            Err(RecvTimeoutError::Disconnected) => panic!("blinkt_rx closed unexpectedly"),
            Err(RecvTimeoutError::Timeout) => heartbeat(&mut blinkt, &mut heartbeat_state),
            Ok(BlinktCmd::IssApproching(when)) => iss_approaching(&mut blinkt, when),
            Ok(BlinktCmd::Terminate) => {
                terminate(&mut blinkt);
                return;
            }
        }
    }
}

fn sightings_mainloop(blinkt_tx: Sender<BlinktCmd>, sigint_rx: Receiver<()>) {
    loop {
        info!("Retrieving RSS feed");
        let sightings = iss_feed::get_sightings();
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
                Err(RecvTimeoutError::Disconnected) => panic!("sigint_tx closed unexpectedly"),
                Err(RecvTimeoutError::Timeout) => {
                    info!("Sending ISS notification");
                    blinkt_tx.send(BlinktCmd::IssApproching(event_ts)).unwrap();
                }
                Ok(()) => {
                    blinkt_tx.send(BlinktCmd::Terminate).unwrap();
                    return; // signal received, exiting
                }
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
    let (sigint_tx, sigint_rx) = channel();

    ctrlc::set_handler(move || {
        warn!("Signal received, exiting");
        sigint_tx.send(()).unwrap();
    })
    .unwrap();

    let blinkt_handle = thread::spawn(move || {
        blinkt_mainloop(blinkt_rx);
    });

    sightings_mainloop(blinkt_tx, sigint_rx);

    blinkt_handle.join().unwrap();
}
