//! Log implementation

use ::core::{
    cell::Cell,
    fmt,
    sync::atomic::{AtomicBool, Ordering},
};
use ::std::{io::Write as _, sync::LazyLock};

use ::bytes::{Buf, BufMut, Bytes, BytesMut};
use ::log::{Level, Log};
use ::serde::{Deserialize, Serialize};
use ::spel_katalog_formats::Timestamp;

/// Get length of format arguments.
fn count_arg_len(args: ::core::fmt::Arguments) -> usize {
    struct Counter(usize);
    impl ::core::fmt::Write for Counter {
        fn write_str(&mut self, s: &str) -> ::core::fmt::Result {
            self.0 += s.len();
            Ok(())
        }
    }
    let mut c = Counter(0);
    _ = ::core::fmt::Write::write_fmt(&mut c, args);
    c.0
}

/// Get bytes from [Buf] where length is given as a leading u64.
fn get_sized_bytes(buf: &mut impl Buf) -> Bytes {
    let len = buf.get_u64() as usize;
    buf.copy_to_bytes(len)
}

/// Write a slice to a [BufMut], first writing the length.
fn put_sized_bytes(buf_mut: &mut impl BufMut, bytes: &[u8]) {
    buf_mut.put_u64(bytes.len() as u64);
    buf_mut.put_slice(bytes);
}

/// Write format arguments to a [BufMut], with length prepended.
fn put_fmt_args(buf_mut: &mut (impl BufMut + AsMut<[u8]>), args: fmt::Arguments<'_>) {
    let off = buf_mut.as_mut().len();
    buf_mut.put_u64(0);

    let mut writer = BufMut::writer(buf_mut);
    writer
        .write_fmt(args)
        .expect("writes to BufMut should always succeed");

    let buf_mut = writer.into_inner();
    let len = buf_mut.as_mut().len() - off - size_of::<u64>();

    (&mut buf_mut.as_mut()[off..]).put_u64(len as u64);
}

/// Channel in use by log.
static CHANNEL: LazyLock<(
    ::flume::Sender<RecordMessage>,
    ::flume::Receiver<RecordMessage>,
)> = LazyLock::new(::flume::unbounded);

/// Stable location of logger.
static LOGGER: ChannelLog = ChannelLog;

/// Is the logger in use.
static IN_USE: AtomicBool = AtomicBool::new(false);

/// Initialize log.
///
/// # Panics
/// If the log cannot be initialized.
pub fn init() {
    if let Err(err) = ::log::set_logger(&LOGGER) {
        panic!("could not initialize log, {err}")
    } else {
        IN_USE.store(true, Ordering::Relaxed);
        ::log::set_max_level(::log::LevelFilter::Info);
    };
}

/// Get a receiver for log records.
///
/// The receiver is a mpmc receiver, and
/// as such might not be the only one receiving
/// messages.
pub fn receiver() -> &'static ::flume::Receiver<RecordMessage> {
    let (_, rx) = &*CHANNEL;
    rx
}

/// Is the custom logger installed.
pub fn is_installed() -> bool {
    thread_local! {
        static IS_INSTALLED: Cell<bool> = Cell::new(IN_USE.load(Ordering::Relaxed));
    }

    if IS_INSTALLED.get() {
        true
    } else {
        let value = IN_USE.load(Ordering::Relaxed);
        IS_INSTALLED.set(value);
        value
    }
}

/// Message sent on channel.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OwnedRecord {
    /// Log level of record.
    pub level: Level, // 1
    /// When the log was created.
    pub timestamp: Timestamp, // 8
    /// Line log occurred on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line: Option<u32>, // 5
    /// Message of record.
    pub message: Bytes, // 8 + n
    /// Target of record.
    pub target: Bytes, // 8 + n
    /// Path to module of logging.
    pub module: Bytes, // 8 + n
    /// Path to file of logging.
    pub file: Bytes, // 8 + n
}

impl OwnedRecord {
    /// Get first line of message.
    pub fn first_line(&self) -> Bytes {
        if let Some(off) = ::memchr::memchr(b'\n', &self.message) {
            self.message.slice(..off)
        } else {
            self.message.clone()
        }
    }
}

/// Record packed as a message.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RecordMessage {
    /// Message packed as bytes.
    /// 1   | level
    /// 8   | timestamp
    /// 1   | has line
    /// 4/0 | line
    /// 8   | message len
    /// n   | message
    /// 8   | target len
    /// n   | target
    /// 8   | module len
    /// n   | module
    /// 8   | file len
    /// n   | file
    inner: Bytes,
}

impl From<&::log::Record<'_>> for RecordMessage {
    fn from(record: &::log::Record<'_>) -> Self {
        let mut buf = BytesMut::with_capacity(
            count_arg_len(*record.args())
                + record.target().len()
                + record.file().map_or_default(|f| f.len())
                + record.module_path().map_or_default(|m| m.len())
                + 42 // 8 * 5 + 1 + 1
                + if record.line().is_some() { 4 } else { 0 },
        );

        buf.put_u8(match record.level() {
            Level::Error => 1,
            Level::Warn => 2,
            Level::Info => 3,
            Level::Debug => 4,
            Level::Trace => 5,
        });

        buf.put_i64(Timestamp::now().into());

        if let Some(line) = record.line() {
            buf.put_u32(line);
        } else {
            buf.put_u32(u32::MAX);
        }

        put_fmt_args(&mut buf, *record.args());
        put_sized_bytes(&mut buf, record.target().as_bytes());
        put_sized_bytes(&mut buf, record.module_path().unwrap_or("").as_bytes());
        put_sized_bytes(&mut buf, record.file().unwrap_or("").as_bytes());

        RecordMessage {
            inner: buf.freeze(),
        }
    }
}

impl From<RecordMessage> for OwnedRecord {
    fn from(message: RecordMessage) -> Self {
        let RecordMessage { mut inner } = message;

        let level = match inner.get_u8() {
            1 => Level::Error,
            2 => Level::Warn,
            3 => Level::Info,
            4 => Level::Debug,
            5 => Level::Trace,
            other => panic!("unknown log level {other}"),
        };

        let timestamp = Timestamp::try_from(inner.get_i64()).expect("timestamp should be valid");

        let line = inner.get_u32();
        let line = if line == u32::MAX { None } else { Some(line) };

        let message = get_sized_bytes(&mut inner);
        let target = get_sized_bytes(&mut inner);
        let module = get_sized_bytes(&mut inner);
        let file = get_sized_bytes(&mut inner);

        OwnedRecord {
            level,
            timestamp,
            line,
            message,
            target,
            module,
            file,
        }
    }
}

impl From<&::log::Record<'_>> for OwnedRecord {
    fn from(value: &::log::Record<'_>) -> Self {
        OwnedRecord::from(RecordMessage::from(value))
    }
}

/// Internal log implementor.
struct ChannelLog;

impl Log for ChannelLog {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        metadata.level() <= Level::Info
    }

    #[expect(clippy::print_stderr, reason = "no alternative")]
    fn log(&self, record: &log::Record) {
        if self.enabled(record.metadata()) {
            let (tx, _) = &*CHANNEL;
            if let Err(err) = tx.send(record.into()) {
                eprintln!("failed to log: {:?}", err.into_inner());
            };
        }
    }

    /// Flusing is a no-op.
    fn flush(&self) {}
}
