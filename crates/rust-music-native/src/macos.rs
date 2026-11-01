//! Own macOS AudioQueue PCM bridge. Audio callback flips preallocated slot flags
//! only; disk reading/decoding stays on the application's independent worker.
use rust_music_core::{AudioFormat, MusicError, PcmBlock, PcmSink, Result};
use std::{
    ffi::c_void,
    ffi::OsStr,
    ptr,
    sync::{
        atomic::AtomicUsize,
        atomic::{AtomicBool, AtomicI64, AtomicU64, Ordering},
        Arc,
    },
    thread,
    time::{Duration, Instant},
};

const SLOTS: usize = 3;
const MAX_TRACKED_QUEUES: usize = 32;
const FRAMES_PER_SLOT: usize = 4096;
const MAX_WAIT: Duration = Duration::from_secs(5);
const MAX_DRAIN_WAIT: Duration = Duration::from_secs(5);
const FORMAT_LPCM: u32 = u32::from_be_bytes(*b"lpcm");
const QUEUE_PROPERTY_CURRENT_DEVICE: u32 = u32::from_be_bytes(*b"aqcd");
const AUDIO_HARDWARE_PROPERTY_DEFAULT_OUTPUT_DEVICE: u32 = u32::from_be_bytes(*b"dOut");
const AUDIO_DEVICE_PROPERTY_DEVICE_UID: u32 = u32::from_be_bytes(*b"uid ");
const AUDIO_OBJECT_PROPERTY_SCOPE_GLOBAL: u32 = u32::from_be_bytes(*b"glob");
const AUDIO_OBJECT_SYSTEM_OBJECT_ID: u32 = 1;
const AUDIOQUEUE_ERR_INVALID_DEVICE: i32 = -66_680;
const FLAG_SIGNED: u32 = 1 << 2;
const FLAG_PACKED: u32 = 1 << 3;
const NO_STATUS: i64 = i64::MIN;

/// Bounded, opt-in observations from the native AudioQueue boundary.
/// Counters saturate at `u64::MAX`; status fields contain the most recent OSStatus.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct AudioQueueDiagnosticsSnapshot {
    pub enabled: bool,
    pub sample_rate: u32,
    pub bits_per_sample: u8,
    pub channels: u8,
    pub output_creation_attempts: u64,
    pub output_creation_status: Option<i32>,
    pub default_route_status: Option<i32>,
    pub allocated_buffers: u64,
    pub allocated_capacity_bytes: u64,
    pub buffer_allocation_status: Option<i32>,
    pub enqueued_buffers: u64,
    pub enqueued_frames: u64,
    pub start_attempts: u64,
    pub start_status: Option<i32>,
    pub current_device_status: Option<i32>,
    pub current_device_associated: Option<bool>,
    pub current_device_matches_default: Option<bool>,
    pub completed_callbacks: u64,
    pub teardown_attempts: u64,
    pub stop_status: Option<i32>,
    pub drain_complete: Option<bool>,
    pub immediate_stop_status: Option<i32>,
    pub dispose_status: Option<i32>,
}

struct AudioQueueDiagnostics {
    enabled: AtomicBool,
    sample_rate: AtomicU64,
    bits_per_sample: AtomicU64,
    channels: AtomicU64,
    output_creation_attempts: AtomicU64,
    output_creation_status: AtomicI64,
    default_route_status: AtomicI64,
    allocated_buffers: AtomicU64,
    allocated_capacity_bytes: AtomicU64,
    buffer_allocation_status: AtomicI64,
    enqueued_buffers: AtomicU64,
    enqueued_frames: AtomicU64,
    start_attempts: AtomicU64,
    start_status: AtomicI64,
    current_device_status: AtomicI64,
    current_device_associated: AtomicI64,
    current_device_matches_default: AtomicI64,
    completed_callbacks: AtomicU64,
    teardown_attempts: AtomicU64,
    stop_status: AtomicI64,
    drain_complete: AtomicI64,
    immediate_stop_status: AtomicI64,
    dispose_status: AtomicI64,
}

impl Default for AudioQueueDiagnostics {
    fn default() -> Self {
        Self {
            enabled: AtomicBool::new(false),
            sample_rate: AtomicU64::new(0),
            bits_per_sample: AtomicU64::new(0),
            channels: AtomicU64::new(0),
            output_creation_attempts: AtomicU64::new(0),
            output_creation_status: AtomicI64::new(NO_STATUS),
            default_route_status: AtomicI64::new(NO_STATUS),
            allocated_buffers: AtomicU64::new(0),
            allocated_capacity_bytes: AtomicU64::new(0),
            buffer_allocation_status: AtomicI64::new(NO_STATUS),
            enqueued_buffers: AtomicU64::new(0),
            enqueued_frames: AtomicU64::new(0),
            start_attempts: AtomicU64::new(0),
            start_status: AtomicI64::new(NO_STATUS),
            current_device_status: AtomicI64::new(NO_STATUS),
            current_device_associated: AtomicI64::new(NO_STATUS),
            current_device_matches_default: AtomicI64::new(NO_STATUS),
            completed_callbacks: AtomicU64::new(0),
            teardown_attempts: AtomicU64::new(0),
            stop_status: AtomicI64::new(NO_STATUS),
            drain_complete: AtomicI64::new(0),
            immediate_stop_status: AtomicI64::new(NO_STATUS),
            dispose_status: AtomicI64::new(NO_STATUS),
        }
    }
}

impl AudioQueueDiagnostics {
    fn status(&self, field: &AtomicI64, value: i32) {
        if self.enabled.load(Ordering::Relaxed) {
            field.store(i64::from(value), Ordering::Relaxed);
        }
    }

    fn add(&self, field: &AtomicU64, value: u64) {
        if !self.enabled.load(Ordering::Relaxed) {
            return;
        }
        let mut current = field.load(Ordering::Relaxed);
        loop {
            let next = current.saturating_add(value);
            match field.compare_exchange_weak(current, next, Ordering::Relaxed, Ordering::Relaxed) {
                Ok(_) => break,
                Err(observed) => current = observed,
            }
        }
    }

    fn snapshot(&self) -> AudioQueueDiagnosticsSnapshot {
        let status = |field: &AtomicI64| match field.load(Ordering::Relaxed) {
            NO_STATUS => None,
            value => i32::try_from(value).ok(),
        };
        let drain_complete = match self.drain_complete.load(Ordering::Relaxed) {
            0 => None,
            1 => Some(false),
            _ => Some(true),
        };
        AudioQueueDiagnosticsSnapshot {
            enabled: self.enabled.load(Ordering::Relaxed),
            sample_rate: self.sample_rate.load(Ordering::Relaxed) as u32,
            bits_per_sample: self.bits_per_sample.load(Ordering::Relaxed) as u8,
            channels: self.channels.load(Ordering::Relaxed) as u8,
            output_creation_attempts: self.output_creation_attempts.load(Ordering::Relaxed),
            output_creation_status: status(&self.output_creation_status),
            default_route_status: status(&self.default_route_status),
            allocated_buffers: self.allocated_buffers.load(Ordering::Relaxed),
            allocated_capacity_bytes: self.allocated_capacity_bytes.load(Ordering::Relaxed),
            buffer_allocation_status: status(&self.buffer_allocation_status),
            enqueued_buffers: self.enqueued_buffers.load(Ordering::Relaxed),
            enqueued_frames: self.enqueued_frames.load(Ordering::Relaxed),
            start_attempts: self.start_attempts.load(Ordering::Relaxed),
            start_status: status(&self.start_status),
            current_device_status: status(&self.current_device_status),
            current_device_associated: match self.current_device_associated.load(Ordering::Relaxed)
            {
                0 => Some(false),
                1 => Some(true),
                _ => None,
            },
            current_device_matches_default: match self
                .current_device_matches_default
                .load(Ordering::Relaxed)
            {
                0 => Some(false),
                1 => Some(true),
                _ => None,
            },
            completed_callbacks: self.completed_callbacks.load(Ordering::Relaxed),
            teardown_attempts: self.teardown_attempts.load(Ordering::Relaxed),
            stop_status: status(&self.stop_status),
            drain_complete,
            immediate_stop_status: status(&self.immediate_stop_status),
            dispose_status: status(&self.dispose_status),
        }
    }

    fn set_drain_complete(&self, complete: bool) {
        if self.enabled.load(Ordering::Relaxed) {
            self.drain_complete
                .store(if complete { 2 } else { 1 }, Ordering::Relaxed);
        }
    }

    fn set_current_device(&self, status: i32, associated: bool, matches_default: Option<bool>) {
        if self.enabled.load(Ordering::Relaxed) {
            self.current_device_status
                .store(i64::from(status), Ordering::Relaxed);
            self.current_device_associated
                .store(i64::from(associated), Ordering::Relaxed);
            self.current_device_matches_default.store(
                matches_default.map_or(NO_STATUS, i64::from),
                Ordering::Relaxed,
            );
        }
    }

    fn set_format(&self, format: AudioFormat) {
        self.sample_rate
            .store(u64::from(format.sample_rate), Ordering::Relaxed);
        self.bits_per_sample
            .store(u64::from(format.bits_per_sample), Ordering::Relaxed);
        self.channels
            .store(u64::from(format.channels), Ordering::Relaxed);
    }

    fn record(&self) -> String {
        let snapshot = self.snapshot();
        format!(
            "RUST_MUSIC_AUDIO_DIAG rate={} bits={} channels={} created={} create_status={} route_status={} allocated={} capacity_bytes={} allocation_status={} enqueued={} frames={} starts={} start_status={} device_status={} device_associated={} device_default={} callbacks={} teardown={} stop_status={} drained={} immediate_stop_status={} dispose_status={}",
            snapshot.sample_rate,
            snapshot.bits_per_sample,
            snapshot.channels,
            snapshot.output_creation_attempts,
            format_status(snapshot.output_creation_status),
            format_status(snapshot.default_route_status),
            snapshot.allocated_buffers,
            snapshot.allocated_capacity_bytes,
            format_status(snapshot.buffer_allocation_status),
            snapshot.enqueued_buffers,
            snapshot.enqueued_frames,
            snapshot.start_attempts,
            format_status(snapshot.start_status),
            format_status(snapshot.current_device_status),
            format_bool(snapshot.current_device_associated),
            format_bool(snapshot.current_device_matches_default),
            snapshot.completed_callbacks,
            snapshot.teardown_attempts,
            format_status(snapshot.stop_status),
            format_drain(snapshot.drain_complete),
            format_status(snapshot.immediate_stop_status),
            format_status(snapshot.dispose_status),
        )
    }
}

fn diagnostics_enabled_from_value(value: Option<&OsStr>) -> bool {
    value.is_some_and(|value| value == "1")
}

fn format_status(status: Option<i32>) -> String {
    status.map_or_else(|| "none".to_owned(), |value| value.to_string())
}

fn format_drain(drained: Option<bool>) -> &'static str {
    match drained {
        Some(false) => "0",
        Some(true) => "1",
        None => "none",
    }
}

fn format_bool(value: Option<bool>) -> &'static str {
    match value {
        Some(false) => "0",
        Some(true) => "1",
        None => "none",
    }
}

#[repr(C)]
struct AudioObjectPropertyAddress {
    selector: u32,
    scope: u32,
    element: u32,
}

#[link(name = "CoreAudio", kind = "framework")]
unsafe extern "C" {
    fn AudioObjectGetPropertyData(
        object_id: u32,
        address: *const AudioObjectPropertyAddress,
        qualifier_data_size: u32,
        qualifier_data: *const c_void,
        data_size: *mut u32,
        out_data: *mut c_void,
    ) -> i32;
}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFEqual(left: *const c_void, right: *const c_void) -> i8;
}

fn default_output_device_uid() -> std::result::Result<*const c_void, i32> {
    let default_output_address = AudioObjectPropertyAddress {
        selector: AUDIO_HARDWARE_PROPERTY_DEFAULT_OUTPUT_DEVICE,
        scope: AUDIO_OBJECT_PROPERTY_SCOPE_GLOBAL,
        element: 0,
    };
    let mut default_device_id = 0_u32;
    let mut size = std::mem::size_of_val(&default_device_id) as u32;
    let status = unsafe {
        AudioObjectGetPropertyData(
            AUDIO_OBJECT_SYSTEM_OBJECT_ID,
            &default_output_address,
            0,
            ptr::null(),
            &mut size,
            (&mut default_device_id as *mut u32).cast(),
        )
    };
    if status != 0 {
        return Err(status);
    }
    if default_device_id == 0 {
        return Err(AUDIOQUEUE_ERR_INVALID_DEVICE);
    }

    let uid_address = AudioObjectPropertyAddress {
        selector: AUDIO_DEVICE_PROPERTY_DEVICE_UID,
        scope: AUDIO_OBJECT_PROPERTY_SCOPE_GLOBAL,
        element: 0,
    };
    let mut default_uid: *const c_void = ptr::null();
    let mut size = std::mem::size_of_val(&default_uid) as u32;
    let status = unsafe {
        AudioObjectGetPropertyData(
            default_device_id,
            &uid_address,
            0,
            ptr::null(),
            &mut size,
            (&mut default_uid as *mut *const c_void).cast(),
        )
    };
    if status != 0 {
        return Err(status);
    }
    if default_uid.is_null() {
        return Err(AUDIOQUEUE_ERR_INVALID_DEVICE);
    }
    Ok(default_uid)
}

fn current_device_matches_default(device_uid: *const c_void) -> Option<bool> {
    if device_uid.is_null() {
        return None;
    }
    let default_uid = default_output_device_uid().ok()?;
    Some(unsafe { CFEqual(device_uid, default_uid) != 0 })
}

fn record_start(
    api: &dyn AudioQueueApi,
    queue: AudioQueueRef,
    diagnostics: &AudioQueueDiagnostics,
) -> i32 {
    diagnostics.add(&diagnostics.start_attempts, 1);
    let status = api.start(queue);
    diagnostics.status(&diagnostics.start_status, status);
    let (device_status, associated, matches_default) = api.current_device(queue);
    diagnostics.set_current_device(device_status, associated, matches_default);
    status
}

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
    fn AudioQueueGetProperty(
        queue: AudioQueueRef,
        property_id: u32,
        out_data: *mut c_void,
        io_data_size: *mut u32,
    ) -> i32;
    fn AudioQueueSetProperty(
        queue: AudioQueueRef,
        property_id: u32,
        property_data: *const c_void,
        property_data_size: u32,
    ) -> i32;
    fn AudioQueuePause(queue: AudioQueueRef) -> i32;
    fn AudioQueueStop(queue: AudioQueueRef, immediate: u8) -> i32;
    fn AudioQueueDispose(queue: AudioQueueRef, immediate: u8) -> i32;
}

trait AudioQueueApi: Send + Sync {
    fn new_output(
        &self,
        format: *const AudioStreamBasicDescription,
        callback: unsafe extern "C" fn(*mut c_void, AudioQueueRef, AudioQueueBufferRef),
        user_data: *mut c_void,
        out_queue: *mut AudioQueueRef,
    ) -> i32;
    fn route_to_default_output(&self, queue: AudioQueueRef) -> i32;
    fn allocate_buffer(
        &self,
        queue: AudioQueueRef,
        bytes: u32,
        out: *mut AudioQueueBufferRef,
    ) -> i32;
    fn enqueue_buffer(&self, queue: AudioQueueRef, buffer: AudioQueueBufferRef) -> i32;
    fn start(&self, queue: AudioQueueRef) -> i32;
    fn current_device(&self, queue: AudioQueueRef) -> (i32, bool, Option<bool>);
    fn pause(&self, queue: AudioQueueRef) -> i32;
    fn stop(&self, queue: AudioQueueRef, immediate: u8) -> i32;
    fn dispose(&self, queue: AudioQueueRef, immediate: u8) -> i32;
}

struct SystemAudioQueue;

impl AudioQueueApi for SystemAudioQueue {
    fn new_output(
        &self,
        format: *const AudioStreamBasicDescription,
        callback: unsafe extern "C" fn(*mut c_void, AudioQueueRef, AudioQueueBufferRef),
        user_data: *mut c_void,
        out_queue: *mut AudioQueueRef,
    ) -> i32 {
        unsafe {
            AudioQueueNewOutput(
                format,
                callback,
                user_data,
                ptr::null_mut(),
                ptr::null_mut(),
                0,
                out_queue,
            )
        }
    }

    fn route_to_default_output(&self, queue: AudioQueueRef) -> i32 {
        let default_uid = match default_output_device_uid() {
            Ok(uid) => uid,
            Err(status) => return status,
        };
        unsafe {
            AudioQueueSetProperty(
                queue,
                QUEUE_PROPERTY_CURRENT_DEVICE,
                (&default_uid as *const *const c_void).cast(),
                std::mem::size_of_val(&default_uid) as u32,
            )
        }
    }

    fn allocate_buffer(
        &self,
        queue: AudioQueueRef,
        bytes: u32,
        out: *mut AudioQueueBufferRef,
    ) -> i32 {
        unsafe { AudioQueueAllocateBuffer(queue, bytes, out) }
    }

    fn enqueue_buffer(&self, queue: AudioQueueRef, buffer: AudioQueueBufferRef) -> i32 {
        unsafe { AudioQueueEnqueueBuffer(queue, buffer, 0, ptr::null()) }
    }

    fn start(&self, queue: AudioQueueRef) -> i32 {
        unsafe { AudioQueueStart(queue, ptr::null()) }
    }

    fn current_device(&self, queue: AudioQueueRef) -> (i32, bool, Option<bool>) {
        let mut device_uid: *const c_void = ptr::null();
        let mut size = std::mem::size_of_val(&device_uid) as u32;
        let status = unsafe {
            AudioQueueGetProperty(
                queue,
                QUEUE_PROPERTY_CURRENT_DEVICE,
                (&mut device_uid as *mut *const c_void).cast(),
                &mut size,
            )
        };
        (
            status,
            !device_uid.is_null(),
            if status == 0 {
                current_device_matches_default(device_uid)
            } else {
                None
            },
        )
    }

    fn pause(&self, queue: AudioQueueRef) -> i32 {
        unsafe { AudioQueuePause(queue) }
    }

    fn stop(&self, queue: AudioQueueRef, immediate: u8) -> i32 {
        unsafe { AudioQueueStop(queue, immediate) }
    }

    fn dispose(&self, queue: AudioQueueRef, immediate: u8) -> i32 {
        unsafe { AudioQueueDispose(queue, immediate) }
    }
}

struct CallbackContext {
    slots: [AudioQueueBufferRef; SLOTS],
    in_flight: [AtomicBool; SLOTS],
    active_callbacks: AtomicUsize,
    diagnostics: Arc<AudioQueueDiagnostics>,
}
// The context is initialized before AudioQueue creation, then its slot
// pointers remain immutable; callback-visible state uses atomics. Ownership
// may move into the process registry after a failed dispose.
unsafe impl Send for CallbackContext {}

impl CallbackContext {
    fn new(diagnostics: Arc<AudioQueueDiagnostics>) -> Self {
        Self {
            slots: [ptr::null_mut(); SLOTS],
            in_flight: std::array::from_fn(|_| AtomicBool::new(false)),
            active_callbacks: AtomicUsize::new(0),
            diagnostics,
        }
    }
}

struct OrphanedQueue {
    api: Arc<dyn AudioQueueApi>,
    queue: AudioQueueRef,
    _context: Box<CallbackContext>,
}

// AudioQueueRef is only passed back to the same API while this record is under
// the registry mutex, and the callback context remains owned with it.
unsafe impl Send for OrphanedQueue {}

struct QueueRegistry {
    active: usize,
    pending_teardowns: usize,
    orphans: Vec<OrphanedQueue>,
}

static QUEUE_REGISTRY: std::sync::Mutex<QueueRegistry> = std::sync::Mutex::new(QueueRegistry {
    active: 0,
    pending_teardowns: 0,
    orphans: Vec::new(),
});

fn registry_lock() -> std::sync::MutexGuard<'static, QueueRegistry> {
    QUEUE_REGISTRY
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn reap_orphans(registry: &mut QueueRegistry) {
    let mut index = 0;
    while index < registry.orphans.len() {
        let orphan = &registry.orphans[index];
        let _ = orphan.api.stop(orphan.queue, 1);
        if orphan.api.dispose(orphan.queue, 1) == 0 {
            registry.orphans.remove(index);
        } else {
            index += 1;
        }
    }
}

fn reserve_queue() -> Result<()> {
    let mut registry = registry_lock();
    reap_orphans(&mut registry);
    if registry.pending_teardowns != 0 || !registry.orphans.is_empty() {
        return Err(MusicError::Output(
            "prior AudioQueue teardown is still pending".into(),
        ));
    }
    if registry.active >= MAX_TRACKED_QUEUES {
        return Err(MusicError::Limit("too many active AudioQueues"));
    }
    registry.active += 1;
    Ok(())
}

fn release_queue(pending_teardown: bool) {
    let mut registry = registry_lock();
    registry.active = registry.active.saturating_sub(1);
    if pending_teardown {
        registry.pending_teardowns = registry.pending_teardowns.saturating_sub(1);
    }
}

fn mark_pending_teardown() {
    registry_lock().pending_teardowns += 1;
}

fn clear_pending_teardown() {
    let mut registry = registry_lock();
    registry.pending_teardowns = registry.pending_teardowns.saturating_sub(1);
}

fn wait_for_drain(context: &CallbackContext, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    loop {
        let buffers_idle = context
            .in_flight
            .iter()
            .all(|slot| !slot.load(Ordering::Acquire));
        let callbacks_idle = context.active_callbacks.load(Ordering::Acquire) == 0;
        if buffers_idle && callbacks_idle {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        thread::sleep(Duration::from_millis(2));
    }
}

struct ActiveCallback<'a>(&'a AtomicUsize);

impl Drop for ActiveCallback<'_> {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::Release);
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
    state.active_callbacks.fetch_add(1, Ordering::AcqRel);
    let _active = ActiveCallback(&state.active_callbacks);
    for index in 0..SLOTS {
        if state.slots[index] == buffer {
            state.in_flight[index].store(false, Ordering::Release);
            state
                .diagnostics
                .add(&state.diagnostics.completed_callbacks, 1);
            return;
        }
    }
}

/// Device and stream negotiation is performed by macOS. Not yet certified as
/// hardware bit-perfect; host must distinguish that from decoder accuracy.
pub struct MacAudioQueueSink {
    api: Arc<dyn AudioQueueApi>,
    queue: AudioQueueRef,
    context: Option<Box<CallbackContext>>,
    format: Option<AudioFormat>,
    started: bool,
    paused: bool,
    registered: bool,
    pending_teardown: bool,
    diagnostics: Arc<AudioQueueDiagnostics>,
    diagnostics_summary_emitted: bool,
    drain_timeout: Duration,
    #[cfg(test)]
    write_timeout: Duration,
}
impl Default for MacAudioQueueSink {
    fn default() -> Self {
        let diagnostics = Arc::new(AudioQueueDiagnostics::default());
        diagnostics.enabled.store(
            diagnostics_enabled_from_value(
                std::env::var_os("RUST_MUSIC_AUDIO_DIAGNOSTICS").as_deref(),
            ),
            Ordering::Relaxed,
        );
        Self {
            api: Arc::new(SystemAudioQueue),
            queue: ptr::null_mut(),
            context: None,
            format: None,
            started: false,
            paused: false,
            registered: false,
            pending_teardown: false,
            diagnostics,
            diagnostics_summary_emitted: false,
            drain_timeout: MAX_DRAIN_WAIT,
            #[cfg(test)]
            write_timeout: MAX_WAIT,
        }
    }
}
impl MacAudioQueueSink {
    /// Enables or disables low-overhead counters for subsequent AudioQueue operations.
    /// They default off unless `RUST_MUSIC_AUDIO_DIAGNOSTICS=1` is set at sink creation.
    /// Call from a non-callback thread.
    pub fn set_diagnostics_enabled(&mut self, enabled: bool) {
        self.diagnostics.enabled.store(enabled, Ordering::Relaxed);
    }

    /// Returns a snapshot suitable for formatting or logging off the audio callback.
    pub fn diagnostics_snapshot(&self) -> AudioQueueDiagnosticsSnapshot {
        self.diagnostics.snapshot()
    }
}

impl PcmSink for MacAudioQueueSink {
    fn open(&mut self, format: AudioFormat) -> Result<()> {
        format.validate()?;
        if !self.queue.is_null() {
            return Err(MusicError::Invalid("AudioQueue already open"));
        }
        if !matches!(format.bits_per_sample, 8 | 16 | 24 | 32) {
            return Err(MusicError::Unsupported(
                "AudioQueue needs packed 8/16/24/32-bit PCM",
            ));
        }
        self.diagnostics.set_format(format);
        reserve_queue()?;
        self.registered = true;
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
        let mut context = Box::new(CallbackContext::new(Arc::clone(&self.diagnostics)));
        let mut queue = ptr::null_mut();
        self.diagnostics
            .add(&self.diagnostics.output_creation_attempts, 1);
        let create_err = self.api.new_output(
            &desc,
            completed,
            (&mut *context as *mut CallbackContext).cast(),
            &mut queue,
        );
        self.diagnostics
            .status(&self.diagnostics.output_creation_status, create_err);
        let route_err = if create_err == 0 {
            let status = self.api.route_to_default_output(queue);
            self.diagnostics
                .status(&self.diagnostics.default_route_status, status);
            status
        } else {
            0
        };
        let err = if create_err != 0 {
            create_err
        } else {
            route_err
        };
        if err != 0 {
            let dispose_status = if queue.is_null() {
                None
            } else {
                self.diagnostics.add(&self.diagnostics.teardown_attempts, 1);
                let status = self.api.dispose(queue, 1);
                self.diagnostics
                    .status(&self.diagnostics.dispose_status, status);
                Some(status)
            };
            let disposed = dispose_status.is_none_or(|status| status == 0);
            if disposed {
                release_queue(false);
                self.registered = false;
            } else {
                self.queue = queue;
                self.context = Some(context);
                self.pending_teardown = true;
                mark_pending_teardown();
            }
            self.emit_diagnostics_summary();
            let failed_stage = if create_err != 0 {
                "AudioQueueNewOutput"
            } else {
                "AudioQueueSetProperty(CurrentDevice)"
            };
            return Err(MusicError::Output(format!("{failed_stage} OSStatus {err}")));
        }
        for slot in &mut context.slots {
            let err =
                self.api
                    .allocate_buffer(queue, FRAMES_PER_SLOT as u32 * bytes_per_frame, slot);
            self.diagnostics
                .status(&self.diagnostics.buffer_allocation_status, err);
            if err != 0 {
                self.diagnostics.add(&self.diagnostics.teardown_attempts, 1);
                let dispose = self.api.dispose(queue, 1);
                self.diagnostics
                    .status(&self.diagnostics.dispose_status, dispose);
                if dispose != 0 {
                    // Keep callback state alive while a partially initialized
                    // queue still exists; a later finish/drop can retry.
                    self.queue = queue;
                    self.context = Some(context);
                    self.format = None;
                    self.started = false;
                    self.pending_teardown = true;
                    mark_pending_teardown();
                    self.emit_diagnostics_summary();
                    return Err(MusicError::Output(format!(
                        "AudioQueueAllocateBuffer OSStatus {err}; dispose={dispose}"
                    )));
                }
                release_queue(false);
                self.registered = false;
                self.emit_diagnostics_summary();
                return Err(MusicError::Output(format!(
                    "AudioQueueAllocateBuffer OSStatus {err}"
                )));
            }
            self.diagnostics.add(&self.diagnostics.allocated_buffers, 1);
            let capacity = if slot.is_null() {
                0
            } else {
                // AudioQueue supplies this capacity in its output buffer header.
                unsafe { (**slot).data_capacity }
            };
            self.diagnostics.add(
                &self.diagnostics.allocated_capacity_bytes,
                u64::from(capacity),
            );
        }
        self.queue = queue;
        self.context = Some(context);
        self.format = Some(format);
        self.started = false;
        self.paused = false;
        Ok(())
    }
    fn pause(&mut self) -> Result<()> {
        if self.queue.is_null() {
            return Err(MusicError::Invalid("AudioQueue is closed"));
        }
        if self.paused {
            return Ok(());
        }
        // Player::play opens the sink before its worker submits the first
        // decoded block. A pause in that interval is a local state change;
        // there is not yet a running AudioQueue to pause.
        if !self.started {
            self.paused = true;
            return Ok(());
        }
        let err = self.api.pause(self.queue);
        if err != 0 {
            return Err(MusicError::Output(format!(
                "AudioQueuePause OSStatus {err}"
            )));
        }
        self.paused = true;
        Ok(())
    }
    fn resume(&mut self) -> Result<()> {
        if self.queue.is_null() || !self.paused {
            return Err(MusicError::Invalid("AudioQueue is not paused"));
        }
        // Do not start an empty AudioQueue. The first subsequent write will
        // enqueue source PCM and start it through the normal write path.
        if !self.started {
            self.paused = false;
            return Ok(());
        }
        let err = record_start(&*self.api, self.queue, &self.diagnostics);
        if err != 0 {
            return Err(MusicError::Output(format!(
                "AudioQueueStart OSStatus {err}"
            )));
        }
        self.paused = false;
        Ok(())
    }
    fn write(&mut self, block: &PcmBlock) -> Result<()> {
        block.validate()?;
        if self.paused {
            return Err(MusicError::Invalid("AudioQueue is paused"));
        }
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
        #[cfg(test)]
        let write_timeout = self.write_timeout;
        #[cfg(not(test))]
        let write_timeout = MAX_WAIT;
        for samples in block.samples.chunks(FRAMES_PER_SLOT * channels) {
            let deadline = Instant::now() + write_timeout;
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
            let err = self.api.enqueue_buffer(self.queue, buffer);
            if err != 0 {
                context.in_flight[index].store(false, Ordering::Release);
                return Err(MusicError::Output(format!(
                    "AudioQueueEnqueueBuffer OSStatus {err}"
                )));
            }
            let frames = (samples.len() / channels) as u64;
            self.diagnostics.add(&self.diagnostics.enqueued_buffers, 1);
            self.diagnostics
                .add(&self.diagnostics.enqueued_frames, frames);
            if !self.started {
                let err = record_start(&*self.api, self.queue, &self.diagnostics);
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
        let queue = self.queue;
        let had_audio = self.started && !self.paused;
        self.diagnostics.add(&self.diagnostics.teardown_attempts, 1);
        // Apple documents stop(false) as asynchronous; dispose(false) is also
        // asynchronous. Keep callback state alive through buffer completion,
        // then use dispose(true), which synchronously ends callbacks before
        // releasing their user data. Docs:
        // https://developer.apple.com/documentation/audiotoolbox/audioqueuestop%28_%3A_%3A%29
        // https://developer.apple.com/documentation/audiotoolbox/audioqueuedispose%28_%3A_%3A%29
        let stop = self.api.stop(queue, if had_audio { 0 } else { 1 });
        self.diagnostics.status(&self.diagnostics.stop_status, stop);
        let drained = stop == 0
            && (!had_audio
                || self
                    .context
                    .as_deref()
                    .is_some_and(|context| wait_for_drain(context, self.drain_timeout)));
        self.diagnostics.set_drain_complete(drained);
        let forced_stop = if stop != 0 || !drained {
            self.api.stop(queue, 1)
        } else {
            0
        };
        if stop != 0 || !drained {
            self.diagnostics
                .status(&self.diagnostics.immediate_stop_status, forced_stop);
        }
        let dispose = self.api.dispose(queue, 1);
        self.diagnostics
            .status(&self.diagnostics.dispose_status, dispose);
        self.emit_diagnostics_summary();

        if dispose == 0 {
            self.queue = ptr::null_mut();
            self.context.take();
            self.format = None;
            self.started = false;
            self.paused = false;
            if self.pending_teardown {
                clear_pending_teardown();
                self.pending_teardown = false;
            }
            if self.registered {
                release_queue(false);
                self.registered = false;
            }
        } else {
            // The queue may still retain callback state. Keep it registered
            // for retry, but refuse further PCM writes after teardown fails.
            if !self.pending_teardown {
                self.pending_teardown = true;
                mark_pending_teardown();
            }
            self.format = None;
            self.started = false;
        }

        if stop != 0 || !drained || forced_stop != 0 || dispose != 0 {
            return Err(MusicError::Output(format!(
                "AudioQueue stop={stop} drain_complete={drained} immediate_stop={forced_stop} dispose={dispose}"
            )));
        }
        Ok(())
    }
}
impl MacAudioQueueSink {
    fn emit_diagnostics_summary(&mut self) {
        if self.diagnostics.enabled.load(Ordering::Relaxed) && !self.diagnostics_summary_emitted {
            eprintln!("{}", self.diagnostics.record());
            self.diagnostics_summary_emitted = true;
        }
    }
}

impl Drop for MacAudioQueueSink {
    fn drop(&mut self) {
        if self.finish().is_err() && !self.queue.is_null() {
            let Some(context) = self.context.take() else {
                return;
            };
            let mut registry = registry_lock();
            if self.pending_teardown {
                registry.pending_teardowns = registry.pending_teardowns.saturating_sub(1);
                self.pending_teardown = false;
            }
            if self.registered {
                registry.active = registry.active.saturating_sub(1);
                self.registered = false;
            }
            debug_assert!(registry.orphans.len() < MAX_TRACKED_QUEUES);
            registry.orphans.push(OrphanedQueue {
                api: self.api.clone(),
                queue: self.queue,
                _context: context,
            });
            self.queue = ptr::null_mut();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    const FAKE_QUEUE: AudioQueueRef = 1usize as AudioQueueRef;
    const FAKE_INVALID_DEVICE: i32 = -66_680;
    static SERIAL_TESTS: Mutex<()> = Mutex::new(());

    struct FakeBuffer {
        header: Box<AudioQueueBuffer>,
        _storage: Box<[u8]>,
    }

    // Fake buffers are only accessed synchronously by the unit test thread.
    unsafe impl Send for FakeBuffer {}

    impl FakeBuffer {
        fn new(bytes: u32) -> Self {
            let mut storage = vec![0u8; bytes as usize].into_boxed_slice();
            let header = Box::new(AudioQueueBuffer {
                data_capacity: bytes,
                data: storage.as_mut_ptr().cast(),
                data_size: 0,
                user_data: ptr::null_mut(),
                packet_description_capacity: 0,
                packet_descriptions: ptr::null_mut(),
                packet_description_count: 0,
            });
            Self {
                header,
                _storage: storage,
            }
        }
    }

    #[derive(Default)]
    struct FakeState {
        new_status: i32,
        new_calls: usize,
        route_status: Option<i32>,
        device_available: Option<bool>,
        fail_allocate_at: Option<usize>,
        allocate_calls: usize,
        enqueue_status: i32,
        start_status: i32,
        current_device_status: Option<i32>,
        current_device_associated: Option<bool>,
        current_device_matches_default: Option<bool>,
        pause_status: i32,
        stop_status: i32,
        dispose_status: i32,
        stop_calls: usize,
        dispose_calls: usize,
        suppress_callbacks: bool,
        callback: Option<unsafe extern "C" fn(*mut c_void, AudioQueueRef, AudioQueueBufferRef)>,
        callback_user_data: usize,
        pending_buffers: Vec<usize>,
        stop_modes: Vec<u8>,
        dispose_modes: Vec<u8>,
        events: Vec<&'static str>,
        enqueued_pcm: Vec<Vec<u8>>,
        buffers: Vec<FakeBuffer>,
    }

    #[derive(Default)]
    struct FakeAudioQueue(Mutex<FakeState>);

    impl AudioQueueApi for FakeAudioQueue {
        fn new_output(
            &self,
            _format: *const AudioStreamBasicDescription,
            callback: unsafe extern "C" fn(*mut c_void, AudioQueueRef, AudioQueueBufferRef),
            user_data: *mut c_void,
            out_queue: *mut AudioQueueRef,
        ) -> i32 {
            let mut state = self.0.lock().unwrap();
            state.new_calls += 1;
            let status = if state.device_available == Some(false) {
                FAKE_INVALID_DEVICE
            } else {
                state.new_status
            };
            if status == 0 {
                state.callback = Some(callback);
                state.callback_user_data = user_data as usize;
                unsafe { *out_queue = FAKE_QUEUE };
            }
            status
        }

        fn route_to_default_output(&self, _queue: AudioQueueRef) -> i32 {
            self.0.lock().unwrap().route_status.unwrap_or(0)
        }

        fn allocate_buffer(
            &self,
            _queue: AudioQueueRef,
            bytes: u32,
            out: *mut AudioQueueBufferRef,
        ) -> i32 {
            let mut state = self.0.lock().unwrap();
            let call = state.allocate_calls;
            state.allocate_calls += 1;
            if state.fail_allocate_at == Some(call) {
                return -50;
            }
            let mut buffer = FakeBuffer::new(bytes);
            let reference = &mut *buffer.header as AudioQueueBufferRef;
            state.buffers.push(buffer);
            unsafe { *out = reference };
            0
        }

        fn enqueue_buffer(&self, _queue: AudioQueueRef, buffer: AudioQueueBufferRef) -> i32 {
            let mut state = self.0.lock().unwrap();
            state.events.push("enqueue");
            let status = if state.device_available == Some(false) {
                FAKE_INVALID_DEVICE
            } else {
                state.enqueue_status
            };
            if status == 0 {
                let size = unsafe { (*buffer).data_size as usize };
                let bytes =
                    unsafe { std::slice::from_raw_parts((*buffer).data.cast::<u8>(), size) };
                state.enqueued_pcm.push(bytes.to_vec());
                state.pending_buffers.push(buffer as usize);
            }
            status
        }

        fn start(&self, _queue: AudioQueueRef) -> i32 {
            let mut state = self.0.lock().unwrap();
            state.events.push("start");
            state.start_status
        }

        fn current_device(&self, _queue: AudioQueueRef) -> (i32, bool, Option<bool>) {
            let state = self.0.lock().unwrap();
            (
                state.current_device_status.unwrap_or(0),
                state.current_device_associated.unwrap_or(true),
                state.current_device_matches_default.or(Some(true)),
            )
        }

        fn pause(&self, _queue: AudioQueueRef) -> i32 {
            let mut state = self.0.lock().unwrap();
            state.events.push("pause");
            state.pause_status
        }

        fn stop(&self, queue: AudioQueueRef, immediate: u8) -> i32 {
            let mut state = self.0.lock().unwrap();
            state.stop_calls += 1;
            state.stop_modes.push(immediate);
            state.events.push(if immediate == 0 {
                "stop_drain"
            } else {
                "stop_immediate"
            });
            let status = state.stop_status;
            let pending = if status == 0 && immediate == 0 && !state.suppress_callbacks {
                std::mem::take(&mut state.pending_buffers)
            } else {
                Vec::new()
            };
            let callback = state.callback;
            let user_data = state.callback_user_data;
            drop(state);

            if let Some(callback) = callback {
                for buffer in pending.iter().copied() {
                    unsafe {
                        callback(
                            user_data as *mut c_void,
                            queue,
                            buffer as AudioQueueBufferRef,
                        );
                    }
                }
            }
            if !pending.is_empty() {
                self.0.lock().unwrap().events.push("callbacks_complete");
            }
            status
        }

        fn dispose(&self, _queue: AudioQueueRef, immediate: u8) -> i32 {
            let mut state = self.0.lock().unwrap();
            state.dispose_calls += 1;
            state.dispose_modes.push(immediate);
            state.events.push("dispose");
            if state.dispose_status == 0 {
                state.buffers.clear();
                state.pending_buffers.clear();
            }
            state.dispose_status
        }
    }

    fn fake_sink() -> (MacAudioQueueSink, Arc<FakeAudioQueue>) {
        let api = Arc::new(FakeAudioQueue::default());
        let mut sink = MacAudioQueueSink::default();
        sink.api = api.clone();
        sink.drain_timeout = Duration::from_millis(20);
        (sink, api)
    }

    fn valid_format() -> AudioFormat {
        AudioFormat::new(44_100, 1, 16).unwrap()
    }

    fn one_sample() -> PcmBlock {
        PcmBlock {
            format: valid_format(),
            samples: vec![123],
        }
    }

    #[test]
    fn nonzero_16_bit_pcm_is_copied_little_endian_and_enqueued_before_start() {
        let _serial = SERIAL_TESTS.lock().unwrap();
        let (mut sink, api) = fake_sink();
        sink.open(valid_format()).unwrap();
        sink.write(&PcmBlock {
            format: valid_format(),
            samples: vec![0x1234, -2, i16::MIN.into(), i16::MAX.into()],
        })
        .unwrap();

        let state = api.0.lock().unwrap();
        assert_eq!(
            state.enqueued_pcm,
            vec![vec![0x34, 0x12, 0xfe, 0xff, 0x00, 0x80, 0xff, 0x7f]]
        );
        assert_eq!(state.events, vec!["enqueue", "start"]);
        assert_eq!(state.pending_buffers.len(), 1);
    }

    #[test]
    fn new_output_failure_leaves_sink_closed() {
        let _serial = SERIAL_TESTS.lock().unwrap();
        let (mut sink, api) = fake_sink();
        api.0.lock().unwrap().new_status = -50;

        assert!(matches!(
            sink.open(valid_format()),
            Err(MusicError::Output(_))
        ));
        assert!(sink.queue.is_null());
        assert!(sink.context.is_none());
        assert_eq!(api.0.lock().unwrap().dispose_calls, 0);
    }

    #[test]
    fn allocation_failure_disposes_partially_created_queue() {
        let _serial = SERIAL_TESTS.lock().unwrap();
        let (mut sink, api) = fake_sink();
        api.0.lock().unwrap().fail_allocate_at = Some(1);

        assert!(matches!(
            sink.open(valid_format()),
            Err(MusicError::Output(_))
        ));
        assert!(sink.queue.is_null());
        assert!(sink.context.is_none());
        let state = api.0.lock().unwrap();
        assert_eq!(state.dispose_calls, 1);
        assert!(state.buffers.is_empty());
        drop(state);

        let (mut next_sink, _) = fake_sink();
        next_sink.open(valid_format()).unwrap();
        next_sink.finish().unwrap();
    }

    #[test]
    fn allocation_and_dispose_failure_preserve_callback_context_for_retry() {
        let _serial = SERIAL_TESTS.lock().unwrap();
        let (mut sink, api) = fake_sink();
        {
            let mut state = api.0.lock().unwrap();
            state.fail_allocate_at = Some(1);
            state.dispose_status = -50;
        }

        assert!(matches!(
            sink.open(valid_format()),
            Err(MusicError::Output(_))
        ));
        assert!(!sink.queue.is_null());
        assert!(sink.context.is_some());
        assert!(matches!(
            sink.write(&one_sample()),
            Err(MusicError::Invalid(_))
        ));

        api.0.lock().unwrap().dispose_status = 0;
        sink.finish().unwrap();
        assert!(sink.queue.is_null());
        assert!(sink.context.is_none());
    }

    #[test]
    fn enqueue_failure_clears_slot_and_finish_disposes_queue() {
        let _serial = SERIAL_TESTS.lock().unwrap();
        let (mut sink, api) = fake_sink();
        sink.open(valid_format()).unwrap();
        api.0.lock().unwrap().enqueue_status = -50;

        assert!(matches!(
            sink.write(&one_sample()),
            Err(MusicError::Output(_))
        ));
        assert!(sink
            .context
            .as_ref()
            .unwrap()
            .in_flight
            .iter()
            .all(|slot| !slot.load(Ordering::Acquire)));
        api.0.lock().unwrap().enqueue_status = 0;
        sink.finish().unwrap();
        assert!(sink.queue.is_null());
        assert!(sink.context.is_none());
        let state = api.0.lock().unwrap();
        assert_eq!(state.events.first(), Some(&"enqueue"));
        assert!(state.enqueued_pcm.is_empty());
    }

    #[test]
    fn simulated_synchronous_invalid_device_enqueue_failure_recovers_after_device_returns() {
        let _serial = SERIAL_TESTS.lock().unwrap();
        let active_before = registry_lock().active;
        let (mut sink, api) = fake_sink();

        // Start a queue while the fake device is available, then have a later
        // active enqueue synchronously report Apple's InvalidDevice OSStatus.
        sink.open(valid_format()).unwrap();
        sink.write(&one_sample()).unwrap();
        assert!(sink.started);
        assert_eq!(registry_lock().active, active_before + 1);
        let failed_slot = sink
            .context
            .as_ref()
            .unwrap()
            .in_flight
            .iter()
            .position(|slot| !slot.load(Ordering::Acquire))
            .unwrap();
        api.0.lock().unwrap().device_available = Some(false);

        assert!(matches!(
            sink.write(&one_sample()),
            Err(MusicError::Output(message)) if message.contains("-66680")
        ));
        assert!(!sink.context.as_ref().unwrap().in_flight[failed_slot].load(Ordering::Acquire));

        // This is a synchronous API-error simulation, not hot-unplug detection.
        // Finishing drains the prior successful buffer and releases capacity.
        sink.finish().unwrap();
        assert!(sink.queue.is_null());
        assert!(sink.context.is_none());
        assert!(!sink.registered);
        assert_eq!(registry_lock().active, active_before);
        assert_eq!(api.0.lock().unwrap().dispose_calls, 1);

        // Restoring the fake device permits a fresh queue to open and write.
        api.0.lock().unwrap().device_available = Some(true);
        let (mut recovered_sink, _) = fake_sink();
        recovered_sink.api = api.clone();
        recovered_sink.open(valid_format()).unwrap();
        recovered_sink.write(&one_sample()).unwrap();
        assert!(recovered_sink.started);
        recovered_sink.finish().unwrap();
        assert!(recovered_sink.queue.is_null());
        assert!(recovered_sink.context.is_none());
        assert_eq!(registry_lock().active, active_before);
        assert_eq!(api.0.lock().unwrap().new_calls, 2);
        assert_eq!(api.0.lock().unwrap().dispose_calls, 2);
    }

    #[test]
    fn graceful_finish_waits_for_callbacks_before_synchronous_dispose() {
        let _serial = SERIAL_TESTS.lock().unwrap();
        let (mut sink, api) = fake_sink();
        sink.open(valid_format()).unwrap();
        sink.write(&one_sample()).unwrap();

        sink.finish().unwrap();

        let state = api.0.lock().unwrap();
        assert_eq!(state.stop_modes, vec![0]);
        assert_eq!(state.dispose_modes, vec![1]);
        let callback_done = state
            .events
            .iter()
            .position(|event| *event == "callbacks_complete")
            .unwrap();
        let dispose = state
            .events
            .iter()
            .position(|event| *event == "dispose")
            .unwrap();
        assert!(callback_done < dispose);
        assert!(state.pending_buffers.is_empty());
    }

    #[test]
    fn drain_timeout_forces_immediate_stop_then_synchronous_dispose() {
        let _serial = SERIAL_TESTS.lock().unwrap();
        let (mut sink, api) = fake_sink();
        sink.open(valid_format()).unwrap();
        sink.write(&one_sample()).unwrap();
        api.0.lock().unwrap().suppress_callbacks = true;

        assert!(matches!(sink.finish(), Err(MusicError::Output(_))));

        let state = api.0.lock().unwrap();
        assert_eq!(state.stop_modes, vec![0, 1]);
        assert_eq!(state.dispose_modes, vec![1]);
        assert!(state.pending_buffers.is_empty());
        assert!(sink.queue.is_null());
        assert!(sink.context.is_none());
    }

    #[test]
    fn simulated_callback_starvation_timeout_escalates_teardown_and_allows_reopen() {
        let _serial = SERIAL_TESTS.lock().unwrap();
        let active_before = registry_lock().active;
        let (mut sink, api) = fake_sink();
        sink.write_timeout = Duration::from_millis(20);

        sink.open(valid_format()).unwrap();
        for _ in 0..SLOTS {
            sink.write(&one_sample()).unwrap();
        }
        assert!(sink.started);
        assert!(sink
            .context
            .as_ref()
            .unwrap()
            .in_flight
            .iter()
            .all(|slot| slot.load(Ordering::Acquire)));
        assert_eq!(api.0.lock().unwrap().pending_buffers.len(), SLOTS);

        // The fake retains all queued buffers and suppresses completion on
        // stop. This models callback starvation only, not physical unplug.
        api.0.lock().unwrap().suppress_callbacks = true;
        assert!(matches!(
            sink.write(&one_sample()),
            Err(MusicError::Output(message)) if message.contains("AudioQueue buffer timed out")
        ));
        assert_eq!(api.0.lock().unwrap().pending_buffers.len(), SLOTS);

        // With no callbacks to drain, finish escalates to immediate stop and
        // synchronous dispose, then releases the queue registry/context.
        assert!(matches!(sink.finish(), Err(MusicError::Output(_))));
        assert!(sink.queue.is_null());
        assert!(sink.context.is_none());
        assert!(!sink.registered);
        let state = api.0.lock().unwrap();
        assert_eq!(state.stop_modes, vec![0, 1]);
        assert_eq!(state.dispose_modes, vec![1]);
        assert!(state.pending_buffers.is_empty());
        drop(state);
        assert_eq!(registry_lock().active, active_before);
        assert_eq!(registry_lock().pending_teardowns, 0);

        // Restore fake callbacks and reuse the same API for a fresh queue.
        // This is simulated recovery, not physical unplug/hot-plug evidence.
        api.0.lock().unwrap().suppress_callbacks = false;
        let (mut recovered_sink, _) = fake_sink();
        recovered_sink.api = api.clone();
        recovered_sink.open(valid_format()).unwrap();
        recovered_sink.write(&one_sample()).unwrap();
        assert!(recovered_sink.started);
        recovered_sink.finish().unwrap();
        assert!(recovered_sink.queue.is_null());
        assert!(recovered_sink.context.is_none());
        assert_eq!(registry_lock().active, active_before);
        let state = api.0.lock().unwrap();
        assert_eq!(state.new_calls, 2);
        assert_eq!(state.dispose_calls, 2);
    }

    #[test]
    fn start_failure_is_cleaned_up_by_finish() {
        let _serial = SERIAL_TESTS.lock().unwrap();
        let (mut sink, api) = fake_sink();
        sink.open(valid_format()).unwrap();
        api.0.lock().unwrap().start_status = -50;

        assert!(matches!(
            sink.write(&one_sample()),
            Err(MusicError::Output(_))
        ));
        assert!(!sink.started);
        {
            let state = api.0.lock().unwrap();
            assert_eq!(state.events, vec!["enqueue", "start"]);
            assert_eq!(state.enqueued_pcm, vec![vec![123, 0]]);
            assert_eq!(state.pending_buffers.len(), 1);
        }
        api.0.lock().unwrap().start_status = 0;
        sink.finish().unwrap();
        assert!(sink.queue.is_null());
        assert!(sink.context.is_none());
        let state = api.0.lock().unwrap();
        assert_eq!(state.stop_modes, vec![1]);
        assert_eq!(state.dispose_modes, vec![1]);
    }

    #[test]
    fn pause_resume_preserves_queue_and_finishes_without_waiting_on_paused_callbacks() {
        let _serial = SERIAL_TESTS.lock().unwrap();
        let (mut sink, api) = fake_sink();
        sink.open(valid_format()).unwrap();
        sink.write(&one_sample()).unwrap();
        sink.pause().unwrap();
        assert!(sink.paused);
        api.0.lock().unwrap().suppress_callbacks = true;
        sink.resume().unwrap();
        assert!(!sink.paused);
        sink.pause().unwrap();
        sink.finish().unwrap();
        let state = api.0.lock().unwrap();
        assert_eq!(
            state
                .events
                .iter()
                .filter(|event| **event == "pause")
                .count(),
            2
        );
        assert!(state.stop_modes.contains(&1));
        assert!(sink.queue.is_null());
    }

    #[test]
    fn pause_failure_is_reported_without_changing_sink_state() {
        let _serial = SERIAL_TESTS.lock().unwrap();
        let (mut sink, api) = fake_sink();
        sink.open(valid_format()).unwrap();
        sink.write(&one_sample()).unwrap();
        api.0.lock().unwrap().pause_status = -50;
        assert!(matches!(sink.pause(), Err(MusicError::Output(_))));
        assert!(!sink.paused);
        api.0.lock().unwrap().suppress_callbacks = false;
        sink.finish().unwrap();
    }

    #[test]
    fn pause_and_resume_before_first_buffer_defer_queue_start_until_write() {
        let _serial = SERIAL_TESTS.lock().unwrap();
        let (mut sink, api) = fake_sink();
        sink.open(valid_format()).unwrap();

        sink.pause().unwrap();
        assert!(sink.paused);
        assert!(!sink.started);
        assert!(api.0.lock().unwrap().events.is_empty());

        sink.resume().unwrap();
        assert!(!sink.paused);
        assert!(!sink.started);
        assert!(api.0.lock().unwrap().events.is_empty());

        sink.write(&one_sample()).unwrap();
        assert!(sink.started);
        assert_eq!(api.0.lock().unwrap().events, vec!["enqueue", "start"]);
        sink.finish().unwrap();
    }

    #[test]
    fn failed_dispose_keeps_callback_context_until_retry_succeeds() {
        let _serial = SERIAL_TESTS.lock().unwrap();
        let (mut sink, api) = fake_sink();
        sink.open(valid_format()).unwrap();
        api.0.lock().unwrap().dispose_status = -50;

        assert!(matches!(sink.finish(), Err(MusicError::Output(_))));
        assert!(!sink.queue.is_null());
        assert!(sink.context.is_some());
        assert!(matches!(
            sink.write(&one_sample()),
            Err(MusicError::Invalid(_))
        ));

        api.0.lock().unwrap().dispose_status = 0;
        sink.finish().unwrap();
        assert!(sink.queue.is_null());
        assert!(sink.context.is_none());
    }

    #[test]
    fn drop_retains_failed_queue_and_retries_before_allowing_new_open() {
        let _serial = SERIAL_TESTS.lock().unwrap();
        let (mut sink, old_api) = fake_sink();
        sink.open(valid_format()).unwrap();
        old_api.0.lock().unwrap().dispose_status = -50;

        drop(sink);
        assert_eq!(registry_lock().orphans.len(), 1);
        assert_eq!(old_api.0.lock().unwrap().dispose_calls, 1);

        let (mut blocked_sink, blocked_api) = fake_sink();
        assert!(matches!(
            blocked_sink.open(valid_format()),
            Err(MusicError::Output(_))
        ));
        assert_eq!(blocked_api.0.lock().unwrap().new_calls, 0);
        assert_eq!(old_api.0.lock().unwrap().dispose_calls, 2);

        old_api.0.lock().unwrap().dispose_status = 0;
        let (mut recovered_sink, recovered_api) = fake_sink();
        recovered_sink.open(valid_format()).unwrap();
        assert_eq!(old_api.0.lock().unwrap().dispose_calls, 3);
        assert!(old_api.0.lock().unwrap().buffers.is_empty());
        assert!(registry_lock().orphans.is_empty());
        assert_eq!(recovered_api.0.lock().unwrap().new_calls, 1);

        recovered_sink.finish().unwrap();
    }

    #[test]
    fn stop_failure_is_reported_after_successful_dispose() {
        let _serial = SERIAL_TESTS.lock().unwrap();
        let (mut sink, api) = fake_sink();
        sink.open(valid_format()).unwrap();
        sink.write(&one_sample()).unwrap();
        api.0.lock().unwrap().stop_status = -50;

        assert!(matches!(sink.finish(), Err(MusicError::Output(_))));
        assert!(sink.queue.is_null());
        assert!(sink.context.is_none());
        let state = api.0.lock().unwrap();
        assert_eq!(state.stop_modes, vec![0, 1]);
        assert_eq!(state.dispose_calls, 1);
        assert_eq!(state.dispose_modes, vec![1]);
    }

    #[test]
    fn open_rejects_invalid_public_format_before_audioqueue_creation() {
        let _serial = SERIAL_TESTS.lock().unwrap();
        let mut sink = MacAudioQueueSink::default();
        let invalid = AudioFormat {
            sample_rate: 44_100,
            channels: 0,
            bits_per_sample: 16,
        };
        assert!(matches!(
            sink.open(invalid),
            Err(MusicError::Unsupported(_))
        ));
        assert!(sink.queue.is_null());
        assert!(sink.context.is_none());
    }

    #[test]
    fn write_rejects_out_of_range_pcm_before_queue_access() {
        let _serial = SERIAL_TESTS.lock().unwrap();
        let mut sink = MacAudioQueueSink::default();
        let format = AudioFormat::new(44_100, 1, 16).unwrap();
        let invalid = PcmBlock {
            format,
            samples: vec![i32::from(i16::MAX) + 1],
        };
        assert!(matches!(sink.write(&invalid), Err(MusicError::Invalid(_))));
        assert!(sink.queue.is_null());
        assert!(sink.context.is_none());
    }

    #[test]
    fn diagnostics_are_opt_in_and_report_successful_queue_lifecycle() {
        let _serial = SERIAL_TESTS.lock().unwrap();
        let (mut sink, _) = fake_sink();
        let disabled = sink.diagnostics_snapshot();
        assert!(!disabled.enabled);
        assert_eq!(disabled.output_creation_attempts, 0);

        sink.set_diagnostics_enabled(true);
        sink.open(valid_format()).unwrap();
        sink.write(&PcmBlock {
            format: valid_format(),
            samples: vec![11, -12, 13, -14],
        })
        .unwrap();
        sink.finish().unwrap();

        let snapshot = sink.diagnostics_snapshot();
        assert!(snapshot.enabled);
        assert_eq!(snapshot.output_creation_attempts, 1);
        assert_eq!(snapshot.output_creation_status, Some(0));
        assert_eq!(snapshot.default_route_status, Some(0));
        assert_eq!(snapshot.sample_rate, 44_100);
        assert_eq!(snapshot.bits_per_sample, 16);
        assert_eq!(snapshot.channels, 1);
        assert_eq!(snapshot.output_creation_status, Some(0));
        assert_eq!(snapshot.allocated_buffers, SLOTS as u64);
        assert_eq!(
            snapshot.allocated_capacity_bytes,
            (SLOTS * FRAMES_PER_SLOT * 2) as u64
        );
        assert_eq!(snapshot.buffer_allocation_status, Some(0));
        assert_eq!(snapshot.enqueued_buffers, 1);
        assert_eq!(snapshot.enqueued_frames, 4);
        assert_eq!(snapshot.start_attempts, 1);
        assert_eq!(snapshot.start_status, Some(0));
        assert_eq!(snapshot.current_device_status, Some(0));
        assert_eq!(snapshot.current_device_associated, Some(true));
        assert_eq!(snapshot.current_device_matches_default, Some(true));
        assert_eq!(snapshot.completed_callbacks, 1);
        assert_eq!(snapshot.teardown_attempts, 1);
        assert_eq!(snapshot.stop_status, Some(0));
        assert_eq!(snapshot.drain_complete, Some(true));
        assert_eq!(snapshot.immediate_stop_status, None);
        assert_eq!(snapshot.dispose_status, Some(0));
        assert_eq!(
            sink.diagnostics.record(),
            "RUST_MUSIC_AUDIO_DIAG rate=44100 bits=16 channels=1 created=1 create_status=0 route_status=0 allocated=3 capacity_bytes=24576 allocation_status=0 enqueued=1 frames=4 starts=1 start_status=0 device_status=0 device_associated=1 device_default=1 callbacks=1 teardown=1 stop_status=0 drained=1 immediate_stop_status=none dispose_status=0"
        );
    }

    #[test]
    fn diagnostics_environment_switch_requires_exact_one() {
        assert!(!diagnostics_enabled_from_value(None));
        assert!(!diagnostics_enabled_from_value(Some(OsStr::new("0"))));
        assert!(!diagnostics_enabled_from_value(Some(OsStr::new("true"))));
        assert!(diagnostics_enabled_from_value(Some(OsStr::new("1"))));
    }

    #[test]
    fn diagnostics_capture_start_and_teardown_failures_without_callback_logging() {
        let _serial = SERIAL_TESTS.lock().unwrap();
        let (mut sink, api) = fake_sink();
        sink.set_diagnostics_enabled(true);
        sink.open(valid_format()).unwrap();
        api.0.lock().unwrap().start_status = -77;
        assert!(matches!(
            sink.write(&one_sample()),
            Err(MusicError::Output(_))
        ));
        api.0.lock().unwrap().stop_status = -50;
        assert!(matches!(sink.finish(), Err(MusicError::Output(_))));

        let snapshot = sink.diagnostics_snapshot();
        assert_eq!(snapshot.start_attempts, 1);
        assert_eq!(snapshot.start_status, Some(-77));
        assert_eq!(snapshot.current_device_status, Some(0));
        assert_eq!(snapshot.current_device_associated, Some(true));
        assert_eq!(snapshot.stop_status, Some(-50));
        assert_eq!(snapshot.drain_complete, Some(false));
        assert_eq!(snapshot.immediate_stop_status, Some(-50));
        assert_eq!(snapshot.dispose_status, Some(0));
    }

    #[test]
    fn diagnostics_capture_creation_and_allocation_osstatus_failures() {
        let _serial = SERIAL_TESTS.lock().unwrap();
        let (mut creation_sink, creation_api) = fake_sink();
        creation_sink.set_diagnostics_enabled(true);
        creation_api.0.lock().unwrap().new_status = -71;
        assert!(matches!(
            creation_sink.open(valid_format()),
            Err(MusicError::Output(_))
        ));
        let creation = creation_sink.diagnostics_snapshot();
        assert_eq!(creation.output_creation_attempts, 1);
        assert_eq!(creation.output_creation_status, Some(-71));
        assert_eq!(creation.allocated_buffers, 0);

        let (mut allocation_sink, allocation_api) = fake_sink();
        allocation_sink.set_diagnostics_enabled(true);
        allocation_api.0.lock().unwrap().fail_allocate_at = Some(1);
        assert!(matches!(
            allocation_sink.open(valid_format()),
            Err(MusicError::Output(_))
        ));
        let allocation = allocation_sink.diagnostics_snapshot();
        assert_eq!(allocation.output_creation_status, Some(0));
        assert_eq!(allocation.allocated_buffers, 1);
        assert_eq!(
            allocation.allocated_capacity_bytes,
            (FRAMES_PER_SLOT * 2) as u64
        );
        assert_eq!(allocation.buffer_allocation_status, Some(-50));
    }

    #[test]
    fn diagnostics_report_missing_current_output_device_after_start() {
        let _serial = SERIAL_TESTS.lock().unwrap();
        let (mut sink, api) = fake_sink();
        sink.set_diagnostics_enabled(true);
        sink.open(valid_format()).unwrap();
        {
            let mut state = api.0.lock().unwrap();
            state.current_device_status = Some(FAKE_INVALID_DEVICE);
            state.current_device_associated = Some(false);
            state.current_device_matches_default = Some(false);
        }
        assert!(sink.write(&one_sample()).is_ok());
        let snapshot = sink.diagnostics_snapshot();
        assert_eq!(snapshot.start_status, Some(0));
        assert_eq!(snapshot.current_device_status, Some(FAKE_INVALID_DEVICE));
        assert_eq!(snapshot.current_device_associated, Some(false));
        assert_eq!(snapshot.current_device_matches_default, Some(false));
    }

    #[test]
    fn default_output_route_failure_disposes_queue_before_playback() {
        let _serial = SERIAL_TESTS.lock().unwrap();
        let (mut sink, api) = fake_sink();
        sink.set_diagnostics_enabled(true);
        api.0.lock().unwrap().route_status = Some(FAKE_INVALID_DEVICE);
        assert!(sink.open(valid_format()).is_err());
        let snapshot = sink.diagnostics_snapshot();
        assert_eq!(snapshot.output_creation_status, Some(0));
        assert_eq!(snapshot.default_route_status, Some(FAKE_INVALID_DEVICE));
        assert_eq!(snapshot.allocated_buffers, 0);
        assert_eq!(snapshot.dispose_status, Some(0));
        assert!(sink.queue.is_null());
        assert!(sink.context.is_none());
    }
}
