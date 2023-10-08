use std::collections::HashMap;
use std::fmt;
use std::os::fd::RawFd;
use strum::IntoEnumIterator;
use strum_macros::EnumIter;

pub(crate) use nix::{
    ioctl_none, ioctl_read, ioctl_readwrite, ioctl_write_ptr, Result as NixResult,
};

#[cfg(any(target_os = "linux"))]
pub(crate) use crate::linux::{
    LinuxPpsInfo as PpsInfoFfi, LinuxPpsIoc as PpsIocImpl, LinuxPpsParams as PpsParamsFfi,
    LinuxTimeSpec as TimeSpecFfi,
};
use crate::PpsTimeU;

#[derive(Debug, Clone, Copy, EnumIter, PartialEq, Eq, Hash)]
#[repr(C)]
pub enum PpsModeBit {
    CaptureAssert = 0x01,
    CaptureClear = 0x02,
    CaptureBoth = 0x03,
    OffsetAssert = 0x10,
    OffsetClear = 0x20,
    CanWait = 0x100,
    CanPoll = 0x200,
    // Kernel actions
    EchoAssert = 0x40,
    EchoClear = 0x80,
    // Timestamp formats
    TsFmtTSpec = 0x1000,
    TsFmtNTPFP = 0x2000,
}

impl fmt::Display for PpsModeBit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CaptureAssert => write!(f, "PPS_CAPTUREASSERT"),
            Self::CaptureClear => write!(f, "PPS_CAPTURECLEAR"),
            Self::CaptureBoth => write!(f, "PPS_CAPTUREBOTH"),
            Self::OffsetAssert => write!(f, "PPS_OFFSETASSERT"),
            Self::OffsetClear => write!(f, "PPS_OFFSETCLEAR"),
            Self::CanWait => write!(f, "PPS_CANWAIT"),
            Self::CanPoll => write!(f, "PPS_CANPOLL"),
            Self::EchoAssert => write!(f, "PPS_ECHOASSERT"),
            Self::EchoClear => write!(f, "PPS_ECHOCLEAR"),
            Self::TsFmtTSpec => write!(f, "PPS_TSFMT_TSPEC"),
            Self::TsFmtNTPFP => write!(f, "PPS_TSFMT_NTPFP"),
        }
    }
}

pub struct PpsModeBuilder {
    mode: i32,
}

impl PpsModeBuilder {
    pub fn new() -> Self {
        Self { mode: 0 }
    }

    pub fn add_mode(&mut self, bit: PpsModeBit) -> &mut Self {
        self.mode |= bit as i32;
        self
    }

    pub fn remove_mode(&mut self, bit: PpsModeBit) -> &mut Self {
        self.mode &= !(bit as i32);
        self
    }

    pub fn build(&self) -> PpsMode {
        PpsMode(self.mode)
    }
}

#[derive(Debug, Copy, Clone, Default)]
pub struct PpsMode(i32);

impl PpsMode {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn get_bits(&self) -> HashMap<PpsModeBit, bool> {
        let mut bits = HashMap::new();
        for bit in PpsModeBit::iter() {
            bits.insert(bit, self.mode_is_set(bit));
        }
        bits
    }

    pub(crate) fn mode_is_set(&self, mode: PpsModeBit) -> bool {
        let bit_test = self.0 & mode as i32;
        bit_test > 0
    }
}

impl From<PpsMode> for i32 {
    fn from(value: PpsMode) -> Self {
        value.0
    }
}

impl From<i32> for PpsMode {
    fn from(value: i32) -> Self {
        Self(value)
    }
}

#[derive(Debug, Copy, Clone)]
pub struct PpsVersion(i32);

impl PpsVersion {
    pub fn new(version: i32) -> Self {
        Self(version)
    }
}

impl Default for PpsVersion {
    /// Default to PPS Version 1
    fn default() -> Self {
        Self::new(1)
    }
}

impl From<PpsVersion> for i32 {
    fn from(value: PpsVersion) -> Self {
        value.0
    }
}

pub trait PpsIoc {
    fn create(fd: RawFd) -> NixResult<()>;
    fn destroy(fd: RawFd) -> NixResult<()>;
    fn get_params(fd: RawFd) -> NixResult<PpsParamsFfi>;
    fn set_params(
        fd: RawFd,
        assert_offset: PpsTimeU,
        clear_offset: PpsTimeU,
        api_version: PpsVersion,
        mode: PpsMode,
    ) -> NixResult<()>;
    fn get_cap(fd: RawFd) -> NixResult<HashMap<PpsModeBit, bool>>;
    fn fetch(fd: RawFd, timeout: TimeSpecFfi) -> NixResult<PpsInfoFfi>;
}
