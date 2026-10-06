//! Portable, bounded, inert FA resource editing. No operating-system dependencies.
#![no_std]
extern crate alloc;
#[cfg(test)]
extern crate std;
#[macro_use]
extern crate alloc as alloc_macros;
pub mod archive;
pub mod audio;
pub mod authoring;
pub mod brf;
pub mod clone_aircraft;
mod dcl;
pub mod definition;
pub mod dependencies;
pub mod document;
pub mod model;
pub mod picture;
pub mod resource_ops;
pub mod save;
#[allow(dead_code)]
mod schema;
pub mod validation;
pub type Result<T> = core::result::Result<T, alloc::string::String>;
pub(crate) fn invalid(message: &str) -> alloc::string::String {
    message.into()
}
pub(crate) fn slice(data: &[u8], start: usize, size: usize) -> Result<&[u8]> {
    data.get(
        start
            ..start
                .checked_add(size)
                .ok_or_else(|| invalid("Offset overflow"))?,
    )
    .ok_or_else(|| invalid("Resource truncated"))
}
pub(crate) fn u16_at(data: &[u8], at: usize) -> Result<usize> {
    Ok(u16::from_le_bytes(slice(data, at, 2)?.try_into().unwrap()) as usize)
}
pub(crate) fn u32_at(data: &[u8], at: usize) -> Result<usize> {
    Ok(u32::from_le_bytes(slice(data, at, 4)?.try_into().unwrap()) as usize)
}
