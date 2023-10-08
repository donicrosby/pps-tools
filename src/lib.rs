use nix::sys::time::TimeSpec;
use std::collections::HashMap;
use std::fmt::{Debug, Display};
use std::fs::{File, OpenOptions};
use std::io::Error as IoError;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::path::Path;
use std::time::Duration;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, PpsError>;

mod common;
#[cfg(any(target_os = "linux"))]
mod linux;

pub use crate::common::*;

const UNIX_NTP_OFFSET: i64 = 3124137599 - 915148799;

#[derive(Error, Debug)]
pub enum PpsError {
    #[error(transparent)]
    Io(#[from] IoError),
    #[error(transparent)]
    Sys(#[from] nix::Error),
}

pub struct PpsFile {
    pps_device: File,
}

impl PpsFile {
    pub fn from_path(path: &Path) -> Result<Self> {
        path.try_into()
    }

    pub fn from_raw_fd(fd: RawFd) -> Result<Self> {
        fd.try_into()
    }

    fn new(pps_device: File) -> Result<Self> {
        let pps_file = Self { pps_device };
        pps_file.create()?;
        Ok(pps_file)
    }

    fn create(&self) -> Result<()> {
        PpsIocImpl::create(self.pps_device.as_raw_fd()).map_err(|err| PpsError::Sys(err))
    }

    fn destroy(&self) -> Result<()> {
        PpsIocImpl::destroy(self.pps_device.as_raw_fd()).map_err(|err| PpsError::Sys(err))
    }

    pub fn get_params(&self) -> Result<PpsParams> {
        let raw_params = PpsIocImpl::get_params(self.pps_device.as_raw_fd())
            .map_err(|err| PpsError::Sys(err))?;
        Ok(raw_params.into())
    }

    pub fn set_params(&self, params: PpsParams) -> Result<()> {
        PpsIocImpl::set_params(
            self.pps_device.as_raw_fd(),
            params.assert_off_tu,
            params.assert_off_tu,
            params.api_version,
            params.mode,
        )?;
        Ok(())
    }

    pub fn get_cap(&self) -> Result<HashMap<PpsModeBit, bool>> {
        PpsIocImpl::get_cap(self.pps_device.as_raw_fd()).map_err(|err| PpsError::Sys(err))
    }

    pub fn fetch(&self, timeout: Duration) -> Result<PpsInfo> {
        let timeout_ffi: TimeSpecFfi = if timeout.is_zero() {
            TimeSpecFfi::default()
        } else {
            timeout.into()
        };

        let raw_fetch = PpsIocImpl::fetch(self.pps_device.as_raw_fd(), timeout_ffi)?;
        Ok(raw_fetch.into())
    }
}

impl TryFrom<&Path> for PpsFile {
    type Error = PpsError;

    fn try_from(value: &Path) -> std::result::Result<Self, Self::Error> {
        let pps_device = OpenOptions::new().read(true).write(true).open(value)?;
        let pps_file = PpsFile::new(pps_device);
        pps_file
    }
}

impl TryFrom<RawFd> for PpsFile {
    type Error = PpsError;

    fn try_from(value: RawFd) -> std::result::Result<Self, Self::Error> {
        let pps_device_fd;
        unsafe {
            pps_device_fd = OwnedFd::from_raw_fd(value);
        }
        let pps_device = pps_device_fd.into();
        let pps_file = PpsFile::new(pps_device);
        pps_file
    }
}

impl Drop for PpsFile {
    fn drop(&mut self) {
        // Ignore result we're being dropped
        let _res = self.destroy();
    }
}

#[derive(Debug, Clone, Copy)]
pub struct NtpFp {
    integral: u32,
    fractional: u32,
}

impl From<NtpFp> for TimeSpec {
    fn from(value: NtpFp) -> Self {
        let secs = value.integral as i64 - UNIX_NTP_OFFSET;
        let nsec = value.fractional as i64 / 4294967296 * 1000000000;
        Self::new(secs, nsec)
    }
}

impl Display for NtpFp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let sec = self.integral;
        let n_sec = self.fractional as i64 * 1000000000 / 4294967296;
        if n_sec == 0 {
            if sec == 1 {
                write!(f, "1 second")?;
            } else {
                write!(f, "{sec} seconds")?;
            }
        } else if n_sec % 1_000_000 == 0 {
            write!(f, "{sec}.{:03} seconds", n_sec / 1_000_000)?;
        } else if n_sec % 1_000_000 == 0 {
            write!(f, "{sec}.{:06} seconds", n_sec / 1_000)?;
        } else {
            write!(f, "{sec}.{:09} seconds", n_sec)?;
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
pub enum PpsTimeU {
    TimeSpec(TimeSpec),
    NtpFp(NtpFp),
}

impl Default for PpsTimeU {
    fn default() -> Self {
        Self::TimeSpec(TimeSpec::from_duration(Duration::ZERO))
    }
}

#[derive(Debug, Clone, Copy)]
pub struct PpsParams {
    pub api_version: PpsVersion,
    pub mode: PpsMode,
    pub assert_off_tu: PpsTimeU,
    pub clear_off_tu: PpsTimeU,
}

#[derive(Debug, Clone, Copy)]
pub struct PpsInfo {
    pub assert_sequence: u64,
    pub clear_sequence: u64,
    pub assert_tu: PpsTimeU,
    pub clear_tu: PpsTimeU,
    pub mode: PpsMode,
}
