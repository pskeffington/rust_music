//! Own macOS AudioQueue PCM bridge. Audio callback flips preallocated slot flags
//! only; disk reading/decoding stays on the application's independent worker.
use rust_music_core::{AudioFormat, MusicError, PcmBlock, PcmSink, Result};
use std::{
    ffi::c_void,
    ptr,
    sync::atomic::{AtomicBool, Ordering},
    thread,
    time::{Duration, Instant},
};

const SLOTS: usize = 3;
const FRAMES_PER_SLOT: usize = 4096;
const MAX_WAIT: Duration = Duration::from_secs(5);
const FORMAT_LPCM: u32 = u32::from_be_bytes(*b"lpcm");
const FLAG_SIGNED: u32 = 1 << 2;
const FLAG_PACKED: u32 = 1 << 3;

#[repr(C)]
struct AudioStreamBasicDescription {
    sample_rate: f64,
    format_id: u32,
    format_flags: u32,
    bytes_per_packet: u32,
    frames_per_packet: u32,
    bytes_per_frame: u32,
    channels_per_frame: u32,
    bits_per_channel: u32,
    reserved: u32,
}
#[repr(C)]
struct AudioQueueBuffer {
    data_capacity: u32,
    data: *mut c_void,
    data_size: u32,
    user_data: *mut c_void,
    packet_description_capacity: u32,
    packet_descriptions: *mut c_void,
    packet_description_count: u32,
}
type AudioQueueRef = *mut c_void;
type AudioQueueBufferRef = *mut AudioQueueBuffer;
#[link(name = "AudioToolbox", kind = "framework")]
unsafe extern "C" {
    fn AudioQueueNewOutput(
        format: *const AudioStreamBasicDescription,
        callback: unsafe extern "C" fn(*mut c_void, AudioQueueRef, AudioQueueBufferRef),
        user_data: *mut c_void,
        run_loop: *mut c_void,
        loop_mode: *mut c_void,
        flags: u32,
        out_queue: *mut AudioQueueRef,
    ) -> i32;
    fn AudioQueueAllocateBuffer(
        queue: AudioQueueRef,
        bytes: u32,
        out: *mut AudioQueueBufferRef,
    ) -> i32;
    fn AudioQueueEnqueueBuffer(
        queue: AudioQueueRef,
        buffer: AudioQueueBufferRef,
        packet_count: u32,
        descriptions: *const c_void,
    ) -> i32;
    fn AudioQueueStart(queue: AudioQueueRef, start: *const c_void) -> i32;
    fn AudioQueueStop(queue: AudioQueueRef, immediate: u8) -> i32;
    fn AudioQueueDispose(queue: AudioQueueRef, immediate: u8) -> i32;
}
struct CallbackContext {
    slots: [AudioQueueBufferRef; SLOTS],
    in_flight: [AtomicBool; SLOTS],
}
impl CallbackContext {
    fn new() -> Self {
        Self {
            slots: [ptr::null_mut(); SLOTS],
            in_flight: std::array::from_fn(|_| AtomicBool::new(false)),
        }
    }
}
unsafe extern "C" fn completed(
    user: *mut c_void,
    _queue: AudioQueueRef,
    buffer: AudioQueueBufferRef,
) {
    if user.is_null() {
        return;
    }
    let state = unsafe { &*(user as *const CallbackContext) };
    for index in 0..SLOTS {
        if state.slots[index] == buffer {
            state.in_flight[index].store(false, Ordering::Release);
            return;
        }
    }
}

/// Device and stream negotiation is performed by macOS. Not yet certified as
/// hardware bit-perfect; host must distinguish that from decoder accuracy.
pub struct MacAudioQueueSink {
    queue: AudioQueueRef,
    context: Option<Box<CallbackContext>>,
    format: Option<AudioFormat>,
    started: bool,
}
impl Default for MacAudioQueueSink {
    fn default() -> Self {
        Self {
            queue: ptr::null_mut(),
            context: None,
            format: None,
            started: false,
        }
    }
}
impl PcmSink for MacAudioQueueSink {
    fn open(&mut self, format: AudioFormat) -> Result<()> {
        if !self.queue.is_null() {
            return Err(MusicError::Invalid("AudioQueue already open"));
        }
        if !matches!(format.bits_per_sample, 8 | 16 | 24 | 32) {
            return Err(MusicError::Unsupported(
                "AudioQueue needs packed 8/16/24/32-bit PCM",
            ));
        }
        let bytes_per_frame = u32::from(format.channels) * u32::from(format.bits_per_sample / 8);
        let desc = AudioStreamBasicDescription {
            sample_rate: f64::from(format.sample_rate),
            format_id: FORMAT_LPCM,
            format_flags: FLAG_PACKED
                | if format.bits_per_sample == 8 {
                    0
                } else {
                    FLAG_SIGNED
                },
            bytes_per_packet: bytes_per_frame,
            frames_per_packet: 1,
            bytes_per_frame,
            channels_per_frame: u32::from(format.channels),
            bits_per_channel: u32::from(format.bits_per_sample),
            reserved: 0,
        };
        let mut context = Box::new(CallbackContext::new());
        let mut queue = ptr::null_mut();
        let err = unsafe {
            AudioQueueNewOutput(
                &desc,
                completed,
                (&mut *context as *mut CallbackContext).cast(),
                ptr::null_mut(),
                ptr::null_mut(),
                0,
                &mut queue,
            )
        };
        if err != 0 {
            return Err(MusicError::Output(format!(
                "AudioQueueNewOutput OSStatus {err}"
            )));
        }
        for slot in &mut context.slots {
            let err = unsafe {
                AudioQueueAllocateBuffer(queue, FRAMES_PER_SLOT as u32 * bytes_per_frame, slot)
            };
            if err != 0 {
                unsafe {
                    AudioQueueDispose(queue, 1);
                }
                return Err(MusicError::Output(format!(
                    "AudioQueueAllocateBuffer OSStatus {err}"
                )));
            }
        }
        self.queue = queue;
        self.context = Some(context);
        self.format = Some(format);
        self.started = false;
        Ok(())
    }
    fn write(&mut self, block: &PcmBlock) -> Result<()> {
        if self.queue.is_null() || self.format != Some(block.format) {
            return Err(MusicError::Invalid(
                "AudioQueue not open or source format changed",
            ));
        }
        let context = self
            .context
            .as_ref()
            .ok_or(MusicError::Invalid("missing AudioQueue callback"))?;
        let n = usize::from(block.format.bits_per_sample / 8);
        let channels = usize::from(block.format.channels);
        for samples in block.samples.chunks(FRAMES_PER_SLOT * channels) {
            let deadline = Instant::now() + MAX_WAIT;
            let index = loop {
                if let Some(i) = (0..SLOTS).find(|&i| !context.in_flight[i].load(Ordering::Acquire))
                {
                    break i;
                }
                if Instant::now() >= deadline {
                    return Err(MusicError::Output("AudioQueue buffer timed out".into()));
                }
                thread::sleep(Duration::from_millis(2));
            };
            let buffer = context.slots[index];
            if buffer.is_null() {
                return Err(MusicError::Invalid("AudioQueue buffer missing"));
            }
            let bytes = samples.len() * n;
            let output =
                unsafe { std::slice::from_raw_parts_mut((*buffer).data.cast::<u8>(), bytes) };
            for (sample, target) in samples.iter().zip(output.chunks_exact_mut(n)) {
                let bytes = sample.to_le_bytes();
                if n == 1 {
                    target[0] = (*sample + 128) as u8;
                } else {
                    target.copy_from_slice(&bytes[..n]);
                }
            }
            unsafe {
                (*buffer).data_size = bytes as u32;
            }
            context.in_flight[index].store(true, Ordering::Release);
            let err = unsafe { AudioQueueEnqueueBuffer(self.queue, buffer, 0, ptr::null()) };
            if err != 0 {
                context.in_flight[index].store(false, Ordering::Release);
                return Err(MusicError::Output(format!(
                    "AudioQueueEnqueueBuffer OSStatus {err}"
                )));
            }
            if !self.started {
                let err = unsafe { AudioQueueStart(self.queue, ptr::null()) };
                if err != 0 {
                    return Err(MusicError::Output(format!(
                        "AudioQueueStart OSStatus {err}"
                    )));
                }
                self.started = true;
            }
        }
        Ok(())
    }
    fn finish(&mut self) -> Result<()> {
        if self.queue.is_null() {
            return Ok(());
        }
        let queue = std::mem::replace(&mut self.queue, ptr::null_mut());
        let had_audio = std::mem::replace(&mut self.started, false);
        self.format = None;
        // non-immediate drain plays buffered PCM before disposal, if any.
        let stop = unsafe { AudioQueueStop(queue, if had_audio { 0 } else { 1 }) };
        let dispose = unsafe { AudioQueueDispose(queue, if had_audio { 0 } else { 1 }) };
        self.context.take();
        if stop != 0 || dispose != 0 {
            return Err(MusicError::Output(format!(
                "AudioQueue stop={stop} dispose={dispose}"
            )));
        }
        Ok(())
    }
}
impl Drop for MacAudioQueueSink {
    fn drop(&mut self) {
        let _ = self.finish();
    }
}
