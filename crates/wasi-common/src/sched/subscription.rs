use crate::clocks::WasiMonotonicClock;
use crate::file::{FileCaps, FileEntryMutExt, TableFileExt, WasiFile};
use crate::table::Table;
use crate::Error;
use bitflags::bitflags;
use cap_std::time::{Duration, Instant};
use std::cell::{Cell, RefMut};

bitflags! {
    pub struct RwEventFlags: u32 {
        const HANGUP = 0b1;
    }
}

pub struct RwSubscription<'a> {
    table: &'a Table,
    fd: u32,
    status: Cell<Option<Result<(u64, RwEventFlags), Error>>>,
}

impl<'a> RwSubscription<'a> {
    /// Create an RwSubscription. This constructor checks to make sure the file we need exists, and
    /// has the correct rights. But, we can't hold onto the WasiFile RefMut inside this structure
    /// (Pat can't convince borrow checker, either not clever enough or a rustc bug), so we need to
    /// re-borrow at use time.
    pub fn new(table: &'a Table, fd: u32) -> Result<Self, Error> {
        let _ = table.get_file_mut(fd)?.get_cap(FileCaps::POLL_READWRITE)?;
        Ok(Self {
            table,
            fd,
            status: Cell::new(None),
        })
    }
    /// This accessor could fail if there is an outstanding borrow of the file.
    pub fn file(&self) -> Result<RefMut<'a, dyn WasiFile>, Error> {
        self.table
            .get_file_mut(self.fd)?
            .get_cap(FileCaps::POLL_READWRITE)
    }
    pub fn complete(&self, size: u64, flags: RwEventFlags) {
        self.status.set(Some(Ok((size, flags))))
    }
    pub fn error(&self, error: Error) {
        self.status.set(Some(Err(error)))
    }
    pub fn result(self) -> Option<Result<(u64, RwEventFlags), Error>> {
        self.status.into_inner()
    }
}

pub struct MonotonicClockSubscription<'a> {
    pub clock: &'a dyn WasiMonotonicClock,
    pub deadline: Instant,
    pub precision: Duration,
}

impl<'a> MonotonicClockSubscription<'a> {
    pub fn now(&self) -> Instant {
        self.clock.now(self.precision)
    }
    pub fn duration_until(&self) -> Option<Duration> {
        self.deadline.checked_duration_since(self.now())
    }
    pub fn result(&self) -> Option<Result<(), Error>> {
        if self.now().checked_duration_since(self.deadline).is_some() {
            Some(Ok(()))
        } else {
            None
        }
    }
}

pub enum Subscription<'a> {
    Read(RwSubscription<'a>),
    Write(RwSubscription<'a>),
    MonotonicClock(MonotonicClockSubscription<'a>),
}

pub enum SubscriptionResult {
    Read(Result<(u64, RwEventFlags), Error>),
    Write(Result<(u64, RwEventFlags), Error>),
    MonotonicClock(Result<(), Error>),
}

impl SubscriptionResult {
    pub fn from_subscription(s: Subscription) -> Option<SubscriptionResult> {
        match s {
            Subscription::Read(s) => s.result().map(SubscriptionResult::Read),
            Subscription::Write(s) => s.result().map(SubscriptionResult::Write),
            Subscription::MonotonicClock(s) => s.result().map(SubscriptionResult::MonotonicClock),
        }
    }
}
