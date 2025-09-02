use core::{
    cell::UnsafeCell,
    hash::Hash,
    mem::{self, MaybeUninit},
    ops::{Deref, DerefMut},
    ptr,
    sync::atomic::{AtomicUsize, Ordering, fence},
};

use crate::backoff::{Backoff, DEFAULT_SPIN_LIMIT};

pub type SpinSeqLock<T> = SpinSeqLockEx<DEFAULT_SPIN_LIMIT, T>;
pub type SpinSeqLockReadGuard<'a, T> = SpinSeqLockReadGuardEx<'a, DEFAULT_SPIN_LIMIT, T>;
pub type SpinSeqLockWriteGuard<'a, T> = SpinSeqLockWriteGuardEx<'a, DEFAULT_SPIN_LIMIT, T>;

pub struct SpinSeqLockEx<const B: isize, T> {
    data: UnsafeCell<T>,
    version: AtomicUsize,
}

impl<const N: isize, T> SpinSeqLockEx<N, T> {
    const UNLOCKED_LOCK: usize = 1;
    const LOCKED: usize = 0;
}
pub struct SpinSeqLockReadGuardEx<'a, const B: isize, T> {
    cell: &'a SpinSeqLockEx<B, T>,
    prev: usize,
}
impl<const B: isize, T> Drop for SpinSeqLockReadGuardEx<'_, B, T> {
    #[inline]
    fn drop(&mut self) {
        self.cell.version.store(self.prev, Ordering::Release);
    }
}
impl<const B: isize, T> Deref for SpinSeqLockReadGuardEx<'_, B, T> {
    type Target = T;
    #[inline]
    fn deref(&self) -> &Self::Target {
        // Safety: safe to deref to shared ref while we hold the read lock
        unsafe { &(*self.cell.data.get()) }
    }
}
impl<const B: isize, T> SpinSeqLockEx<B, T> {
    #[inline]
    pub fn read(&self) -> SpinSeqLockReadGuardEx<'_, B, T> {
        let mut backoff = Backoff::<B>::new();
        loop {
            let prev = self.version.swap(Self::LOCKED, Ordering::Acquire);

            if prev != Self::LOCKED {
                return SpinSeqLockReadGuardEx { cell: self, prev };
            }

            backoff.snooze();
        }
    }
}

pub struct SpinSeqLockWriteGuardEx<'a, const B: isize, T> {
    cell: &'a SpinSeqLockEx<B, T>,
    next: usize,
}
impl<const B: isize, T> Drop for SpinSeqLockWriteGuardEx<'_, B, T> {
    #[inline]
    fn drop(&mut self) {
        self.cell.version.store(self.next, Ordering::Release);
    }
}
impl<const B: isize, T> Deref for SpinSeqLockWriteGuardEx<'_, B, T> {
    type Target = T;
    #[inline]
    fn deref(&self) -> &Self::Target {
        // Safety: safe to deref while we hold the write lock
        unsafe { &(*self.cell.data.get()) }
    }
}
impl<const B: isize, T> DerefMut for SpinSeqLockWriteGuardEx<'_, B, T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        // Safety: safe to deref while we hold the write lock
        unsafe { &mut (*self.cell.data.get()) }
    }
}
impl<const B: isize, T> SpinSeqLockEx<B, T> {
    #[inline]
    pub fn write(&self) -> SpinSeqLockWriteGuardEx<'_, B, T> {
        let mut backoff = Backoff::<B>::new();
        loop {
            let prev = self.version.swap(Self::LOCKED, Ordering::Acquire);

            if prev != Self::LOCKED {
                fence(Ordering::Release);
                return SpinSeqLockWriteGuardEx {
                    cell: self,
                    next: prev + 1,
                };
            }

            backoff.snooze();
        }
    }
}

impl<const B: isize, T> SpinSeqLockEx<B, T> {
    #[inline]
    pub fn access<R>(&self, callback: impl FnOnce(&T) -> R) -> R {
        let read_guard = self.read();
        callback(&read_guard)
    }
    #[inline]
    pub fn access_mut<R>(&self, callback: impl FnOnce(&mut T) -> R) -> R {
        let mut write_guard = self.write();
        callback(&mut write_guard)
    }
}

impl<const B: isize, T: Copy> SpinSeqLockEx<B, T> {
    #[inline]
    fn optimistic_read(&self) -> Option<T> {
        #[cfg(not(miri))]
        {
            let version = self.version.load(Ordering::Acquire);
            if version != Self::LOCKED {
                // We need a volatile_read here because other threads might concurrently modify the value.
                // In Rust/C++ memory model, data races are *always UB*, even if we can always
                // detect the data race and discard the result.
                // LLVM memory model allows for this use case, which is probably the reason things dont blow up.
                let data = unsafe { ptr::read_volatile(self.data.get().cast::<MaybeUninit<T>>()) };
                fence(Ordering::Acquire);
                if self.version.load(Ordering::Relaxed) == version {
                    // Safety: since the version did not change, we can be sure that there was no writes while we were reading the value.
                    return Some(unsafe { data.assume_init() });
                }
            }
        }
        None
    }
    #[inline]
    pub fn load(&self) -> T {
        self.optimistic_read().unwrap_or_else(|| *self.read())
    }
}

unsafe impl<const B: isize, T: Send> Send for SpinSeqLockEx<B, T> {}
unsafe impl<const B: isize, T: Send> Sync for SpinSeqLockEx<B, T> {}

impl<const B: isize, T> SpinSeqLockEx<B, T> {
    #[inline]
    pub fn swap(&self, other: &mut T) {
        mem::swap(&mut *self.write(), other)
    }
    #[inline]
    pub fn replace_mut(&mut self, other: T) -> T {
        mem::replace(self.get_mut(), other)
    }
    #[inline]
    pub fn replace(&self, other: T) -> T {
        mem::replace(&mut *self.write(), other)
    }
    #[inline]
    pub const fn new(val: T) -> Self {
        Self {
            data: UnsafeCell::new(val),
            version: AtomicUsize::new(Self::UNLOCKED_LOCK),
        }
    }
    #[inline]
    pub fn into_inner(self) -> T {
        self.data.into_inner()
    }
    #[inline]
    pub fn get_mut(&mut self) -> &mut T {
        self.data.get_mut()
    }
    #[inline]
    pub fn store(&self, v: T) {
        *self.write() = v;
    }
}
impl<const B: isize, T: Default> SpinSeqLockEx<B, T> {
    #[inline]
    pub fn take(&self) -> T {
        mem::take(&mut self.write())
    }
}
impl<const B: isize, T: Default> Default for SpinSeqLockEx<B, T> {
    #[inline]
    fn default() -> Self {
        Self {
            data: UnsafeCell::new(T::default()),
            version: AtomicUsize::new(Self::UNLOCKED_LOCK),
        }
    }
}
impl<const B: isize, T: Copy> Clone for SpinSeqLockEx<B, T> {
    #[inline]
    fn clone(&self) -> Self {
        let data = self.load();
        Self {
            data: UnsafeCell::new(data),
            version: AtomicUsize::new(Self::UNLOCKED_LOCK),
        }
    }
}
impl<const B: isize, T: Clone> SpinSeqLockEx<B, T> {
    #[inline]
    pub fn clone2(&self) -> Self {
        let data = self.read();
        Self {
            data: UnsafeCell::new(data.clone()),
            version: AtomicUsize::new(Self::UNLOCKED_LOCK),
        }
    }
}
impl<const B: isize, T: core::fmt::Debug + Copy> core::fmt::Debug for SpinSeqLockEx<B, T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("AtomicCellOpt")
            .field("data", &self.load())
            .finish()
    }
}
impl<const B: isize, T: Hash + Copy> Hash for SpinSeqLockEx<B, T> {
    #[inline]
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        self.load().hash(state);
    }
}
impl<const B: isize, T: PartialEq + Copy> PartialEq for SpinSeqLockEx<B, T> {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.load() == other.load()
    }
}
impl<const B: isize, T: Eq + Copy> Eq for SpinSeqLockEx<B, T> {}

impl<const B: isize, T: PartialOrd + Copy> PartialOrd for SpinSeqLockEx<B, T> {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        self.load().partial_cmp(&other.load())
    }
}
impl<const B: isize, T: Ord + Copy> Ord for SpinSeqLockEx<B, T> {
    #[inline]
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        if core::ptr::eq(self, other) {
            core::cmp::Ordering::Equal
        } else {
            self.load().cmp(&other.load())
        }
    }
}
#[cfg(feature = "serde")]
mod ser_de {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    use crate::spin_seqlock::SpinSeqLockEx;
    impl<const B: isize, T: Serialize + Copy> Serialize for SpinSeqLockEx<B, T> {
        fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
            self.load().serialize(serializer)
        }
    }
    impl<'a, const B: isize, T: Deserialize<'a> + Copy> Deserialize<'a> for SpinSeqLockEx<B, T> {
        fn deserialize<D: Deserializer<'a>>(deserializer: D) -> Result<Self, D::Error> {
            Ok(Self::new(T::deserialize(deserializer)?))
        }
    }
}
impl<const B: isize, T: Copy> From<T> for SpinSeqLockEx<B, T> {
    #[inline]
    fn from(value: T) -> Self {
        Self::new(value)
    }
}
