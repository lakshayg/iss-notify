use super::blinkt_common::{BlinktT, Result};

const N_PIXELS: usize = 8;
const CLEAR_PIXEL: (u8, u8, u8, f32) = (0, 0, 0, 0.0);

pub struct BlinktMock {
    local: [(u8, u8, u8, f32); N_PIXELS],
    pixel: [(u8, u8, u8, f32); N_PIXELS],
}

impl BlinktMock {
    pub fn new() -> Result<Self> {
        Ok(BlinktMock {
            local: [CLEAR_PIXEL; N_PIXELS],
            pixel: [CLEAR_PIXEL; N_PIXELS],
        })
    }
}

impl BlinktT for BlinktMock {
    fn set_all_pixels(&mut self, red: u8, green: u8, blue: u8) {
        for i in 0..N_PIXELS {
            self.set_pixel(i, red, green, blue);
        }
    }

    fn set_pixel(&mut self, pixel: usize, red: u8, green: u8, blue: u8) {
        self.local[pixel] = (red, green, blue, self.local[pixel].3);
    }

    fn show(&mut self) -> Result<()> {
        self.pixel = self.local;
        Ok(())
    }

    fn clear(&mut self) {
        self.pixel = [CLEAR_PIXEL; N_PIXELS];
    }

    fn set_clear_on_drop(&mut self, _clear_on_drop: bool) {}

    fn set_all_pixels_brightness(&mut self, brightness: f32) {
        for i in 0..N_PIXELS {
            self.local[i].3 = brightness;
        }
    }
}
