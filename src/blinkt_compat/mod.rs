mod blinkt_common;
pub use self::blinkt_common::*;

mod blinkt_mock;
pub use self::blinkt_mock::*;

#[cfg(feature = "default")]
mod blinkt_real;
#[cfg(feature = "default")]
pub type Blinkt = self::blinkt_real::BlinktReal;

#[cfg(not(feature = "default"))]
pub type Blinkt = self::blinkt_mock::BlinktMock;
