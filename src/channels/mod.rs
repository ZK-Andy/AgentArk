//! Communication channels - HTTP, Telegram, etc.

pub mod http;
pub mod web;

#[cfg(feature = "telegram")]
pub mod telegram;
