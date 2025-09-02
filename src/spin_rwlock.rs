use core::{
    cell::UnsafeCell,
    ops::{Deref, DerefMut},
    sync::atomic::{AtomicIsize, Ordering},
};

use crate::backoff::{Backoff, DEFAULT_SPIN_LIMIT};

pub type SpinRwLock<T> = SpinRwLockEx<DEFAULT_SPIN_LIMIT, T>;
pub type SpinRwLockReadGuard<'a, T> = SpinRwLockReadGuardEx<'a, DEFAULT_SPIN_LIMIT, T>;
pub type SpinRwLockWriteGuard<'a, T> = SpinRwLockWriteGuardEx<'a, DEFAULT_SPIN_LIMIT, T>;


const SPIN_RW_LOCK_LOCKED: isize = -1;
const SPIN_RW_LOCK_UNLOCKED: isize = 0;
pub struct SpinRwLockEx<const S: isize, T> {
    data: UnsafeCell<T>,
    readers: AtomicIsize,
}
#[repr(transparent)]
pub struct SpinRwLockReadGuardEx<'a, const S: isize, T> {
    lock: &'a SpinRwLockEx<S, T>,
}
#[repr(transparent)]
pub struct SpinRwLockWriteGuardEx<'a, const S: isize, T> {
    lock: &'a SpinRwLockEx<S, T>,
}
impl<const S: isize, T> Drop for SpinRwLockReadGuardEx<'_, S, T> {
    #[inline]
    fn drop(&mut self) {
        self.lock.readers.fetch_sub(1, Ordering::Release);
    }
}
impl<const S: isize, T> Drop for SpinRwLockWriteGuardEx<'_, S, T> {
    #[inline]
    fn drop(&mut self) {
        self.lock
            .readers
            .store(SPIN_RW_LOCK_UNLOCKED, Ordering::Release);
    }
}
impl<const S: isize, T> Deref for SpinRwLockReadGuardEx<'_, S, T> {
    type Target = T;
    #[inline]
    fn deref(&self) -> &Self::Target {
        // Safety: safe to deref while we hold the read lock
        unsafe { &*self.lock.data.get() }
    }
}
impl<const S: isize, T> Deref for SpinRwLockWriteGuardEx<'_, S, T> {
    type Target = T;
    #[inline]
    fn deref(&self) -> &Self::Target {
        // Safety: safe to deref while we hold the write lock
        unsafe { &*self.lock.data.get() }
    }
}
impl<const S: isize, T> DerefMut for SpinRwLockWriteGuardEx<'_, S, T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        // Safety: safe to deref while we hold the write lock
        unsafe { &mut *self.lock.data.get() }
    }
}
impl<const S: isize, T> SpinRwLockEx<S, T> {
    #[inline]
    pub fn new(val: T) -> Self {
        Self {
            data: UnsafeCell::new(val),
            readers: AtomicIsize::new(SPIN_RW_LOCK_UNLOCKED),
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
    pub fn write(&self) -> SpinRwLockWriteGuardEx<'_, S, T> {
        let mut backoff = Backoff::<S>::new();
        loop {
            if self
                .readers
                .compare_exchange(
                    SPIN_RW_LOCK_UNLOCKED,
                    SPIN_RW_LOCK_LOCKED,
                    Ordering::Acquire,
                    Ordering::Relaxed,
                )
                .is_ok()
            {
                return SpinRwLockWriteGuardEx { lock: self };
            }
            backoff.snooze();
        }
    }
    pub fn read(&self) -> SpinRwLockReadGuardEx<'_, S, T> {
        let mut backoff = Backoff::<S>::new();
        let mut current = self.readers.load(Ordering::Relaxed);
        loop {
            if current == SPIN_RW_LOCK_LOCKED {
                backoff.snooze();
                current = self.readers.load(Ordering::Relaxed);
                continue;
            }
            match self.readers.compare_exchange(
                current,
                current.wrapping_add(1),
                Ordering::Acquire,
                Ordering::Relaxed,
            ) {
                Ok(_) => {
                    return SpinRwLockReadGuardEx { lock: self };
                }
                Err(prev) => {
                    current = prev;
                    backoff.snooze();
                }
            }
        }
    }
}
unsafe impl<const S: isize, T: Send> Send for SpinRwLockEx<S, T> {}
unsafe impl<const S: isize, T: Send + Sync> Sync for SpinRwLockEx<S, T> {}
