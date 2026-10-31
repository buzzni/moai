//! Control an already open explorer or web tab. The socket never starts a window.

pub mod call;
#[cfg(unix)]
pub mod place;
pub mod proposal;
pub mod serve;
pub mod wire;
