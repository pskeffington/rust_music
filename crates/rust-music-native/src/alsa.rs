//! Linux libasound PCM ABI bridge, independent Rust configuration and sample packing.
//! The caller must select `hw:` explicitly for a candidate direct, non-mixed path.
use rust_music_core::{AudioFormat, MusicError, PcmBlock, PcmSink, Result};
use std::{ffi::{c_char, c_int, c_void, CString}, ptr};

#[repr(C)] struct SndPcm { _private: [u8; 0] }
#[link(name = "asound")]
unsafe extern "C" {
    fn snd_pcm_open(out: *mut *mut SndPcm, device: *const c_char, stream: c_int, mode: c_int) -> c_int;
    fn snd_pcm_close(pcm: *mut SndPcm) -> c_int;
    fn snd_pcm_set_params(pcm: *mut SndPcm, format: c_int, access: c_int, channels: u32,
                          rate: u32, soft_resample: c_int, latency: u32) -> c_int;
    fn snd_pcm_format_value(name: *const c_char) -> c_int;
    fn snd_pcm_writei(pcm: *mut SndPcm, buffer: *const c_void, size: usize) -> isize;
    fn snd_pcm_recover(pcm: *mut SndPcm, error: c_int, silent: c_int) -> c_int;
    fn snd_pcm_drain(pcm: *mut SndPcm) -> c_int;
}

/// The host selects the ALSA device. `strict_hw` requires a direct `hw:` path.
pub struct AlsaSink {
    device: CString,
    strict_hw: bool,
    handle: *mut SndPcm,
    format: Option<AudioFormat>,
}
impl AlsaSink {
    pub fn new(device: &str, strict_hw: bool) -> Result<Self> {
        if strict_hw && !device.starts_with("hw:") {
            return Err(MusicError::Unsupported("strict ALSA mode requires direct hw: device"));
        }
        let device = CString::new(device).map_err(|_| MusicError::Invalid("NUL in ALSA device"))?;
        Ok(Self { device, strict_hw, handle: ptr::null_mut(), format: None })
    }
}
impl PcmSink for AlsaSink {
    fn open(&mut self, format: AudioFormat) -> Result<()> {
        if !self.handle.is_null() { return Err(MusicError::Invalid("ALSA output already open")); }
        let name = match format.bits_per_sample {
            8 => c"U8", 16 => c"S16_LE", 24 => c"S24_3LE", 32 => c"S32_LE",
            _ => return Err(MusicError::Unsupported("ALSA output needs 8/16/24/32 packed PCM")),
        };
        let format_id = unsafe { snd_pcm_format_value(name.as_ptr()) };
        if format_id < 0 { return Err(MusicError::Unsupported("ALSA sample format unavailable")); }
        let mut handle: *mut SndPcm = ptr::null_mut();
        let err = unsafe { snd_pcm_open(&mut handle, self.device.as_ptr(), 0, 0) };
        if err < 0 { return Err(MusicError::Output(format!("ALSA open error {err}"))); }
        let err = unsafe { snd_pcm_set_params(handle, format_id, 3, u32::from(format.channels),
            format.sample_rate, 0, if self.strict_hw { 90_000 } else { 150_000 }) };
        if err < 0 {
            unsafe { snd_pcm_close(handle); }
            return Err(MusicError::Output(format!("ALSA exact sample format negotiation failed: {err}")));
        }
        self.handle = handle;
        self.format = Some(format);
        Ok(())
    }
    fn write(&mut self, block: &PcmBlock) -> Result<()> {
        if self.handle.is_null() || self.format != Some(block.format) {
            return Err(MusicError::Invalid("ALSA sink not open or track format changed"));
        }
        let n = usize::from(block.format.bits_per_sample / 8);
        let mut packed = Vec::with_capacity(n * block.samples.len());
        for &s in &block.samples {
            match n {
                1 => packed.push((s + 128) as u8),
                2 => packed.extend_from_slice(&(s as i16).to_le_bytes()),
                3 => packed.extend_from_slice(&s.to_le_bytes()[..3]),
                4 => packed.extend_from_slice(&s.to_le_bytes()),
                _ => unreachable!(),
            }
        }
        let bytes_per_frame = n * usize::from(block.format.channels);
        let mut remaining = block.frames();
        let mut byte_offset = 0;
        let mut recoveries = 0;
        while remaining > 0 {
            let written = unsafe {
                snd_pcm_writei(self.handle, packed[byte_offset..].as_ptr().cast(), remaining)
            };
            if written < 0 {
                if recoveries >= 2 { return Err(MusicError::Output(format!("ALSA write error {written}"))); }
                let recovery = unsafe { snd_pcm_recover(self.handle, written as c_int, 1) };
                if recovery < 0 { return Err(MusicError::Output(format!("ALSA recovery error {recovery}"))); }
                recoveries += 1;
                continue;
            }
            if written == 0 { return Err(MusicError::Output("ALSA zero-progress write".to_owned())); }
            let frames = written as usize;
            if frames > remaining { return Err(MusicError::Invalid("ALSA wrote more frames than requested")); }
            remaining -= frames;
            byte_offset += frames * bytes_per_frame;
        }
        Ok(())
    }
    fn finish(&mut self) -> Result<()> {
        if self.handle.is_null() { return Ok(()); }
        let handle = std::mem::replace(&mut self.handle, ptr::null_mut());
        self.format = None;
        let drain = unsafe { snd_pcm_drain(handle) };
        let close = unsafe { snd_pcm_close(handle) };
        if drain < 0 || close < 0 {
            return Err(MusicError::Output(format!("ALSA drain={drain} close={close}")));
        }
        Ok(())
    }
}
impl Drop for AlsaSink {
    fn drop(&mut self) { let _ = self.finish(); }
}
