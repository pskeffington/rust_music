//! Linux ALSA kernel UAPI PCM playback; no libasound, PulseAudio, JACK or PipeWire.
//!
//! The only non-Rust calls are the platform's ioctl ABI and the filesystem node
//! `/dev/snd/pcmC{card}D{device}p`. No mixer plugin, resampler or server is used.
//! Only 64-bit Linux kernel UAPI layouts are implemented; physical playback and
//! the exact DAC path still require device-specific acceptance testing.
use rust_music_core::{AudioFormat, MusicError, PcmBlock, PcmSink, Result};
use std::{
    ffi::{c_int, c_ulong, c_void},
    fs::{File, OpenOptions},
    io,
    mem::size_of,
    os::fd::AsRawFd,
    path::PathBuf,
};

#[cfg(not(target_pointer_width = "64"))]
compile_error!("The direct ALSA UAPI backend currently supports 64-bit Linux only");

const ACCESS_RW_INTERLEAVED: u32 = 3;
const FORMAT_U8: u32 = 1;
const FORMAT_S16_LE: u32 = 2;
const FORMAT_S32_LE: u32 = 10;
const FORMAT_S24_3LE: u32 = 32;
const NO_RESAMPLE: u32 = 1;
const EXACT_INTEGER: u32 = 1 << 2;
const SAMPLE_BITS: usize = 0;
const FRAME_BITS: usize = 1;
const CHANNELS: usize = 2;
const RATE: usize = 3;
const PERIOD_SIZE: usize = 5;
const PERIODS: usize = 7;
const BUFFER_SIZE: usize = 9;

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct Mask {
    bits: [u32; 8],
}
impl Mask {
    fn only(&mut self, value: u32) {
        self.bits = [0; 8];
        self.bits[(value / 32) as usize] = 1 << (value % 32);
    }
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct Interval {
    min: u32,
    max: u32,
    flags: u32,
}
impl Interval {
    fn exact(&mut self, value: u32) {
        *self = Self {
            min: value,
            max: value,
            flags: EXACT_INTEGER,
        };
    }
    fn range(&mut self, min: u32, max: u32) {
        *self = Self {
            min,
            max,
            flags: EXACT_INTEGER,
        };
    }
}

// ABI snapshot: /usr/include/sound/asound.h snd_pcm_hw_params (Linux 64 bit).
// The sizes and offsets are checked by unit tests against the kernel UAPI.
#[repr(C)]
struct HwParams {
    flags: u32,
    masks: [Mask; 3],
    mres: [Mask; 5],
    intervals: [Interval; 12],
    ires: [Interval; 9],
    rmask: u32,
    cmask: u32,
    info: u32,
    msbits: u32,
    rate_num: u32,
    rate_den: u32,
    fifo_size: usize,
    sync: [u8; 16],
    reserved: [u8; 48],
}
impl HwParams {
    fn new(format: AudioFormat) -> Self {
        let mut value = Self {
            flags: NO_RESAMPLE,
            masks: [Mask {
                bits: [u32::MAX; 8],
            }; 3],
            mres: [Mask::default(); 5],
            intervals: [Interval {
                min: 0,
                max: u32::MAX,
                flags: 0,
            }; 12],
            ires: [Interval::default(); 9],
            rmask: u32::MAX,
            cmask: 0,
            info: 0,
            msbits: 0,
            rate_num: 0,
            rate_den: 0,
            fifo_size: 0,
            sync: [0; 16],
            reserved: [0; 48],
        };
        let sample_format = match format.bits_per_sample {
            8 => FORMAT_U8,
            16 => FORMAT_S16_LE,
            24 => FORMAT_S24_3LE,
            32 => FORMAT_S32_LE,
            _ => unreachable!("format validated before HwParams::new"),
        };
        value.masks[0].only(ACCESS_RW_INTERLEAVED);
        value.masks[1].only(sample_format);
        value.masks[2].only(0); // PCM subformat STD
        value.intervals[SAMPLE_BITS].exact(u32::from(format.bits_per_sample));
        value.intervals[FRAME_BITS]
            .exact(u32::from(format.bits_per_sample) * u32::from(format.channels));
        value.intervals[CHANNELS].exact(u32::from(format.channels));
        value.intervals[RATE].exact(format.sample_rate);
        value.intervals[PERIOD_SIZE].range(128, 8192);
        value.intervals[PERIODS].range(2, 8);
        value.intervals[BUFFER_SIZE].range(256, 65_536);
        value
    }
    fn exact_format(&self, format: AudioFormat) -> bool {
        [
            (SAMPLE_BITS, u32::from(format.bits_per_sample)),
            (
                FRAME_BITS,
                u32::from(format.bits_per_sample) * u32::from(format.channels),
            ),
            (CHANNELS, u32::from(format.channels)),
            (RATE, format.sample_rate),
        ]
        .into_iter()
        .all(|(idx, expected)| {
            self.intervals[idx].min == expected && self.intervals[idx].max == expected
        })
    }
}

#[repr(C)]
struct SwParams {
    tstamp_mode: c_int,
    period_step: u32,
    sleep_min: u32,
    avail_min: usize,
    xfer_align: usize,
    start_threshold: usize,
    stop_threshold: usize,
    silence_threshold: usize,
    silence_size: usize,
    boundary: usize,
    proto: u32,
    tstamp_type: u32,
    reserved: [u8; 56],
}
impl SwParams {
    fn new(period: usize, buffer: usize) -> Self {
        let mut boundary = buffer;
        while boundary <= (isize::MAX as usize / 2).saturating_sub(buffer) {
            boundary *= 2;
        }
        Self {
            tstamp_mode: 0,
            period_step: 1,
            sleep_min: 0,
            avail_min: period,
            xfer_align: 1,
            start_threshold: 1,
            stop_threshold: buffer,
            silence_threshold: 0,
            silence_size: 0,
            boundary,
            proto: 0,
            tstamp_type: 0,
            reserved: [0; 56],
        }
    }
}

#[repr(C)]
struct Transfer {
    result: isize,
    buffer: *mut c_void,
    frames: usize,
}

// ioctl is part of the target OS's C ABI, not a downloaded sound library.
unsafe extern "C" {
    fn ioctl(fd: c_int, request: c_ulong, ...) -> c_int;
}

const fn ioc(dir: u64, number: u8, size: usize) -> c_ulong {
    ((dir << 30) | ((size as u64) << 16) | ((b'A' as u64) << 8) | number as u64) as c_ulong
}
const fn readwrite<T>(number: u8) -> c_ulong {
    ioc(3, number, size_of::<T>())
}
const fn write<T>(number: u8) -> c_ulong {
    ioc(1, number, size_of::<T>())
}
const fn none(number: u8) -> c_ulong {
    ioc(0, number, 0)
}

fn call<T>(file: &File, request: c_ulong, data: &mut T) -> io::Result<()> {
    // SAFETY: data is an initialized repr(C) buffer and remains live for ioctl.
    let rc = unsafe { ioctl(file.as_raw_fd(), request, data as *mut T) };
    if rc < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}
fn command(file: &File, request: c_ulong) -> io::Result<()> {
    // SAFETY: zero-argument ALSA ioctls consume no user pointer.
    let rc = unsafe { ioctl(file.as_raw_fd(), request) };
    if rc < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}
fn as_output(operation: &'static str, error: io::Error) -> MusicError {
    MusicError::Output(format!("ALSA kernel {operation}: {error}"))
}

fn device_path(device: &str) -> Result<PathBuf> {
    // Reject ALSA plugin names (`default`, `plughw`, `dmix`) and raw filesystem
    // paths. Accept only an explicit, bounded hardware-card/device pair.
    let pair = device.strip_prefix("hw:").ok_or(MusicError::Unsupported(
        "native Linux output needs an explicit hw:CARD,DEVICE; ALSA plugins are not used",
    ))?;
    let (card, item) = pair
        .split_once(',')
        .ok_or(MusicError::Invalid("expected hw:CARD,DEVICE"))?;
    fn component(value: &str) -> Option<u16> {
        if value.is_empty() || value.len() > 3 || !value.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        value.parse::<u16>().ok().filter(|n| *n <= 255)
    }
    let card = component(card).ok_or(MusicError::Invalid("invalid ALSA card number"))?;
    let item = component(item).ok_or(MusicError::Invalid("invalid ALSA device number"))?;
    Ok(PathBuf::from(format!("/dev/snd/pcmC{card}D{item}p")))
}

/// Own direct ALSA kernel PCM adapter. Only `hw:CARD,DEVICE` is accepted.
/// `strict_hw` is retained for host compatibility; the direct backend is always strict.
pub struct AlsaSink {
    device: PathBuf,
    handle: Option<File>,
    format: Option<AudioFormat>,
    paused: bool,
}
impl AlsaSink {
    pub fn new(device: &str, _strict_hw: bool) -> Result<Self> {
        Ok(Self {
            device: device_path(device)?,
            handle: None,
            format: None,
            paused: false,
        })
    }
}
impl PcmSink for AlsaSink {
    fn open(&mut self, format: AudioFormat) -> Result<()> {
        format.validate()?;
        if self.handle.is_some() {
            return Err(MusicError::Invalid("ALSA output already open"));
        }
        if !matches!(format.bits_per_sample, 8 | 16 | 24 | 32) {
            return Err(MusicError::Unsupported(
                "Linux kernel ALSA supports packed 8/16/24/32-bit PCM only",
            ));
        }
        let file = OpenOptions::new().write(true).open(&self.device)?;
        let mut hw = HwParams::new(format);
        call(&file, readwrite::<HwParams>(0x10), &mut hw).map_err(|e| as_output("HW_REFINE", e))?;
        if !hw.exact_format(format) {
            return Err(MusicError::Unsupported(
                "hardware cannot negotiate source PCM format exactly",
            ));
        }
        call(&file, readwrite::<HwParams>(0x11), &mut hw).map_err(|e| as_output("HW_PARAMS", e))?;
        if !hw.exact_format(format) {
            return Err(MusicError::Unsupported(
                "hardware altered the requested PCM parameters",
            ));
        }
        let period = hw.intervals[PERIOD_SIZE].min as usize;
        let buffer = hw.intervals[BUFFER_SIZE].min as usize;
        if period == 0 || buffer < period || buffer > 65_536 {
            return Err(MusicError::Unsupported(
                "invalid hardware period/buffer negotiation",
            ));
        }
        let mut sw = SwParams::new(period, buffer);
        call(&file, readwrite::<SwParams>(0x13), &mut sw).map_err(|e| as_output("SW_PARAMS", e))?;
        command(&file, none(0x40)).map_err(|e| as_output("PREPARE", e))?;
        self.format = Some(format);
        self.paused = false;
        self.handle = Some(file);
        Ok(())
    }
    fn write(&mut self, block: &PcmBlock) -> Result<()> {
        block.validate()?;
        if self.paused {
            return Err(MusicError::Invalid("ALSA output is paused"));
        }
        if self.format != Some(block.format) {
            return Err(MusicError::Invalid(
                "Linux sink not open or PCM format changed",
            ));
        }
        let file = self
            .handle
            .as_ref()
            .ok_or(MusicError::Invalid("Linux sink is closed"))?;
        let width = usize::from(block.format.bits_per_sample / 8);
        let capacity = block
            .samples
            .len()
            .checked_mul(width)
            .ok_or(MusicError::Limit("output size overflow"))?;
        let mut packed = Vec::with_capacity(capacity);
        for &sample in &block.samples {
            match width {
                1 => packed.push((sample + 128) as u8),
                2 => packed.extend_from_slice(&(sample as i16).to_le_bytes()),
                3 => packed.extend_from_slice(&sample.to_le_bytes()[..3]),
                4 => packed.extend_from_slice(&sample.to_le_bytes()),
                _ => unreachable!(),
            }
        }
        let frame_bytes = width * usize::from(block.format.channels);
        let mut left = block.frames();
        let mut offset = 0usize;
        let mut recoveries = 0;
        while left != 0 {
            let mut transfer = Transfer {
                result: 0,
                buffer: packed[offset..].as_mut_ptr().cast::<c_void>(),
                frames: left,
            };
            match call(file, write::<Transfer>(0x50), &mut transfer) {
                Ok(()) if transfer.result > 0 => {
                    let frames = transfer.result as usize;
                    if frames > left {
                        return Err(MusicError::Invalid("ALSA reported excess frames"));
                    }
                    left -= frames;
                    offset += frames * frame_bytes;
                }
                Ok(()) if transfer.result == 0 => {
                    return Err(MusicError::Output("ALSA zero-progress write".to_owned()));
                }
                failed => {
                    let recoverable = match &failed {
                        Err(e) => e.raw_os_error() == Some(32), // Linux EPIPE (underrun)
                        Ok(()) => transfer.result == -32,
                    };
                    if !recoverable || recoveries >= 2 {
                        return Err(MusicError::Output(format!(
                            "ALSA direct PCM write failed: {:?}, frames={}",
                            failed.err(),
                            transfer.result
                        )));
                    }
                    command(file, none(0x40)).map_err(|e| as_output("underrun recovery", e))?;
                    recoveries += 1;
                }
            }
        }
        Ok(())
    }
    fn pause(&mut self) -> Result<()> {
        let file = self
            .handle
            .as_ref()
            .ok_or(MusicError::Invalid("ALSA sink is closed"))?;
        if self.paused {
            return Ok(());
        }
        let mut enabled: c_int = 1;
        call(file, write::<c_int>(0x45), &mut enabled).map_err(|e| as_output("PAUSE", e))?;
        self.paused = true;
        Ok(())
    }
    fn resume(&mut self) -> Result<()> {
        let file = self
            .handle
            .as_ref()
            .ok_or(MusicError::Invalid("ALSA sink is closed"))?;
        if !self.paused {
            return Err(MusicError::Invalid("ALSA sink is not paused"));
        }
        let mut enabled: c_int = 0;
        call(file, write::<c_int>(0x45), &mut enabled).map_err(|e| as_output("RESUME", e))?;
        self.paused = false;
        Ok(())
    }
    fn finish(&mut self) -> Result<()> {
        let was_paused = self.paused;
        let handle = self.handle.take();
        self.format = None;
        self.paused = false;
        if let Some(file) = handle {
            if !was_paused {
                command(&file, none(0x44)).map_err(|e| as_output("DRAIN", e))?;
            }
        }
        Ok(())
    }
}
impl Drop for AlsaSink {
    fn drop(&mut self) {
        let _ = self.finish();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn kernel_struct_sizes_and_requests_match_uapi_64bit() {
        assert_eq!(size_of::<Mask>(), 32);
        assert_eq!(size_of::<Interval>(), 12);
        assert_eq!(size_of::<HwParams>(), 608);
        assert_eq!(size_of::<SwParams>(), 136);
        assert_eq!(size_of::<Transfer>(), 24);
        assert_eq!(readwrite::<HwParams>(0x10) as u64, 0xc2604110);
        assert_eq!(readwrite::<SwParams>(0x13) as u64, 0xc0884113);
        assert_eq!(write::<Transfer>(0x50) as u64, 0x40184150);
        assert_eq!(write::<c_int>(0x45) as u64, 0x40044145);
    }
    #[test]
    fn accepts_only_direct_bounded_pcm_devices() {
        assert_eq!(
            device_path("hw:2,1").unwrap().to_str(),
            Some("/dev/snd/pcmC2D1p")
        );
        for wrong in [
            "default",
            "plughw:0,0",
            "dmix",
            "hw:0",
            "hw:0,0/../x",
            "hw:a,0",
            "hw:256,0",
            "hw:0,0,1",
        ] {
            assert!(device_path(wrong).is_err(), "{wrong}");
        }
    }
}
