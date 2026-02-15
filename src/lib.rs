#[cfg(not(any(target_os = "linux", target_os = "macos")))]
compile_error!("rewinder supports only Linux and macOS targets.");

pub mod auth;
pub mod config;
pub mod db;
pub mod error;
pub mod models;
pub mod routes;
pub mod scanner;
pub mod templates;
pub mod trash;
pub mod watcher;
