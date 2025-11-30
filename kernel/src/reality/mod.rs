//src/reality/mod.rs
pub mod causality;
pub mod dream;
pub mod intent;

pub use causality::*;
pub use dream::*;
pub use intent::*;

pub fn init() {
    causality::init();
    dream::init();
}