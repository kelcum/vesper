//! Fast Flags: catalog, validation, verification records, and applying them to Roblox.

pub mod apply;
pub mod catalog;
pub mod model;
pub mod validate;
pub mod verification;

pub use catalog::Catalog;
pub use model::FlagSet;
