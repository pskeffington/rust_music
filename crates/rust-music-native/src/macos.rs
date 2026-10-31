//! Own macOS AudioQueue PCM bridge. Audio callback flips preallocated slot flags
//! only; disk reading/decoding stays on the application's independent worker.
use rust_music_core::{AudioFormat, MusicError, PcmBlock, PcmSink, Result};
use std::{
    ffi::c_void,
    ptr,
    sync::{
        atomic::AtomicUsize,
        atomic::{AtomicBool, Ordering},
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
    fn allocate_buffer(
        &self,
        queue: AudioQueueRef,
        bytes: u32,
        out: *mut AudioQueueBufferRef,
    ) -> i32;
    fn enqueue_buffer(&self, queue: AudioQueueRef, buffer: AudioQueueBufferRef) -> i32;
    fn start(&self, queue: AudioQueueRef) -> i32;
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
}
// The context is initialized before AudioQueue creation, then its slot
// pointers remain immutable; callback-visible state uses atomics. Ownership
// may move into the process registry after a failed dispose.
unsafe impl Send for CallbackContext {}

impl CallbackContext {
    fn new() -> Self {
        Self {
            slots: [ptr::null_mut(); SLOTS],
            in_flight: std::array::from_fn(|_| AtomicBool::new(false)),
            active_callbacks: AtomicUsize::new(0),
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
    drain_timeout: Duration,
}
impl Default for MacAudioQueueSink {
    fn default() -> Self {
        Self {
            api: Arc::new(SystemAudioQueue),
            queue: ptr::null_mut(),
            context: None,
            format: None,
            started: false,
            paused: false,
            registered: false,
            pending_teardown: false,
            drain_timeout: MAX_DRAIN_WAIT,
        }
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
        let mut context = Box::new(CallbackContext::new());
        let mut queue = ptr::null_mut();
        let err = self.api.new_output(
            &desc,
            completed,
            (&mut *context as *mut CallbackContext).cast(),
            &mut queue,
        );
        if err != 0 {
            let disposed = queue.is_null() || self.api.dispose(queue, 1) == 0;
            if disposed {
                release_queue(false);
                self.registered = false;
            } else {
                self.queue = queue;
                self.context = Some(context);
                self.pending_teardown = true;
                mark_pending_teardown();
            }
            return Err(MusicError::Output(format!(
                "AudioQueueNewOutput OSStatus {err}"
            )));
        }
        for slot in &mut context.slots {
            let err =
                self.api
                    .allocate_buffer(queue, FRAMES_PER_SLOT as u32 * bytes_per_frame, slot);
            if err != 0 {
                let dispose = self.api.dispose(queue, 1);
                if dispose != 0 {
                    // Keep callback state alive while a partially initialized
                    // queue still exists; a later finish/drop can retry.
                    self.queue = queue;
                    self.context = Some(context);
                    self.format = None;
                    self.started = false;
                    self.pending_teardown = true;
                    mark_pending_teardown();
                    return Err(MusicError::Output(format!(
                        "AudioQueueAllocateBuffer OSStatus {err}; dispose={dispose}"
                    )));
                }
                release_queue(false);
                self.registered = false;
                return Err(MusicError::Output(format!(
                    "AudioQueueAllocateBuffer OSStatus {err}"
                )));
            }
        }
        self.queue = queue;
        self.context = Some(context);
        self.format = Some(format);
        self.started = false;
        self.paused = false;
        Ok(())
    }
    fn pause(&mut self) -> Result<()> {
        if self.queue.is_null() || !self.started {
            return Err(MusicError::Invalid("AudioQueue is not playing"));
        }
        if self.paused {
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
        if self.queue.is_null() || !self.started || !self.paused {
            return Err(MusicError::Invalid("AudioQueue is not paused"));
        }
        let err = self.api.start(self.queue);
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
            let err = self.api.enqueue_buffer(self.queue, buffer);
            if err != 0 {
                context.in_flight[index].store(false, Ordering::Release);
                return Err(MusicError::Output(format!(
                    "AudioQueueEnqueueBuffer OSStatus {err}"
                )));
            }
            if !self.started {
                let err = self.api.start(self.queue);
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
        // Apple documents stop(false) as asynchronous; dispose(false) is also
        // asynchronous. Keep callback state alive through buffer completion,
        // then use dispose(true), which synchronously ends callbacks before
        // releasing their user data. Docs:
        // https://developer.apple.com/documentation/audiotoolbox/audioqueuestop%28_%3A_%3A%29
        // https://developer.apple.com/documentation/audiotoolbox/audioqueuedispose%28_%3A_%3A%29
        let stop = self.api.stop(queue, if had_audio { 0 } else { 1 });
        let drained = stop == 0
            && (!had_audio
                || self
                    .context
                    .as_deref()
                    .is_some_and(|context| wait_for_drain(context, self.drain_timeout)));
        let forced_stop = if stop != 0 || !drained {
            self.api.stop(queue, 1)
        } else {
            0
        };
        let dispose = self.api.dispose(queue, 1);

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
        fail_allocate_at: Option<usize>,
        allocate_calls: usize,
        enqueue_status: i32,
        start_status: i32,
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
            if state.new_status == 0 {
                state.callback = Some(callback);
                state.callback_user_data = user_data as usize;
                unsafe { *out_queue = FAKE_QUEUE };
            }
            state.new_status
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
            let status = state.enqueue_status;
            if status == 0 {
                state.pending_buffers.push(buffer as usize);
            }
            status
        }

        fn start(&self, _queue: AudioQueueRef) -> i32 {
            let mut state = self.0.lock().unwrap();
            state.events.push("start");
            state.start_status
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
        api.0.lock().unwrap().start_status = 0;
        sink.finish().unwrap();
        assert!(sink.queue.is_null());
        assert!(sink.context.is_none());
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
}
