//! Hardware collectors.
//!
//! Each collector owns whatever state it needs to turn raw kernel counters
//! into rates (previous tick + timestamp) and is driven from a single
//! sampler thread, so none of them need interior mutability or locking.

pub mod battery;
pub mod cpu;
pub mod disk;
pub mod gpu;
pub mod network;
pub mod process;
pub mod sensors;
pub mod system;
