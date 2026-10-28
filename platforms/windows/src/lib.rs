//! Windows TSF adapter. Portable validation code has no Windows SDK dependency.
#![deny(unsafe_op_in_unsafe_fn)]

#[cfg(any(windows, test))]
mod keys;
#[cfg(any(windows, test))]
mod resource;
#[cfg(any(windows, test))]
mod session;
pub mod text;

#[cfg(windows)]
mod tsf;
