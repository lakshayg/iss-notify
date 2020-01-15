extern crate blinkt;

use super::blinkt_common::{BlinktT, Result};
use self::blinkt::Blinkt;
use log::error;

pub struct BlinktReal {
    blinkt: Blinkt,
}

impl BlinktReal {
    pub fn new() -> Result<Self> {
        Blinkt::new()
            .map(|blinkt| Self { blinkt })
            .map_err(|e| error!("Blinkt::new returned error {:?}", e))
    }
}

impl BlinktT for BlinktReal {
    fn set_all_pixels(&mut self, red: u8, green: u8, blue: u8) {
        self.blinkt.set_all_pixels(red, green, blue)
    }

    fn set_pixel(&mut self, pixel: usize, red: u8, green: u8, blue: u8) {
        self.blinkt.set_pixel(pixel, red, green, blue)
    }

    fn show(&mut self) -> Result<()> {
        self.blinkt
            .show()
            .map_err(|e| error!("Blinkt::show returned error {:?}", e))
    }

    fn clear(&mut self) {
        self.blinkt.clear()
    }

    fn set_clear_on_drop(&mut self, clear_on_drop: bool) {
        self.blinkt.set_clear_on_drop(clear_on_drop)
    }

    fn set_all_pixels_brightness(&mut self, brightness: f32) {
        self.blinkt.set_all_pixels_brightness(brightness)
    }
}
