//! Own macOS AudioQueue PCM bridge. Audio callback flips preallocated slot flags
//! only; disk reading/decoding stays on the application's independent worker.
use rust_music_core::{AudioFormat, MusicError, PcmBlock, PcmSink, Result};
use std::{
    ffi::{c_char, c_void, CStr, OsStr},
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
// kAudioQueueParam_Volume from AudioToolbox's public ABI.
const QUEUE_PARAMETER_VOLUME: u32 = 1;
const AUDIO_HARDWARE_PROPERTY_DEVICES: u32 = u32::from_be_bytes(*b"dev#");
const AUDIO_HARDWARE_PROPERTY_DEFAULT_OUTPUT_DEVICE: u32 = u32::from_be_bytes(*b"dOut");
const AUDIO_DEVICE_PROPERTY_DEVICE_UID: u32 = u32::from_be_bytes(*b"uid ");
const AUDIO_OBJECT_PROPERTY_NAME: u32 = u32::from_be_bytes(*b"lnam");
const AUDIO_DEVICE_PROPERTY_STREAMS: u32 = u32::from_be_bytes(*b"stm#");
const AUDIO_OBJECT_PROPERTY_SCOPE_GLOBAL: u32 = u32::from_be_bytes(*b"glob");
const AUDIO_OBJECT_PROPERTY_SCOPE_OUTPUT: u32 = u32::from_be_bytes(*b"outp");
const AUDIO_OBJECT_SYSTEM_OBJECT_ID: u32 = 1;
const AUDIOQUEUE_ERR_INVALID_DEVICE: i32 = -66_680;
const MAX_AUDIO_DEVICE_IDS: usize = 256;
const FLAG_SIGNED: u32 = 1 << 2;
const FLAG_PACKED: u32 = 1 << 3;
const NO_STATUS: i64 = i64::MIN;
const MAX_DEVICE_TEXT_BYTES: usize = 512;
const CF_STRING_ENCODING_UTF8: u32 = 0x0800_0100;
const OUTPUT_DEVICE_UID_ENV: &str = "RUST_MUSIC_OUTPUT_DEVICE_UID";

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
    pub default_route_device_list_status: Option<i32>,
    pub default_route_device_listed: Option<bool>,
    pub route_target_device_listed: Option<bool>,
    pub allocated_buffers: u64,
    pub allocated_capacity_bytes: u64,
    pub buffer_allocation_status: Option<i32>,
    pub enqueued_buffers: u64,
    pub enqueued_frames: u64,
    pub nonzero_samples: u64,
    pub peak_sample: u64,
    pub start_attempts: u64,
    pub start_status: Option<i32>,
    pub current_device_status: Option<i32>,
    pub current_device_associated: Option<bool>,
    pub current_device_matches_default: Option<bool>,
    pub current_device_matches_target: Option<bool>,
    pub current_device_list_status: Option<i32>,
    pub current_device_listed: Option<bool>,
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
    default_route_device_list_status: AtomicI64,
    default_route_device_listed: AtomicI64,
    route_target_device_listed: AtomicI64,
    allocated_buffers: AtomicU64,
    allocated_capacity_bytes: AtomicU64,
    buffer_allocation_status: AtomicI64,
    enqueued_buffers: AtomicU64,
    enqueued_frames: AtomicU64,
    nonzero_samples: AtomicU64,
    peak_sample: AtomicU64,
    start_attempts: AtomicU64,
    start_status: AtomicI64,
    current_device_status: AtomicI64,
    current_device_associated: AtomicI64,
    current_device_matches_default: AtomicI64,
    current_device_matches_target: AtomicI64,
    current_device_list_status: AtomicI64,
    current_device_listed: AtomicI64,
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
            default_route_device_list_status: AtomicI64::new(NO_STATUS),
            default_route_device_listed: AtomicI64::new(NO_STATUS),
            route_target_device_listed: AtomicI64::new(NO_STATUS),
            allocated_buffers: AtomicU64::new(0),
            allocated_capacity_bytes: AtomicU64::new(0),
            buffer_allocation_status: AtomicI64::new(NO_STATUS),
            enqueued_buffers: AtomicU64::new(0),
            enqueued_frames: AtomicU64::new(0),
            nonzero_samples: AtomicU64::new(0),
            peak_sample: AtomicU64::new(0),
            start_attempts: AtomicU64::new(0),
            start_status: AtomicI64::new(NO_STATUS),
            current_device_status: AtomicI64::new(NO_STATUS),
            current_device_associated: AtomicI64::new(NO_STATUS),
            current_device_matches_default: AtomicI64::new(NO_STATUS),
            current_device_matches_target: AtomicI64::new(NO_STATUS),
            current_device_list_status: AtomicI64::new(NO_STATUS),
            current_device_listed: AtomicI64::new(NO_STATUS),
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
    fn reset_session(&self) {
        self.sample_rate.store(0, Ordering::Relaxed);
        self.bits_per_sample.store(0, Ordering::Relaxed);
        self.channels.store(0, Ordering::Relaxed);
        self.output_creation_attempts.store(0, Ordering::Relaxed);
        self.output_creation_status
            .store(NO_STATUS, Ordering::Relaxed);
        self.default_route_status
            .store(NO_STATUS, Ordering::Relaxed);
        self.default_route_device_list_status
            .store(NO_STATUS, Ordering::Relaxed);
        self.default_route_device_listed
            .store(NO_STATUS, Ordering::Relaxed);
        self.route_target_device_listed
            .store(NO_STATUS, Ordering::Relaxed);
        self.allocated_buffers.store(0, Ordering::Relaxed);
        self.allocated_capacity_bytes.store(0, Ordering::Relaxed);
        self.buffer_allocation_status
            .store(NO_STATUS, Ordering::Relaxed);
        self.enqueued_buffers.store(0, Ordering::Relaxed);
        self.enqueued_frames.store(0, Ordering::Relaxed);
        self.nonzero_samples.store(0, Ordering::Relaxed);
        self.peak_sample.store(0, Ordering::Relaxed);
        self.start_attempts.store(0, Ordering::Relaxed);
        self.start_status.store(NO_STATUS, Ordering::Relaxed);
        self.current_device_status
            .store(NO_STATUS, Ordering::Relaxed);
        self.current_device_associated
            .store(NO_STATUS, Ordering::Relaxed);
        self.current_device_matches_default
            .store(NO_STATUS, Ordering::Relaxed);
        self.current_device_matches_target
            .store(NO_STATUS, Ordering::Relaxed);
        self.current_device_list_status
            .store(NO_STATUS, Ordering::Relaxed);
        self.current_device_listed
            .store(NO_STATUS, Ordering::Relaxed);
        self.completed_callbacks.store(0, Ordering::Relaxed);
        self.teardown_attempts.store(0, Ordering::Relaxed);
        self.stop_status.store(NO_STATUS, Ordering::Relaxed);
        self.drain_complete.store(0, Ordering::Relaxed);
        self.immediate_stop_status
            .store(NO_STATUS, Ordering::Relaxed);
        self.dispose_status.store(NO_STATUS, Ordering::Relaxed);
    }

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
            default_route_device_list_status: status(&self.default_route_device_list_status),
            default_route_device_listed: optional_bool(&self.default_route_device_listed),
            route_target_device_listed: optional_bool(&self.route_target_device_listed),
            allocated_buffers: self.allocated_buffers.load(Ordering::Relaxed),
            allocated_capacity_bytes: self.allocated_capacity_bytes.load(Ordering::Relaxed),
            buffer_allocation_status: status(&self.buffer_allocation_status),
            enqueued_buffers: self.enqueued_buffers.load(Ordering::Relaxed),
            enqueued_frames: self.enqueued_frames.load(Ordering::Relaxed),
            nonzero_samples: self.nonzero_samples.load(Ordering::Relaxed),
            peak_sample: self.peak_sample.load(Ordering::Relaxed),
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
            current_device_matches_target: optional_bool(&self.current_device_matches_target),
            current_device_list_status: status(&self.current_device_list_status),
            current_device_listed: optional_bool(&self.current_device_listed),
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

    fn set_default_route(&self, route: DefaultOutputRoute) {
        if self.enabled.load(Ordering::Relaxed) {
            self.default_route_status
                .store(i64::from(route.status), Ordering::Relaxed);
            self.default_route_device_list_status.store(
                route.device_list_status.map_or(NO_STATUS, i64::from),
                Ordering::Relaxed,
            );
            self.default_route_device_listed.store(
                route.default_device_listed.map_or(NO_STATUS, i64::from),
                Ordering::Relaxed,
            );
            self.route_target_device_listed.store(
                route.target_device_listed.map_or(NO_STATUS, i64::from),
                Ordering::Relaxed,
            );
        }
    }

    fn set_current_device_observation(&self, observation: CurrentDeviceObservation) {
        if self.enabled.load(Ordering::Relaxed) {
            self.current_device_status
                .store(i64::from(observation.status), Ordering::Relaxed);
            self.current_device_associated
                .store(i64::from(observation.associated), Ordering::Relaxed);
            self.current_device_matches_default.store(
                observation.matches_default.map_or(NO_STATUS, i64::from),
                Ordering::Relaxed,
            );
            self.current_device_matches_target.store(
                observation.matches_target.map_or(NO_STATUS, i64::from),
                Ordering::Relaxed,
            );
            self.current_device_list_status.store(
                observation.device_list_status.map_or(NO_STATUS, i64::from),
                Ordering::Relaxed,
            );
            self.current_device_listed.store(
                observation.device_listed.map_or(NO_STATUS, i64::from),
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
            "RUST_MUSIC_AUDIO_DIAG rate={} bits={} channels={} created={} create_status={} route_status={} route_device_list_status={} route_default_listed={} route_target_listed={} allocated={} capacity_bytes={} allocation_status={} enqueued={} frames={} nonzero_samples={} peak_sample={} starts={} start_status={} device_status={} device_associated={} device_default={} device_target={} device_list_status={} device_listed={} callbacks={} teardown={} stop_status={} drained={} immediate_stop_status={} dispose_status={}",
            snapshot.sample_rate,
            snapshot.bits_per_sample,
            snapshot.channels,
            snapshot.output_creation_attempts,
            format_status(snapshot.output_creation_status),
            format_status(snapshot.default_route_status),
            format_status(snapshot.default_route_device_list_status),
            format_bool(snapshot.default_route_device_listed),
            format_bool(snapshot.route_target_device_listed),
            snapshot.allocated_buffers,
            snapshot.allocated_capacity_bytes,
            format_status(snapshot.buffer_allocation_status),
            snapshot.enqueued_buffers,
            snapshot.enqueued_frames,
            snapshot.nonzero_samples,
            snapshot.peak_sample,
            snapshot.start_attempts,
            format_status(snapshot.start_status),
            format_status(snapshot.current_device_status),
            format_bool(snapshot.current_device_associated),
            format_bool(snapshot.current_device_matches_default),
            format_bool(snapshot.current_device_matches_target),
            format_status(snapshot.current_device_list_status),
            format_bool(snapshot.current_device_listed),
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

fn optional_bool(field: &AtomicI64) -> Option<bool> {
    match field.load(Ordering::Relaxed) {
        0 => Some(false),
        1 => Some(true),
        _ => None,
    }
}

#[repr(C)]
struct AudioObjectPropertyAddress {
    selector: u32,
    scope: u32,
    element: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RouteFailureStage {
    DefaultOutputLookup,
    DeviceListEnumeration,
    DefaultDeviceNotListed,
    DefaultDeviceUidLookup,
    SelectedOutputNotListed,
    QueueDeviceBinding,
    QueueVolumeBinding,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RouteValidationFailure {
    status: i32,
    stage: RouteFailureStage,
    device_list_status: Option<i32>,
    default_device_listed: Option<bool>,
    target_device_listed: Option<bool>,
}

impl RouteFailureStage {
    const fn description(self) -> &'static str {
        match self {
            Self::DefaultOutputLookup => "CoreAudio default-output lookup",
            Self::DeviceListEnumeration => "CoreAudio output-device list enumeration",
            Self::DefaultDeviceNotListed => "CoreAudio default output is not in the device list",
            Self::DefaultDeviceUidLookup => "CoreAudio default-output UID lookup",
            Self::SelectedOutputNotListed => {
                "selected CoreAudio output is not in the output-device list"
            }
            Self::QueueDeviceBinding => "AudioQueue CurrentDevice binding",
            Self::QueueVolumeBinding => "AudioQueue unity-volume setting",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct DefaultOutputRoute {
    status: i32,
    device_list_status: Option<i32>,
    default_device_listed: Option<bool>,
    target_device_listed: Option<bool>,
    failure_stage: Option<RouteFailureStage>,
}

struct RouteBindingResult {
    observation: DefaultOutputRoute,
    retained_uid: Option<OwnedCfString>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CurrentDeviceObservation {
    status: i32,
    associated: bool,
    matches_default: Option<bool>,
    matches_target: Option<bool>,
    device_list_status: Option<i32>,
    device_listed: Option<bool>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct QueueStartResult {
    status: i32,
    queue_started: bool,
    failure_stage: Option<&'static str>,
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
    fn AudioObjectGetPropertyDataSize(
        object_id: u32,
        address: *const AudioObjectPropertyAddress,
        qualifier_data_size: u32,
        qualifier_data: *const c_void,
        out_data_size: *mut u32,
    ) -> i32;
}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFEqual(left: *const c_void, right: *const c_void) -> i8;
    fn CFRelease(value: *const c_void);
    fn CFStringGetCString(
        value: *const c_void,
        buffer: *mut c_char,
        buffer_size: isize,
        encoding: u32,
    ) -> u8;
}

struct OwnedCfString(*const c_void);

impl Drop for OwnedCfString {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { CFRelease(self.0) };
        }
    }
}

fn audio_device_uid(device_id: u32) -> std::result::Result<OwnedCfString, i32> {
    let uid_address = AudioObjectPropertyAddress {
        selector: AUDIO_DEVICE_PROPERTY_DEVICE_UID,
        scope: AUDIO_OBJECT_PROPERTY_SCOPE_GLOBAL,
        element: 0,
    };
    let mut uid: *const c_void = ptr::null();
    let mut size = std::mem::size_of_val(&uid) as u32;
    let status = unsafe {
        AudioObjectGetPropertyData(
            device_id,
            &uid_address,
            0,
            ptr::null(),
            &mut size,
            (&mut uid as *mut *const c_void).cast(),
        )
    };
    if status != 0 {
        return Err(status);
    }
    if uid.is_null() || size as usize != std::mem::size_of::<*const c_void>() {
        return Err(AUDIOQUEUE_ERR_INVALID_DEVICE);
    }
    Ok(OwnedCfString(uid))
}

fn audio_device_string(device_id: u32, selector: u32) -> std::result::Result<String, i32> {
    let address = AudioObjectPropertyAddress {
        selector,
        scope: AUDIO_OBJECT_PROPERTY_SCOPE_GLOBAL,
        element: 0,
    };
    let mut value: *const c_void = ptr::null();
    let mut size = std::mem::size_of_val(&value) as u32;
    let status = unsafe {
        AudioObjectGetPropertyData(
            device_id,
            &address,
            0,
            ptr::null(),
            &mut size,
            (&mut value as *mut *const c_void).cast(),
        )
    };
    if status != 0 {
        return Err(status);
    }
    if value.is_null() || size as usize != std::mem::size_of::<*const c_void>() {
        return Err(AUDIOQUEUE_ERR_INVALID_DEVICE);
    }
    let owned = OwnedCfString(value);
    let mut buffer = [0 as c_char; MAX_DEVICE_TEXT_BYTES];
    let converted = unsafe {
        CFStringGetCString(
            owned.0,
            buffer.as_mut_ptr(),
            buffer.len() as isize,
            CF_STRING_ENCODING_UTF8,
        )
    };
    if converted == 0 {
        return Err(AUDIOQUEUE_ERR_INVALID_DEVICE);
    }
    let text = unsafe { CStr::from_ptr(buffer.as_ptr()) }
        .to_str()
        .map_err(|_| AUDIOQUEUE_ERR_INVALID_DEVICE)?
        .to_owned();
    if text.is_empty() || text.len() >= MAX_DEVICE_TEXT_BYTES {
        return Err(AUDIOQUEUE_ERR_INVALID_DEVICE);
    }
    Ok(text)
}

fn audio_device_has_output(device_id: u32) -> bool {
    let address = AudioObjectPropertyAddress {
        selector: AUDIO_DEVICE_PROPERTY_STREAMS,
        scope: AUDIO_OBJECT_PROPERTY_SCOPE_OUTPUT,
        element: 0,
    };
    let mut size = 0_u32;
    unsafe {
        AudioObjectGetPropertyDataSize(device_id, &address, 0, ptr::null(), &mut size) == 0
            && size >= std::mem::size_of::<u32>() as u32
    }
}

fn enumerate_output_devices() -> Result<Vec<(String, String)>> {
    let (device_ids, count) = listed_audio_device_ids().map_err(|status| {
        MusicError::Output(format!("CoreAudio output-device list OSStatus {status}"))
    })?;
    let mut devices = Vec::with_capacity(count);
    for device_id in device_ids[..count].iter().copied() {
        if !audio_device_has_output(device_id) {
            continue;
        }
        let (Ok(uid), Ok(name)) = (
            audio_device_string(device_id, AUDIO_DEVICE_PROPERTY_DEVICE_UID),
            audio_device_string(device_id, AUDIO_OBJECT_PROPERTY_NAME),
        ) else {
            continue;
        };
        devices.push((uid, name));
    }
    Ok(devices)
}

fn choose_output_device_id(
    output_devices: &[(u32, String)],
    selected_uid: Option<&str>,
    default_id: Option<u32>,
) -> std::result::Result<u32, RouteFailureStage> {
    if let Some(selected_uid) = selected_uid {
        if selected_uid.is_empty()
            || selected_uid.len() >= MAX_DEVICE_TEXT_BYTES
            || selected_uid.chars().any(char::is_control)
        {
            return Err(RouteFailureStage::SelectedOutputNotListed);
        }
        return output_devices
            .iter()
            .find_map(|(id, uid)| (uid == selected_uid).then_some(*id))
            .ok_or(RouteFailureStage::SelectedOutputNotListed);
    }
    let default_id = default_id.ok_or(RouteFailureStage::DefaultOutputLookup)?;
    output_devices
        .iter()
        .find_map(|(id, _)| (*id == default_id).then_some(*id))
        .ok_or(RouteFailureStage::DefaultDeviceNotListed)
}

fn listed_audio_device_ids() -> std::result::Result<([u32; MAX_AUDIO_DEVICE_IDS], usize), i32> {
    let devices_address = AudioObjectPropertyAddress {
        selector: AUDIO_HARDWARE_PROPERTY_DEVICES,
        scope: AUDIO_OBJECT_PROPERTY_SCOPE_GLOBAL,
        element: 0,
    };
    let mut devices = [0_u32; MAX_AUDIO_DEVICE_IDS];
    let mut size = std::mem::size_of_val(&devices) as u32;
    let status = unsafe {
        AudioObjectGetPropertyData(
            AUDIO_OBJECT_SYSTEM_OBJECT_ID,
            &devices_address,
            0,
            ptr::null(),
            &mut size,
            devices.as_mut_ptr().cast(),
        )
    };
    if status != 0 {
        return Err(status);
    }
    if size as usize > std::mem::size_of_val(&devices)
        || (size as usize) % std::mem::size_of::<u32>() != 0
    {
        return Err(AUDIOQUEUE_ERR_INVALID_DEVICE);
    }
    Ok((devices, size as usize / std::mem::size_of::<u32>()))
}

fn resolve_output_device_uid(
    selected_uid: Option<&str>,
) -> std::result::Result<(u32, OwnedCfString, Option<bool>), RouteValidationFailure> {
    let (device_ids, count) =
        listed_audio_device_ids().map_err(|status| RouteValidationFailure {
            status,
            stage: RouteFailureStage::DeviceListEnumeration,
            device_list_status: Some(status),
            default_device_listed: None,
            target_device_listed: None,
        })?;
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
    if status == 0 && size as usize != std::mem::size_of::<u32>() {
        return Err(RouteValidationFailure {
            status: AUDIOQUEUE_ERR_INVALID_DEVICE,
            stage: RouteFailureStage::DefaultOutputLookup,
            device_list_status: Some(0),
            default_device_listed: None,
            target_device_listed: None,
        });
    }
    let output_devices = device_ids[..count]
        .iter()
        .copied()
        .filter(|device_id| audio_device_has_output(*device_id))
        .filter_map(|device_id| {
            audio_device_string(device_id, AUDIO_DEVICE_PROPERTY_DEVICE_UID)
                .ok()
                .map(|uid| (device_id, uid))
        })
        .collect::<Vec<_>>();
    let default_id = (status == 0 && default_device_id != 0).then_some(default_device_id);
    let default_device_listed = default_id
        .map(|id| output_devices.iter().any(|(device_id, _)| *device_id == id))
        .or_else(|| (status == 0).then_some(false));
    let target_id =
        choose_output_device_id(&output_devices, selected_uid, default_id).map_err(|stage| {
            RouteValidationFailure {
                status: if status != 0 && stage == RouteFailureStage::DefaultOutputLookup {
                    status
                } else {
                    AUDIOQUEUE_ERR_INVALID_DEVICE
                },
                stage,
                device_list_status: Some(0),
                default_device_listed,
                target_device_listed: Some(false),
            }
        })?;
    let target_uid = audio_device_uid(target_id).map_err(|uid_status| RouteValidationFailure {
        status: uid_status,
        stage: RouteFailureStage::DefaultDeviceUidLookup,
        device_list_status: Some(0),
        default_device_listed,
        target_device_listed: Some(true),
    })?;
    Ok((target_id, target_uid, default_device_listed))
}

fn current_device_uid_membership(device_uid: *const c_void) -> (Option<i32>, Option<bool>) {
    if device_uid.is_null() {
        return (None, None);
    }
    let (device_ids, count) = match listed_audio_device_ids() {
        Ok(devices) => devices,
        Err(status) => return (Some(status), None),
    };
    let mut first_error = None;
    for device_id in &device_ids[..count] {
        match audio_device_uid(*device_id) {
            Ok(uid) => {
                if unsafe { CFEqual(device_uid, uid.0) } != 0 {
                    return (Some(0), Some(true));
                }
            }
            Err(status) => {
                first_error.get_or_insert(status);
            }
        }
    }
    match first_error {
        Some(status) => (Some(status), None),
        None => (Some(0), Some(false)),
    }
}

fn current_device_matches_default(device_uid: *const c_void) -> Option<bool> {
    if device_uid.is_null() {
        return None;
    }
    let (_, default_uid, _) = resolve_output_device_uid(None).ok()?;
    Some(unsafe { CFEqual(device_uid, default_uid.0) != 0 })
}

fn current_device_matches_target(
    device_uid: *const c_void,
    target_uid: *const c_void,
) -> Option<bool> {
    if device_uid.is_null() || target_uid.is_null() {
        return None;
    }
    Some(unsafe { CFEqual(device_uid, target_uid) != 0 })
}

fn record_start(
    api: &dyn AudioQueueApi,
    queue: AudioQueueRef,
    target_uid: *const c_void,
    diagnostics: &AudioQueueDiagnostics,
) -> QueueStartResult {
    diagnostics.add(&diagnostics.start_attempts, 1);
    let start_status = api.start(queue);
    diagnostics.status(&diagnostics.start_status, start_status);
    let observation = api.current_device(queue, target_uid);
    diagnostics.set_current_device_observation(observation);
    if start_status != 0 {
        QueueStartResult {
            status: start_status,
            queue_started: false,
            failure_stage: Some("AudioQueueStart"),
        }
    } else if observation.status != 0 {
        QueueStartResult {
            status: observation.status,
            queue_started: true,
            failure_stage: Some("AudioQueue CurrentDevice lookup"),
        }
    } else if !observation.associated {
        QueueStartResult {
            status: AUDIOQUEUE_ERR_INVALID_DEVICE,
            queue_started: true,
            failure_stage: Some("AudioQueue has no associated CurrentDevice"),
        }
    } else if observation.device_list_status != Some(0) {
        QueueStartResult {
            status: observation
                .device_list_status
                .unwrap_or(AUDIOQUEUE_ERR_INVALID_DEVICE),
            queue_started: true,
            failure_stage: Some("CoreAudio output-device membership verification"),
        }
    } else if observation.device_listed != Some(true) {
        QueueStartResult {
            status: AUDIOQUEUE_ERR_INVALID_DEVICE,
            queue_started: true,
            failure_stage: Some("AudioQueue CurrentDevice UID is not in the CoreAudio device list"),
        }
    } else if observation.matches_target != Some(true) {
        QueueStartResult {
            status: AUDIOQUEUE_ERR_INVALID_DEVICE,
            queue_started: true,
            failure_stage: Some(
                "AudioQueue CurrentDevice does not match the selected output device",
            ),
        }
    } else {
        QueueStartResult {
            status: 0,
            queue_started: true,
            failure_stage: None,
        }
    }
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
    fn AudioQueueSetParameter(queue: AudioQueueRef, parameter: u32, value: f32) -> i32;
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
    fn route_to_default_output(
        &self,
        queue: AudioQueueRef,
        selected_uid: Option<&str>,
    ) -> RouteBindingResult;
    fn allocate_buffer(
        &self,
        queue: AudioQueueRef,
        bytes: u32,
        out: *mut AudioQueueBufferRef,
    ) -> i32;
    fn enqueue_buffer(&self, queue: AudioQueueRef, buffer: AudioQueueBufferRef) -> i32;
    fn start(&self, queue: AudioQueueRef) -> i32;
    fn current_device(
        &self,
        queue: AudioQueueRef,
        target_uid: *const c_void,
    ) -> CurrentDeviceObservation;
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

    fn route_to_default_output(
        &self,
        queue: AudioQueueRef,
        selected_uid: Option<&str>,
    ) -> RouteBindingResult {
        let (_, target_uid, default_device_listed) = match resolve_output_device_uid(selected_uid) {
            Ok(route) => route,
            Err(failure) => {
                return RouteBindingResult {
                    observation: DefaultOutputRoute {
                        status: failure.status,
                        device_list_status: failure.device_list_status,
                        default_device_listed: failure.default_device_listed,
                        target_device_listed: failure.target_device_listed,
                        failure_stage: Some(failure.stage),
                    },
                    retained_uid: None,
                };
            }
        };
        let route_status = unsafe {
            AudioQueueSetProperty(
                queue,
                QUEUE_PROPERTY_CURRENT_DEVICE,
                (&target_uid.0 as *const *const c_void).cast(),
                std::mem::size_of_val(&target_uid.0) as u32,
            )
        };
        let (status, failure_stage) = if route_status != 0 {
            (route_status, Some(RouteFailureStage::QueueDeviceBinding))
        } else {
            let volume_status =
                unsafe { AudioQueueSetParameter(queue, QUEUE_PARAMETER_VOLUME, 1.0) };
            (
                volume_status,
                (volume_status != 0).then_some(RouteFailureStage::QueueVolumeBinding),
            )
        };
        RouteBindingResult {
            observation: DefaultOutputRoute {
                status,
                device_list_status: Some(0),
                default_device_listed,
                target_device_listed: Some(true),
                failure_stage,
            },
            retained_uid: Some(target_uid),
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

    fn current_device(
        &self,
        queue: AudioQueueRef,
        target_uid: *const c_void,
    ) -> CurrentDeviceObservation {
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
        if status != 0 {
            return CurrentDeviceObservation {
                status,
                associated: false,
                matches_default: None,
                matches_target: None,
                device_list_status: None,
                device_listed: None,
            };
        }
        if size as usize != std::mem::size_of::<*const c_void>() {
            return CurrentDeviceObservation {
                status: AUDIOQUEUE_ERR_INVALID_DEVICE,
                associated: false,
                matches_default: None,
                matches_target: None,
                device_list_status: None,
                device_listed: None,
            };
        }
        let associated = !device_uid.is_null();
        let (device_list_status, device_listed) = current_device_uid_membership(device_uid);
        let matches_default = current_device_matches_default(device_uid);
        let matches_target = current_device_matches_target(device_uid, target_uid);
        if associated {
            unsafe { CFRelease(device_uid) };
        }
        CurrentDeviceObservation {
            status,
            associated,
            matches_default,
            matches_target,
            device_list_status,
            device_listed,
        }
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
    _route_uid: Option<OwnedCfString>,
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
    route_uid: Option<OwnedCfString>,
    selected_output_uid: Option<String>,
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
        let selected_output_uid =
            std::env::var_os(OUTPUT_DEVICE_UID_ENV).and_then(|value| match value.into_string() {
                Ok(value) if value.is_empty() => None,
                Ok(value) => Some(value),
                Err(_) => Some("\0".to_owned()),
            });
        Self {
            api: Arc::new(SystemAudioQueue),
            queue: ptr::null_mut(),
            context: None,
            route_uid: None,
            selected_output_uid,
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
    /// Enumerates the currently attached output-capable devices as (UID, name).
    /// UIDs are for local route selection and must not be logged or displayed.
    pub fn available_output_devices() -> Result<Vec<(String, String)>> {
        enumerate_output_devices()
    }

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
        self.diagnostics.reset_session();
        self.diagnostics_summary_emitted = false;
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
        let route_binding = if create_err == 0 {
            let binding = self
                .api
                .route_to_default_output(queue, self.selected_output_uid.as_deref());
            self.diagnostics.set_default_route(binding.observation);
            binding
        } else {
            RouteBindingResult {
                observation: DefaultOutputRoute {
                    status: 0,
                    device_list_status: None,
                    default_device_listed: None,
                    target_device_listed: None,
                    failure_stage: None,
                },
                retained_uid: None,
            }
        };
        self.route_uid = route_binding.retained_uid;
        let err = if create_err != 0 {
            create_err
        } else {
            route_binding.observation.status
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
                self.route_uid.take();
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
                route_binding
                    .observation
                    .failure_stage
                    .unwrap_or(RouteFailureStage::QueueDeviceBinding)
                    .description()
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
                self.route_uid.take();
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
        let target_uid = self.route_uid.as_ref().map_or(ptr::null(), |uid| uid.0);
        let start = record_start(&*self.api, self.queue, target_uid, &self.diagnostics);
        if start.status != 0 {
            return Err(MusicError::Output(format!(
                "{} OSStatus {}",
                start.failure_stage.unwrap_or("AudioQueue start validation"),
                start.status
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
        if self.diagnostics.enabled.load(Ordering::Relaxed) {
            self.diagnostics.add(
                &self.diagnostics.nonzero_samples,
                block.samples.iter().filter(|&&sample| sample != 0).count() as u64,
            );
            let peak = block
                .samples
                .iter()
                .map(|sample| sample.unsigned_abs())
                .max()
                .unwrap_or(0);
            self.diagnostics
                .peak_sample
                .fetch_max(peak.into(), Ordering::Relaxed);
        }
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
                let target_uid = self.route_uid.as_ref().map_or(ptr::null(), |uid| uid.0);
                let start = record_start(&*self.api, self.queue, target_uid, &self.diagnostics);
                self.started = start.queue_started;
                if start.status != 0 {
                    return Err(MusicError::Output(format!(
                        "{} OSStatus {}",
                        start.failure_stage.unwrap_or("AudioQueue start validation"),
                        start.status
                    )));
                }
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
            self.route_uid.take();
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
                _route_uid: self.route_uid.take(),
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
        route_device_list_status: Option<i32>,
        route_default_device_listed: Option<bool>,
        route_target_device_listed: Option<bool>,
        device_available: Option<bool>,
        fail_allocate_at: Option<usize>,
        allocate_calls: usize,
        enqueue_status: i32,
        start_status: i32,
        current_device_status: Option<i32>,
        current_device_associated: Option<bool>,
        current_device_matches_default: Option<bool>,
        current_device_matches_target: Option<bool>,
        current_device_list_status: Option<i32>,
        current_device_listed: Option<bool>,
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

        fn route_to_default_output(
            &self,
            _queue: AudioQueueRef,
            selected_uid: Option<&str>,
        ) -> RouteBindingResult {
            let state = self.0.lock().unwrap();
            let device_list_status = state.route_device_list_status.or(Some(0));
            let default_device_listed = if device_list_status == Some(0) {
                state.route_default_device_listed.or(Some(true))
            } else {
                None
            };
            let target_device_listed = if device_list_status == Some(0) {
                state.route_target_device_listed.or(Some(true))
            } else {
                None
            };
            let computed_status = if device_list_status != Some(0) {
                device_list_status.unwrap_or(FAKE_INVALID_DEVICE)
            } else if (selected_uid.is_none() && default_device_listed != Some(true))
                || target_device_listed != Some(true)
            {
                FAKE_INVALID_DEVICE
            } else {
                0
            };
            let status = state.route_status.unwrap_or(computed_status);
            let failure_stage = if selected_uid.is_some() && target_device_listed == Some(false) {
                Some(RouteFailureStage::SelectedOutputNotListed)
            } else if selected_uid.is_none() && default_device_listed == Some(false) {
                Some(RouteFailureStage::DefaultDeviceNotListed)
            } else if device_list_status != Some(0) {
                Some(RouteFailureStage::DeviceListEnumeration)
            } else if status != 0 {
                Some(RouteFailureStage::QueueDeviceBinding)
            } else {
                None
            };
            RouteBindingResult {
                observation: DefaultOutputRoute {
                    status,
                    device_list_status,
                    default_device_listed,
                    target_device_listed,
                    failure_stage,
                },
                retained_uid: None,
            }
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

        fn current_device(
            &self,
            _queue: AudioQueueRef,
            _target_uid: *const c_void,
        ) -> CurrentDeviceObservation {
            let state = self.0.lock().unwrap();
            CurrentDeviceObservation {
                status: state.current_device_status.unwrap_or(0),
                associated: state.current_device_associated.unwrap_or(true),
                matches_default: state.current_device_matches_default.or(Some(true)),
                matches_target: state.current_device_matches_target.or(Some(true)),
                device_list_status: state.current_device_list_status.or(Some(0)),
                device_listed: state.current_device_listed.or(Some(true)),
            }
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
        assert_eq!(snapshot.default_route_device_list_status, Some(0));
        assert_eq!(snapshot.default_route_device_listed, Some(true));
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
        assert_eq!(snapshot.nonzero_samples, 4);
        assert_eq!(snapshot.peak_sample, 14);
        assert_eq!(snapshot.start_attempts, 1);
        assert_eq!(snapshot.start_status, Some(0));
        assert_eq!(snapshot.current_device_status, Some(0));
        assert_eq!(snapshot.current_device_associated, Some(true));
        assert_eq!(snapshot.current_device_matches_default, Some(true));
        assert_eq!(snapshot.current_device_matches_target, Some(true));
        assert_eq!(snapshot.current_device_list_status, Some(0));
        assert_eq!(snapshot.current_device_listed, Some(true));
        assert_eq!(snapshot.completed_callbacks, 1);
        assert_eq!(snapshot.teardown_attempts, 1);
        assert_eq!(snapshot.stop_status, Some(0));
        assert_eq!(snapshot.drain_complete, Some(true));
        assert_eq!(snapshot.immediate_stop_status, None);
        assert_eq!(snapshot.dispose_status, Some(0));
        assert_eq!(
            sink.diagnostics.record(),
            "RUST_MUSIC_AUDIO_DIAG rate=44100 bits=16 channels=1 created=1 create_status=0 route_status=0 route_device_list_status=0 route_default_listed=1 route_target_listed=1 allocated=3 capacity_bytes=24576 allocation_status=0 enqueued=1 frames=4 nonzero_samples=4 peak_sample=14 starts=1 start_status=0 device_status=0 device_associated=1 device_default=1 device_target=1 device_list_status=0 device_listed=1 callbacks=1 teardown=1 stop_status=0 drained=1 immediate_stop_status=none dispose_status=0"
        );
    }

    #[test]
    fn persistent_sink_diagnostics_reset_for_each_open_session() {
        let _serial = SERIAL_TESTS.lock().unwrap();
        let (mut sink, _) = fake_sink();
        sink.set_diagnostics_enabled(true);

        sink.open(valid_format()).unwrap();
        sink.write(&PcmBlock {
            format: valid_format(),
            samples: vec![1, 2, 3, 4],
        })
        .unwrap();
        sink.finish().unwrap();
        let first = sink.diagnostics_snapshot();
        assert_eq!(first.enqueued_frames, 4);
        assert_eq!(first.completed_callbacks, 1);
        assert_eq!(first.teardown_attempts, 1);
        assert!(sink.diagnostics_summary_emitted);

        sink.open(valid_format()).unwrap();
        let reset = sink.diagnostics_snapshot();
        assert!(reset.enabled);
        assert_eq!(reset.output_creation_attempts, 1);
        assert_eq!(reset.enqueued_buffers, 0);
        assert_eq!(reset.enqueued_frames, 0);
        assert_eq!(reset.completed_callbacks, 0);
        assert_eq!(reset.teardown_attempts, 0);
        assert_eq!(reset.stop_status, None);
        assert_eq!(reset.default_route_status, Some(0));
        assert_eq!(reset.default_route_device_listed, Some(true));
        assert!(!sink.diagnostics_summary_emitted);

        sink.write(&PcmBlock {
            format: valid_format(),
            samples: vec![5, 6],
        })
        .unwrap();
        sink.finish().unwrap();
        let second = sink.diagnostics_snapshot();
        assert_eq!(second.enqueued_frames, 2);
        assert_eq!(second.enqueued_buffers, 1);
        assert_eq!(second.completed_callbacks, 1);
        assert_eq!(second.teardown_attempts, 1);
        let second_record = sink.diagnostics.record();
        assert!(second_record.contains("frames=2"));
        assert!(second_record.contains("callbacks=1"));
        assert!(second_record.contains("route_default_listed=1"));
        assert!(second_record.contains("route_target_listed=1"));
        assert!(!second_record.contains("frames=4"));
        assert!(sink.diagnostics_summary_emitted);
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
    fn current_device_query_failure_is_reported_and_stops_playback_start() {
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
        let error = sink.write(&one_sample()).unwrap_err().to_string();
        assert!(error.contains("AudioQueue CurrentDevice lookup"));
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

    #[test]
    fn default_output_must_be_listed_before_queue_binding() {
        let _serial = SERIAL_TESTS.lock().unwrap();
        let (mut sink, api) = fake_sink();
        sink.set_diagnostics_enabled(true);
        api.0.lock().unwrap().route_default_device_listed = Some(false);
        let error = sink.open(valid_format()).unwrap_err().to_string();
        assert!(error.contains("CoreAudio default output is not in the device list"));
        let snapshot = sink.diagnostics_snapshot();
        assert_eq!(snapshot.default_route_device_list_status, Some(0));
        assert_eq!(snapshot.default_route_device_listed, Some(false));
        assert_eq!(snapshot.default_route_status, Some(FAKE_INVALID_DEVICE));
        assert_eq!(snapshot.allocated_buffers, 0);
        assert!(sink.queue.is_null());
    }

    #[test]
    fn default_output_list_query_failure_stops_before_queue_binding() {
        let _serial = SERIAL_TESTS.lock().unwrap();
        let (mut sink, api) = fake_sink();
        sink.set_diagnostics_enabled(true);
        api.0.lock().unwrap().route_device_list_status = Some(-50);
        let error = sink.open(valid_format()).unwrap_err().to_string();
        assert!(error.contains("CoreAudio output-device list enumeration"));
        let snapshot = sink.diagnostics_snapshot();
        assert_eq!(snapshot.default_route_device_list_status, Some(-50));
        assert_eq!(snapshot.default_route_device_listed, None);
        assert_eq!(snapshot.default_route_status, Some(-50));
        assert_eq!(snapshot.allocated_buffers, 0);
        assert!(sink.queue.is_null());
    }

    #[test]
    fn current_device_uid_must_be_listed_before_pcm_start_is_accepted() {
        let _serial = SERIAL_TESTS.lock().unwrap();
        let (mut sink, api) = fake_sink();
        sink.set_diagnostics_enabled(true);
        sink.open(valid_format()).unwrap();
        api.0.lock().unwrap().current_device_listed = Some(false);
        let error = sink.write(&one_sample()).unwrap_err().to_string();
        assert!(error.contains("AudioQueue CurrentDevice UID is not in the CoreAudio device list"));
        let snapshot = sink.diagnostics_snapshot();
        assert_eq!(snapshot.default_route_device_listed, Some(true));
        assert_eq!(snapshot.current_device_list_status, Some(0));
        assert_eq!(snapshot.current_device_listed, Some(false));
        assert_eq!(snapshot.current_device_matches_default, Some(true));
        assert!(
            sink.started,
            "successful AudioQueueStart must still be stopped during cleanup"
        );
        sink.finish().unwrap();
    }

    #[test]
    fn current_device_must_match_selected_output_target() {
        let _serial = SERIAL_TESTS.lock().unwrap();
        let (mut sink, api) = fake_sink();
        sink.set_diagnostics_enabled(true);
        sink.open(valid_format()).unwrap();
        api.0.lock().unwrap().current_device_matches_target = Some(false);
        let error = sink.write(&one_sample()).unwrap_err().to_string();
        assert!(error.contains("does not match the selected output device"));
        let snapshot = sink.diagnostics_snapshot();
        assert_eq!(snapshot.current_device_matches_target, Some(false));
        assert_eq!(snapshot.current_device_listed, Some(true));
        assert!(sink.started);
        sink.finish().unwrap();
    }

    #[test]
    fn selected_output_uid_may_target_a_non_default_listed_output() {
        let outputs = vec![(10, "Built-in".to_owned()), (20, "USB-DAC".to_owned())];
        assert_eq!(
            choose_output_device_id(&outputs, Some("USB-DAC"), Some(10)),
            Ok(20)
        );
        assert_eq!(choose_output_device_id(&outputs, None, Some(10)), Ok(10));
        assert_eq!(
            choose_output_device_id(&outputs, Some("removed-device"), Some(10)),
            Err(RouteFailureStage::SelectedOutputNotListed)
        );
        assert_eq!(
            choose_output_device_id(&outputs, None, Some(99)),
            Err(RouteFailureStage::DefaultDeviceNotListed)
        );
        assert_eq!(
            choose_output_device_id(&outputs, Some("bad\nuid"), Some(10)),
            Err(RouteFailureStage::SelectedOutputNotListed)
        );
    }

    #[test]
    fn selected_target_is_accepted_even_when_not_the_system_default() {
        let _serial = SERIAL_TESTS.lock().unwrap();
        let (mut sink, api) = fake_sink();
        sink.set_diagnostics_enabled(true);
        sink.selected_output_uid = Some("external-output-uid".to_owned());
        {
            let mut state = api.0.lock().unwrap();
            state.route_default_device_listed = Some(false);
            state.current_device_matches_default = Some(false);
        }
        sink.open(valid_format()).unwrap();
        sink.write(&one_sample()).unwrap();
        let snapshot = sink.diagnostics_snapshot();
        assert_eq!(snapshot.default_route_device_listed, Some(false));
        assert_eq!(snapshot.route_target_device_listed, Some(true));
        assert_eq!(snapshot.current_device_matches_default, Some(false));
        assert_eq!(snapshot.current_device_matches_target, Some(true));
        sink.finish().unwrap();
    }
}
