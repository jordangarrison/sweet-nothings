//! Clipboard module
//!
//! Provides clipboard operations including copy and paste simulation.

use anyhow::Result;
use std::process::Command;

/// Trait for clipboard operations
#[allow(dead_code)]
pub trait Clipboard {
    /// Copy text to clipboard
    fn copy(&mut self, text: &str) -> Result<()>;

    /// Simulate a paste operation (Ctrl+V)
    fn paste(&self) -> Result<()>;

    /// Check if paste simulation is supported
    fn supports_paste(&self) -> bool;
}

/// Clipboard implementation using arboard
pub struct SystemClipboard {
    clipboard: arboard::Clipboard,
}

impl SystemClipboard {
    pub fn new() -> Result<Self> {
        let clipboard = arboard::Clipboard::new()?;
        Ok(Self { clipboard })
    }
}

impl Clipboard for SystemClipboard {
    fn copy(&mut self, text: &str) -> Result<()> {
        self.clipboard.set_text(text)?;
        Ok(())
    }

    fn paste(&self) -> Result<()> {
        paste_text()
    }

    fn supports_paste(&self) -> bool {
        // Check if we have the necessary tools available
        cfg!(target_os = "linux") || cfg!(target_os = "macos") || cfg!(target_os = "windows")
    }
}

/// Simulate a paste operation (Ctrl+V / Cmd+V)
#[cfg(target_os = "linux")]
fn paste_text() -> Result<()> {
    // Check if running under Wayland
    if std::env::var("WAYLAND_DISPLAY").is_ok() {
        // Use wtype for Wayland
        if which("wtype").is_ok() {
            Command::new("wtype")
                .args(["-M", "ctrl", "-s", "50", "v", "-m", "ctrl"])
                .status()?;
            return Ok(());
        }
    }

    // Fall back to xdotool for X11
    if which("xdotool").is_ok() {
        Command::new("xdotool").args(["key", "ctrl+v"]).status()?;
        return Ok(());
    }

    anyhow::bail!("No paste tool available. Install wtype (Wayland) or xdotool (X11)")
}

#[cfg(target_os = "macos")]
fn paste_text() -> Result<()> {
    Command::new("osascript")
        .args(["-e", "tell application \"System Events\" to keystroke \"v\" using command down"])
        .status()?;
    Ok(())
}

#[cfg(target_os = "windows")]
fn paste_text() -> Result<()> {
    Command::new("powershell")
        .args([
            "-Command",
            "Add-Type -AssemblyName System.Windows.Forms; [System.Windows.Forms.SendKeys]::SendWait('^v')",
        ])
        .status()?;
    Ok(())
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
fn paste_text() -> Result<()> {
    anyhow::bail!("Paste simulation not supported on this platform")
}

/// Check if a binary exists in PATH
fn which(name: &str) -> Result<()> {
    let output = Command::new("which").arg(name).output()?;
    if output.status.success() {
        Ok(())
    } else {
        anyhow::bail!("not found")
    }
}
