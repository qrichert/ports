#![allow(clippy::module_name_repetitions)]

mod listening_ports;
pub mod lsof;
pub mod ps;
#[cfg(target_os = "linux")]
pub mod ss;

pub use listening_ports::{ListeningPort, ListeningPorts, ListeningPortsError, ProcessInfo};
