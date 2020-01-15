pub type Result<T> = std::result::Result<T, ()>;

pub trait BlinktT {
    fn set_all_pixels(&mut self, red: u8, green: u8, blue: u8);
    fn set_pixel(&mut self, pixel: usize, red: u8, green: u8, blue: u8);
    fn show(&mut self) -> Result<()>;
    fn clear(&mut self);
    fn set_clear_on_drop(&mut self, clear_on_drop: bool);
    fn set_all_pixels_brightness(&mut self, brightness: f32);
}
