use super::common::{ioctl_read, ioctl_readwrite, ioctl_write_ptr, PpsIoc};
use crate::{PpsInfo, PpsMode, PpsModeBit, PpsParams, PpsTimeU, PpsVersion};
use nix::{libc::c_int, sys::time::TimeSpec, Result};
use std::collections::HashMap;
use std::os::fd::RawFd;
use std::time::Duration;

const PPS_MAGIC: u8 = b'p';
const PPS_IOC_GETPARAMS: u8 = 0xA1;
const PPS_IOC_SETPARAMS: u8 = 0xA2;
const PPS_IOC_GETCAP: u8 = 0xA3;
const PPS_IOC_FETCH: u8 = 0xA4;

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct LinuxTimeSpec {
    pub tv_sec: i64,
    pub tv_nsec: i32,
    pub flags: u32,
}

impl Default for LinuxTimeSpec {
    fn default() -> Self {
        Self {
            tv_sec: 0,
            tv_nsec: 0,
            flags: 1,
        }
    }
}

impl From<LinuxTimeSpec> for TimeSpec {
    fn from(value: LinuxTimeSpec) -> Self {
        TimeSpec::new(value.tv_sec, value.tv_nsec as i64)
    }
}

impl From<TimeSpec> for LinuxTimeSpec {
    fn from(value: TimeSpec) -> Self {
        Self {
            tv_sec: value.tv_sec(),
            tv_nsec: value.tv_nsec() as i32,
            flags: 0,
        }
    }
}

impl From<Duration> for LinuxTimeSpec {
    fn from(value: Duration) -> Self {
        let ts: TimeSpec = value.into();
        ts.into()
    }
}

#[derive(Clone, Copy)]
#[repr(C)]
pub union LinuxPpsTime {
    pub tspec: LinuxTimeSpec,
}

impl Default for LinuxPpsTime {
    fn default() -> Self {
        Self {
            tspec: LinuxTimeSpec::default(),
        }
    }
}

impl From<PpsTimeU> for LinuxPpsTime {
    fn from(value: PpsTimeU) -> Self {
        match value {
            PpsTimeU::TimeSpec(ts) => Self { tspec: ts.into() },
            PpsTimeU::NtpFp(ntp) => {
                let ts: TimeSpec = ntp.into();
                Self { tspec: ts.into() }
            }
        }
    }
}

fn get_tus_from_pps_time(
    mode: PpsMode,
    assert_tu: LinuxPpsTime,
    clear_tu: LinuxPpsTime,
) -> (PpsTimeU, PpsTimeU) {
    assert!(mode.mode_is_set(PpsModeBit::TsFmtTSpec));
    let assert_tu_ts;
    let clear_tu_ts;
    unsafe {
        assert_tu_ts = assert_tu.tspec;
        clear_tu_ts = clear_tu.tspec;
    }
    let assert_tu = PpsTimeU::TimeSpec(assert_tu_ts.into());
    let clear_tu = PpsTimeU::TimeSpec(clear_tu_ts.into());
    (assert_tu, clear_tu)
}

#[derive(Clone, Copy, Default)]
#[repr(C)]
pub struct LinuxPpsInfo {
    pub assert_sequence: i32,
    pub clear_sequence: i32,
    pub assert_tu: LinuxPpsTime,
    pub clear_tu: LinuxPpsTime,
    pub current_mode: c_int,
}

impl From<LinuxPpsInfo> for PpsInfo {
    fn from(value: LinuxPpsInfo) -> Self {
        let (assert_tu, clear_tu) =
            get_tus_from_pps_time(value.current_mode.into(), value.assert_tu, value.clear_tu);
        Self {
            assert_sequence: value.assert_sequence as u64,
            assert_tu,
            clear_sequence: value.clear_sequence as u64,
            clear_tu,
            mode: value.current_mode.into(),
        }
    }
}

#[derive(Clone, Copy, Default)]
#[repr(C)]
pub struct LinuxPpsParams {
    pub api_version: c_int,
    pub mode: c_int,
    pub assert_off_tu: LinuxPpsTime,
    pub clear_off_tu: LinuxPpsTime,
}

impl LinuxPpsParams {
    pub fn new(
        api_version: c_int,
        mode: c_int,
        assert_off_tu: LinuxPpsTime,
        clear_off_tu: LinuxPpsTime,
    ) -> Self {
        Self {
            api_version,
            mode,
            assert_off_tu,
            clear_off_tu,
        }
    }
}

impl From<LinuxPpsParams> for PpsParams {
    fn from(value: LinuxPpsParams) -> Self {
        let (assert_off_tu, clear_off_tu) =
            get_tus_from_pps_time(value.mode.into(), value.assert_off_tu, value.clear_off_tu);
        Self {
            api_version: PpsVersion::new(value.api_version),
            mode: value.mode.into(),
            assert_off_tu,
            clear_off_tu,
        }
    }
}

#[derive(Clone, Copy, Default)]
#[repr(C)]
pub struct LinuxPpsFetchArgs {
    pub info: LinuxPpsInfo,
    pub timeout: LinuxTimeSpec,
}

ioctl_read!(
    linux_get_pps_params,
    PPS_MAGIC,
    PPS_IOC_GETPARAMS,
    LinuxPpsParams
);
ioctl_write_ptr!(
    linux_set_pps_params,
    PPS_MAGIC,
    PPS_IOC_SETPARAMS,
    LinuxPpsParams
);
ioctl_read!(linux_get_pps_cap, PPS_MAGIC, PPS_IOC_GETCAP, i32);
ioctl_readwrite!(
    linux_fetch_pps_info,
    PPS_MAGIC,
    PPS_IOC_FETCH,
    LinuxPpsFetchArgs
);

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct LinuxPpsIoc;

impl PpsIoc for LinuxPpsIoc {
    /// Linux does not support creating
    fn create(_fd: RawFd) -> Result<()> {
        Ok(())
    }
    /// Linux does not support destroying
    fn destroy(_fd: RawFd) -> Result<()> {
        Ok(())
    }

    fn get_params(fd: RawFd) -> Result<LinuxPpsParams> {
        let mut params = LinuxPpsParams::default();
        let _res;
        unsafe {
            _res = linux_get_pps_params(fd, &mut params)?;
        }
        Ok(params)
    }
    fn set_params(
        fd: RawFd,
        assert_offset: PpsTimeU,
        clear_offset: PpsTimeU,
        api_version: PpsVersion,
        mode: PpsMode,
    ) -> Result<()> {
        let params = LinuxPpsParams::new(
            api_version.into(),
            mode.into(),
            assert_offset.into(),
            clear_offset.into(),
        );
        let _res;
        unsafe {
            _res = linux_set_pps_params(fd, &params)?;
        }
        Ok(())
    }
    fn get_cap(fd: RawFd) -> Result<HashMap<PpsModeBit, bool>> {
        let mut ffi_cap: c_int = 0;
        let cap: PpsMode;
        let _res;
        unsafe {
            _res = linux_get_pps_cap(fd, &mut ffi_cap)?;
        }
        cap = ffi_cap.into();
        Ok(cap.get_bits())
    }
    fn fetch(fd: RawFd, timeout: LinuxTimeSpec) -> Result<LinuxPpsInfo> {
        let mut fetch_args = LinuxPpsFetchArgs::default();
        fetch_args.timeout = timeout;
        let _res;
        unsafe {
            _res = linux_fetch_pps_info(fd, &mut fetch_args)?;
        }
        Ok(fetch_args.info)
    }
}
