//! Core navigation/registry logic, independent of Tauri and CEF.
pub mod error;
pub mod registry;
pub mod resolver;

pub use error::{AppError, AppResult};
pub use registry::{normalize_domain, RegistryClient};
pub use resolver::{Choice, NavigationOutcome, Navigator, OutcomeKind, Source};
