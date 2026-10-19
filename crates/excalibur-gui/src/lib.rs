//! `excalibur-gui`: Hardware-accelerated desktop workstation powered by egui and eframe.

pub mod app;
pub mod audio;
pub mod chrome;
pub mod icons;
pub mod theme;
pub mod views;
pub mod widgets;

pub use app::{BrowserTab, GuiApp, XModeLayout, XModeTab};
pub use audio::AudioEngine;
