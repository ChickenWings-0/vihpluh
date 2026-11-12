//! Thin safe wrapper over the libmpv client API. This is the only place (together with the
//! video backends and media services' render contexts) that calls libmpv directly.

use std::ffi::{CStr, CString, c_char, c_int, c_void};
use std::ptr::{self, NonNull};

use libmpv2_sys as sys;

#[derive(Debug, Clone, thiserror::Error)]
pub enum MpvError {
    #[error("mpv could not be created")]
    Create,
    #[error("{message} (mpv error {code})")]
    Api { code: i32, message: String },
    #[error("text passed to mpv contains a NUL byte")]
    Nul,
}

pub type MpvResult<T> = Result<T, MpvError>;

fn check(code: c_int) -> MpvResult<()> {
    if code >= 0 {
        return Ok(());
    }
    // SAFETY: mpv_error_string returns a static string for any code.
    let message = unsafe { CStr::from_ptr(sys::mpv_error_string(code)) }.to_string_lossy();
    Err(MpvError::Api { code, message: message.into_owned() })
}

fn cstring(text: &str) -> MpvResult<CString> {
    CString::new(text).map_err(|_| MpvError::Nul)
}

/// A value in mpv's generic node format.
#[derive(Debug, Clone, PartialEq)]
pub enum Node {
    None,
    String(String),
    Flag(bool),
    Int(i64),
    Double(f64),
    Array(Vec<Node>),
    Map(Vec<(String, Node)>),
}

impl Node {
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Flag(b) => Some(*b),
            _ => None,
        }
    }

    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Self::Int(i) => Some(*i),
            Self::Double(d) if d.fract() == 0.0 => Some(*d as i64),
            _ => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Self::Double(d) => Some(*d),
            Self::Int(i) => Some(*i as f64),
            _ => None,
        }
    }

    pub fn as_array(&self) -> Option<&[Node]> {
        match self {
            Self::Array(items) => Some(items),
            _ => None,
        }
    }

    /// Looks up a key in a map node.
    pub fn get(&self, key: &str) -> Option<&Node> {
        match self {
            Self::Map(entries) => entries.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    pub fn str_field(&self, key: &str) -> Option<&str> {
        self.get(key).and_then(Node::as_str)
    }

    pub fn i64_field(&self, key: &str) -> Option<i64> {
        self.get(key).and_then(Node::as_i64)
    }

    pub fn f64_field(&self, key: &str) -> Option<f64> {
        self.get(key).and_then(Node::as_f64)
    }

    pub fn bool_field(&self, key: &str) -> Option<bool> {
        self.get(key).and_then(Node::as_bool)
    }

    /// # Safety
    /// `node` must point to a valid `mpv_node` produced by libmpv.
    unsafe fn from_raw(node: *const sys::mpv_node) -> Self {
        // SAFETY: the caller guarantees `node` is valid; the union member read is selected by
        // `format`, as the API requires.
        unsafe {
            let node = &*node;
            match node.format {
                sys::mpv_format_MPV_FORMAT_STRING => Self::String(lossy(node.u.string)),
                sys::mpv_format_MPV_FORMAT_FLAG => Self::Flag(node.u.flag != 0),
                sys::mpv_format_MPV_FORMAT_INT64 => Self::Int(node.u.int64),
                sys::mpv_format_MPV_FORMAT_DOUBLE => Self::Double(node.u.double_),
                sys::mpv_format_MPV_FORMAT_NODE_ARRAY => {
                    let list = &*node.u.list;
                    let count = usize::try_from(list.num).unwrap_or(0);
                    Self::Array((0..count).map(|i| Self::from_raw(list.values.add(i))).collect())
                }
                sys::mpv_format_MPV_FORMAT_NODE_MAP => {
                    let list = &*node.u.list;
                    let count = usize::try_from(list.num).unwrap_or(0);
                    Self::Map(
                        (0..count)
                            .map(|i| (lossy(*list.keys.add(i)), Self::from_raw(list.values.add(i))))
                            .collect(),
                    )
                }
                _ => Self::None,
            }
        }
    }
}

/// # Safety
/// `text` must be null or a valid NUL-terminated string.
unsafe fn lossy(text: *const c_char) -> String {
    if text.is_null() {
        return String::new();
    }
    // SAFETY: checked non-null; the caller guarantees NUL termination.
    unsafe { CStr::from_ptr(text) }.to_string_lossy().into_owned()
}

/// Property formats vihpluh observes or reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Node,
    Double,
    Flag,
    Int,
    String,
}

impl Format {
    fn raw(self) -> sys::mpv_format {
        match self {
            Self::Node => sys::mpv_format_MPV_FORMAT_NODE,
            Self::Double => sys::mpv_format_MPV_FORMAT_DOUBLE,
            Self::Flag => sys::mpv_format_MPV_FORMAT_FLAG,
            Self::Int => sys::mpv_format_MPV_FORMAT_INT64,
            Self::String => sys::mpv_format_MPV_FORMAT_STRING,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndReason {
    Eof,
    Stop,
    Quit,
    Error,
    Redirect,
    Other,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    /// The wait timed out or was interrupted by `wakeup`.
    None,
    Shutdown,
    LogMessage {
        prefix: String,
        level: String,
        text: String,
    },
    StartFile,
    FileLoaded,
    EndFile {
        reason: EndReason,
        /// mpv's error text when `reason` is `Error`.
        error: Option<String>,
    },
    Seek,
    PlaybackRestart,
    VideoReconfig,
    AudioReconfig,
    /// `value` is `Node::None` when the property is unavailable.
    PropertyChange {
        id: u64,
        name: String,
        value: Node,
    },
    /// Reply to an asynchronous command or property write that failed.
    AsyncError {
        id: u64,
        error: String,
    },
    Other,
}

/// An owned libmpv client handle. All methods are safe to call from any thread, except
/// [`Mpv::wait_event`], which must only ever be called from one thread at a time.
pub struct Mpv {
    ctx: NonNull<sys::mpv_handle>,
}

// SAFETY: the libmpv client API is documented as thread-safe for a single handle.
unsafe impl Send for Mpv {}
// SAFETY: as above.
unsafe impl Sync for Mpv {}

impl Mpv {
    /// Creates an uninitialised handle; set options, then call [`Mpv::initialize`].
    pub fn create() -> MpvResult<Self> {
        // SAFETY: mpv_create has no preconditions.
        let ctx = unsafe { sys::mpv_create() };
        NonNull::new(ctx).map(|ctx| Self { ctx }).ok_or(MpvError::Create)
    }

    pub fn raw(&self) -> *mut sys::mpv_handle {
        self.ctx.as_ptr()
    }

    pub fn set_option(&self, name: &str, value: &str) -> MpvResult<()> {
        let (name, value) = (cstring(name)?, cstring(value)?);
        // SAFETY: valid handle and NUL-terminated strings.
        check(unsafe { sys::mpv_set_option_string(self.raw(), name.as_ptr(), value.as_ptr()) })
    }

    pub fn initialize(&self) -> MpvResult<()> {
        // SAFETY: valid, not yet initialised handle.
        check(unsafe { sys::mpv_initialize(self.raw()) })
    }

    pub fn request_log_messages(&self, level: &str) -> MpvResult<()> {
        let level = cstring(level)?;
        // SAFETY: valid handle and NUL-terminated string.
        check(unsafe { sys::mpv_request_log_messages(self.raw(), level.as_ptr()) })
    }

    fn with_args<T>(args: &[&str], f: impl FnOnce(*mut *const c_char) -> T) -> MpvResult<T> {
        let owned: Vec<CString> = args.iter().map(|a| cstring(a)).collect::<MpvResult<_>>()?;
        let mut pointers: Vec<*const c_char> = owned.iter().map(|a| a.as_ptr()).collect();
        pointers.push(ptr::null());
        Ok(f(pointers.as_mut_ptr()))
    }

    /// Runs a command and waits for it to complete.
    pub fn command(&self, args: &[&str]) -> MpvResult<()> {
        // SAFETY: `pointers` is a NULL-terminated array of valid C strings that outlives the call.
        check(Self::with_args(args, |argv| unsafe { sys::mpv_command(self.raw(), argv) })?)
    }

    /// Queues a command without waiting. A failure arrives later as [`Event::AsyncError`]
    /// carrying `id`.
    pub fn command_async(&self, id: u64, args: &[&str]) -> MpvResult<()> {
        // SAFETY: as in `command`; mpv copies the arguments before returning.
        check(Self::with_args(args, |argv| unsafe {
            sys::mpv_command_async(self.raw(), id, argv)
        })?)
    }

    pub fn set_string(&self, name: &str, value: &str) -> MpvResult<()> {
        let (name, value) = (cstring(name)?, cstring(value)?);
        // SAFETY: valid handle and NUL-terminated strings.
        check(unsafe { sys::mpv_set_property_string(self.raw(), name.as_ptr(), value.as_ptr()) })
    }

    pub fn set_f64(&self, name: &str, value: f64) -> MpvResult<()> {
        let name = cstring(name)?;
        let mut value = value;
        // SAFETY: `value` is a valid f64 for MPV_FORMAT_DOUBLE; mpv copies it.
        check(unsafe {
            sys::mpv_set_property(
                self.raw(),
                name.as_ptr(),
                sys::mpv_format_MPV_FORMAT_DOUBLE,
                (&raw mut value).cast::<c_void>(),
            )
        })
    }

    pub fn set_i64(&self, name: &str, value: i64) -> MpvResult<()> {
        let name = cstring(name)?;
        let mut value = value;
        // SAFETY: `value` is a valid i64 for MPV_FORMAT_INT64; mpv copies it.
        check(unsafe {
            sys::mpv_set_property(
                self.raw(),
                name.as_ptr(),
                sys::mpv_format_MPV_FORMAT_INT64,
                (&raw mut value).cast::<c_void>(),
            )
        })
    }

    pub fn set_bool(&self, name: &str, value: bool) -> MpvResult<()> {
        let name = cstring(name)?;
        let mut flag: c_int = c_int::from(value);
        // SAFETY: `flag` is a valid int for MPV_FORMAT_FLAG; mpv copies it.
        check(unsafe {
            sys::mpv_set_property(
                self.raw(),
                name.as_ptr(),
                sys::mpv_format_MPV_FORMAT_FLAG,
                (&raw mut flag).cast::<c_void>(),
            )
        })
    }

    pub fn get_string(&self, name: &str) -> Option<String> {
        let name = cstring(name).ok()?;
        // SAFETY: valid handle and name; a non-null result is an mpv-allocated string that we
        // copy and then free with mpv_free.
        unsafe {
            let raw = sys::mpv_get_property_string(self.raw(), name.as_ptr());
            if raw.is_null() {
                return None;
            }
            let text = lossy(raw);
            sys::mpv_free(raw.cast::<c_void>());
            Some(text)
        }
    }

    pub fn get_f64(&self, name: &str) -> Option<f64> {
        let name = cstring(name).ok()?;
        let mut value = 0.0f64;
        // SAFETY: `value` is a valid out-pointer for MPV_FORMAT_DOUBLE.
        let code = unsafe {
            sys::mpv_get_property(
                self.raw(),
                name.as_ptr(),
                sys::mpv_format_MPV_FORMAT_DOUBLE,
                (&raw mut value).cast::<c_void>(),
            )
        };
        (code >= 0).then_some(value)
    }

    pub fn get_i64(&self, name: &str) -> Option<i64> {
        let name = cstring(name).ok()?;
        let mut value = 0i64;
        // SAFETY: `value` is a valid out-pointer for MPV_FORMAT_INT64.
        let code = unsafe {
            sys::mpv_get_property(
                self.raw(),
                name.as_ptr(),
                sys::mpv_format_MPV_FORMAT_INT64,
                (&raw mut value).cast::<c_void>(),
            )
        };
        (code >= 0).then_some(value)
    }

    pub fn get_bool(&self, name: &str) -> Option<bool> {
        let name = cstring(name).ok()?;
        let mut flag: c_int = 0;
        // SAFETY: `flag` is a valid out-pointer for MPV_FORMAT_FLAG.
        let code = unsafe {
            sys::mpv_get_property(
                self.raw(),
                name.as_ptr(),
                sys::mpv_format_MPV_FORMAT_FLAG,
                (&raw mut flag).cast::<c_void>(),
            )
        };
        (code >= 0).then_some(flag != 0)
    }

    pub fn get_node(&self, name: &str) -> Option<Node> {
        let name = cstring(name).ok()?;
        // SAFETY: `node` is a valid out-pointer for MPV_FORMAT_NODE; on success mpv fills it and
        // we release its contents with mpv_free_node_contents after copying.
        unsafe {
            let mut node = std::mem::zeroed::<sys::mpv_node>();
            let code = sys::mpv_get_property(
                self.raw(),
                name.as_ptr(),
                sys::mpv_format_MPV_FORMAT_NODE,
                (&raw mut node).cast::<c_void>(),
            );
            if code < 0 {
                return None;
            }
            let value = Node::from_raw(&raw const node);
            sys::mpv_free_node_contents(&raw mut node);
            Some(value)
        }
    }

    /// Subscribes to changes of a property; they arrive as [`Event::PropertyChange`] with `id`.
    pub fn observe(&self, id: u64, name: &str, format: Format) -> MpvResult<()> {
        let name = cstring(name)?;
        // SAFETY: valid handle and NUL-terminated name.
        check(unsafe { sys::mpv_observe_property(self.raw(), id, name.as_ptr(), format.raw()) })
    }

    /// Interrupts a blocking [`Mpv::wait_event`] on another thread.
    pub fn wakeup(&self) {
        // SAFETY: valid handle; explicitly safe from any thread.
        unsafe { sys::mpv_wakeup(self.raw()) }
    }

    /// Waits up to `timeout` seconds for the next event. Call from one thread only.
    pub fn wait_event(&self, timeout: f64) -> Event {
        // SAFETY: the returned pointer is valid until the next wait_event call on this handle,
        // and every payload is converted to owned data before returning.
        unsafe {
            let event = &*sys::mpv_wait_event(self.raw(), timeout);
            match event.event_id {
                sys::mpv_event_id_MPV_EVENT_NONE => Event::None,
                sys::mpv_event_id_MPV_EVENT_SHUTDOWN => Event::Shutdown,
                sys::mpv_event_id_MPV_EVENT_LOG_MESSAGE => {
                    let message = &*event.data.cast::<sys::mpv_event_log_message>();
                    Event::LogMessage {
                        prefix: lossy(message.prefix),
                        level: lossy(message.level),
                        text: lossy(message.text).trim_end().to_string(),
                    }
                }
                sys::mpv_event_id_MPV_EVENT_START_FILE => Event::StartFile,
                sys::mpv_event_id_MPV_EVENT_FILE_LOADED => Event::FileLoaded,
                sys::mpv_event_id_MPV_EVENT_END_FILE => {
                    let end = &*event.data.cast::<sys::mpv_event_end_file>();
                    let reason = match end.reason {
                        sys::mpv_end_file_reason_MPV_END_FILE_REASON_EOF => EndReason::Eof,
                        sys::mpv_end_file_reason_MPV_END_FILE_REASON_STOP => EndReason::Stop,
                        sys::mpv_end_file_reason_MPV_END_FILE_REASON_QUIT => EndReason::Quit,
                        sys::mpv_end_file_reason_MPV_END_FILE_REASON_ERROR => EndReason::Error,
                        sys::mpv_end_file_reason_MPV_END_FILE_REASON_REDIRECT => {
                            EndReason::Redirect
                        }
                        _ => EndReason::Other,
                    };
                    let error = (reason == EndReason::Error)
                        .then(|| lossy(sys::mpv_error_string(end.error)));
                    Event::EndFile { reason, error }
                }
                sys::mpv_event_id_MPV_EVENT_SEEK => Event::Seek,
                sys::mpv_event_id_MPV_EVENT_PLAYBACK_RESTART => Event::PlaybackRestart,
                sys::mpv_event_id_MPV_EVENT_VIDEO_RECONFIG => Event::VideoReconfig,
                sys::mpv_event_id_MPV_EVENT_AUDIO_RECONFIG => Event::AudioReconfig,
                sys::mpv_event_id_MPV_EVENT_PROPERTY_CHANGE => {
                    let property = &*event.data.cast::<sys::mpv_event_property>();
                    Event::PropertyChange {
                        id: event.reply_userdata,
                        name: lossy(property.name),
                        value: property_value(property),
                    }
                }
                sys::mpv_event_id_MPV_EVENT_COMMAND_REPLY
                | sys::mpv_event_id_MPV_EVENT_SET_PROPERTY_REPLY
                    if event.error < 0 =>
                {
                    Event::AsyncError {
                        id: event.reply_userdata,
                        error: lossy(sys::mpv_error_string(event.error)),
                    }
                }
                _ => Event::Other,
            }
        }
    }
}

/// # Safety
/// `property` must come from a live `MPV_EVENT_PROPERTY_CHANGE` event.
unsafe fn property_value(property: &sys::mpv_event_property) -> Node {
    if property.data.is_null() {
        return Node::None;
    }
    // SAFETY: `data` points to a value of the type selected by `format`, per the client API.
    unsafe {
        match property.format {
            sys::mpv_format_MPV_FORMAT_NODE => {
                Node::from_raw(property.data.cast::<sys::mpv_node>())
            }
            sys::mpv_format_MPV_FORMAT_DOUBLE => Node::Double(*property.data.cast::<f64>()),
            sys::mpv_format_MPV_FORMAT_FLAG => Node::Flag(*property.data.cast::<c_int>() != 0),
            sys::mpv_format_MPV_FORMAT_INT64 => Node::Int(*property.data.cast::<i64>()),
            sys::mpv_format_MPV_FORMAT_STRING => {
                Node::String(lossy(*property.data.cast::<*const c_char>()))
            }
            _ => Node::None,
        }
    }
}

impl Drop for Mpv {
    fn drop(&mut self) {
        // SAFETY: we own the handle; this blocks until the core has shut down.
        unsafe { sys::mpv_terminate_destroy(self.raw()) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn node_accessors() {
        let node = Node::Map(vec![
            ("id".into(), Node::Int(2)),
            ("lang".into(), Node::String("eng".into())),
            ("default".into(), Node::Flag(true)),
            ("fps".into(), Node::Double(23.976)),
        ]);
        assert_eq!(node.i64_field("id"), Some(2));
        assert_eq!(node.str_field("lang"), Some("eng"));
        assert_eq!(node.bool_field("default"), Some(true));
        assert_eq!(node.f64_field("fps"), Some(23.976));
        assert_eq!(node.f64_field("id"), Some(2.0), "ints widen to floats");
        assert_eq!(node.i64_field("fps"), None, "fractional floats are not ints");
        assert_eq!(node.str_field("missing"), None);
        assert_eq!(Node::Array(vec![Node::Int(1)]).as_array().map(<[Node]>::len), Some(1));
    }

    #[test]
    fn strings_with_nul_are_rejected() {
        assert!(matches!(cstring("a\0b"), Err(MpvError::Nul)));
    }

    /// Exercises the real library end to end: create, initialise headless, read versions.
    #[test]
    fn headless_handle_reports_versions() {
        let mpv = Mpv::create().unwrap();
        mpv.set_option("vo", "null").unwrap();
        mpv.set_option("ao", "null").unwrap();
        mpv.set_option("config", "no").unwrap();
        mpv.initialize().unwrap();
        let version = mpv.get_string("mpv-version").unwrap();
        assert!(version.contains("mpv"), "{version}");
        assert!(mpv.get_string("ffmpeg-version").is_some());
        assert_eq!(mpv.get_bool("idle-active"), Some(true), "libmpv idles by default");
        assert!(mpv.set_option("definitely-not-an-option", "1").is_err());
        assert!(matches!(mpv.wait_event(0.0), Event::None | Event::Other));
    }
}
