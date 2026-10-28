//! Tiny, independently authored native audio shims. No CPAL/Rodio/libFLAC.
//! Mac AudioQueue and Linux ALSA wrap OS-provided PCM devices, not codecs.
#![deny(unsafe_op_in_unsafe_fn)]

#[cfg(all(target_os = "linux", feature = "alsa"))]
mod alsa;
#[cfg(all(target_os = "linux", feature = "alsa"))]
pub use alsa::AlsaSink;

#[cfg(all(target_os = "macos", feature = "macos"))]
mod macos;
#[cfg(all(target_os = "macos", feature = "macos"))]
pub use macos::MacAudioQueueSink;
