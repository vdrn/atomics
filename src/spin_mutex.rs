use core::{
    cell::UnsafeCell,
    ops::{Deref, DerefMut},
    sync::atomic::{AtomicBool, Ordering},
};

use crate::backoff::{Backoff, DEFAULT_SPIN_LIMIT};

pub type SpinMutex<T> = SpinMutexEx<DEFAULT_SPIN_LIMIT, T>;
pub type SpinMutexGuard<'a, T> = SpinMutexExGuard<'a, DEFAULT_SPIN_LIMIT, T>;

pub struct SpinMutexEx<const S: isize, T> {
    data: UnsafeCell<T>,
    locked: AtomicBool,
}
#[repr(transparent)]
pub struct SpinMutexExGuard<'a, const S: isize, T> {
    lock: &'a SpinMutexEx<S, T>,
}
impl<const S: isize, T> Drop for SpinMutexExGuard<'_, S, T> {
    #[inline]
    fn drop(&mut self) {
        self.lock.locked.store(false, Ordering::Release);
    }
}
impl<const S: isize, T> Deref for SpinMutexExGuard<'_, S, T> {
    type Target = T;
    #[inline]
    fn deref(&self) -> &Self::Target {
        // Safety: safe to deref while we hold the read lock
        unsafe { &*self.lock.data.get() }
    }
}
impl<const S: isize, T: Default> Default for SpinMutexEx<S, T> {
    #[inline]
    fn default() -> Self {
        Self {
            data: UnsafeCell::new(T::default()),
            locked: AtomicBool::new(false),
        }
    }
}
impl<const S: isize, T> DerefMut for SpinMutexExGuard<'_, S, T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        // Safety: safe to deref while we hold the write lock
        unsafe { &mut *self.lock.data.get() }
    }
}
impl<const S: isize, T: core::fmt::Debug> core::fmt::Debug for SpinMutexEx<S, T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("SpinLock")
            .field("data", &self.data.get())
            .finish()
    }
}
impl<const S: isize, T> SpinMutexEx<S, T> {
    #[inline]
    pub fn new(val: T) -> Self {
        Self {
            data: UnsafeCell::new(val),
            locked: AtomicBool::new(false),
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
    pub fn lock(&self) -> SpinMutexExGuard<'_, S, T> {
        let mut backoff = Backoff::<S>::new();
        loop {
            if self
                .locked
                .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
                .is_ok()
            {
                return SpinMutexExGuard { lock: self };
            }
            backoff.snooze();
        }
    }
}
unsafe impl<const S: isize, T: Send> Send for SpinMutexEx<S, T> {}
unsafe impl<const S: isize, T: Send> Sync for SpinMutexEx<S, T> {}
