use core::{
    mem,
    ops::{Deref, DerefMut},
    ptr,
    sync::atomic::{AtomicPtr, AtomicUsize, Ordering},
};

use crate::backoff::{Backoff, DEFAULT_SPIN_LIMIT};

const INIT_UNLOCKED: usize = 1;
const LOCKED: usize = 0;

pub struct SpinSeqLockAtomicPtrEx<const B: isize, T> {
    ptr: AtomicPtr<T>,
    version: AtomicUsize,
}
pub type SpinSeqLockAtomicPtr<T> = SpinSeqLockAtomicPtrEx<DEFAULT_SPIN_LIMIT, T>;
pub type SpinSeqLockAtomicPtrReadGuard<'a, T> = SpinSeqLockAtomicPtrReadGuardEx<'a, DEFAULT_SPIN_LIMIT, T>;
pub type SpinSeqLockAtomicPtrWriteGuard<'a, T> = SpinSeqLockAtomicPtrWriteGuardEx<'a, DEFAULT_SPIN_LIMIT, T>;

pub struct SpinSeqLockAtomicPtrReadGuardEx<'a, const B: isize, T> {
    cell: &'a SpinSeqLockAtomicPtrEx<B, T>,
    ptr_snapshoot: *mut T,
    prev: usize,
}
impl<const B: isize, T> Drop for SpinSeqLockAtomicPtrReadGuardEx<'_, B, T> {
    #[inline]
    fn drop(&mut self) {
        self.cell.version.store(self.prev, Ordering::Release);
    }
}
impl<const B: isize, T> Deref for SpinSeqLockAtomicPtrReadGuardEx<'_, B, T> {
    type Target = *mut T;
    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.ptr_snapshoot
    }
}
pub struct SpinSeqLockAtomicPtrWriteGuardEx<'a, const B: isize, T> {
    cell: &'a SpinSeqLockAtomicPtrEx<B, T>,
    ptr_snapshoot: *mut T,
    next: usize,
}
impl<const B: isize, T> Drop for SpinSeqLockAtomicPtrWriteGuardEx<'_, B, T> {
    #[inline]
    fn drop(&mut self) {
        self.cell.ptr.store(self.ptr_snapshoot, Ordering::Release);
        self.cell.version.store(self.next, Ordering::Release);
    }
}
impl<const B: isize, T> Deref for SpinSeqLockAtomicPtrWriteGuardEx<'_, B, T> {
    type Target = *mut T;
    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.ptr_snapshoot
    }
}
impl<const B: isize, T> DerefMut for SpinSeqLockAtomicPtrWriteGuardEx<'_, B, T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.ptr_snapshoot
    }
}

impl<const B: isize, T> SpinSeqLockAtomicPtrEx<B, T> {
    #[inline]
    pub fn read(&self) -> SpinSeqLockAtomicPtrReadGuardEx<'_, B, T> {
        let mut backoff = Backoff::<B>::new();
        loop {
            let Some(guard) = self.try_read() else {
                backoff.snooze();
                continue;
            };
            return guard;
        }
    }
    #[inline]
    pub fn try_read(&self) -> Option<SpinSeqLockAtomicPtrReadGuardEx<'_, B, T>> {
        let prev = self.version.swap(LOCKED, Ordering::Acquire);

        if prev != LOCKED {
            return Some(SpinSeqLockAtomicPtrReadGuardEx {
                cell: self,
                prev,
                ptr_snapshoot: self.load(),
            });
        }
        None
    }

    #[inline]
    pub fn write(&self) -> SpinSeqLockAtomicPtrWriteGuardEx<'_, B, T> {
        let mut backoff = Backoff::<B>::new();
        loop {
            let Some(guard) = self.try_write() else {
                backoff.snooze();
                continue;
            };
            return guard;
        }
    }
    #[inline]
    pub fn try_write(&self) -> Option<SpinSeqLockAtomicPtrWriteGuardEx<'_, B, T>> {
        let prev = self.version.swap(LOCKED, Ordering::Acquire);

        if prev != LOCKED {
            return Some(SpinSeqLockAtomicPtrWriteGuardEx {
                cell: self,
                next: prev + 1,
                ptr_snapshoot: self.load(),
            });
        }

        None
    }
    // #[inline]
    // pub fn access<R>(&self, callback: impl FnOnce(&T) -> R) -> R {
    //     let read_guard = self.read();
    //     callback(&read_guard)
    // }
    // #[inline]
    // pub fn access_mut<R>(&self, callback: impl FnOnce(&mut T) -> R) -> R {
    //     let mut write_guard = self.write();
    //     callback(&mut write_guard)
    // }

    #[inline]
    fn optimistic_read(&self) -> Option<*mut T> {
        for _ in 0..DEFAULT_SPIN_LIMIT {
            let version = self.version.load(Ordering::Acquire);
            if version != LOCKED {
                let data = self.ptr.load(Ordering::Acquire);

                if self.version.load(Ordering::Relaxed) == version {
                    return Some(data);
                }
            }
        }
        None
    }
    #[inline]
    pub fn load(&self) -> *mut T {
        self.optimistic_read().unwrap_or_else(|| *self.read())
    }
    #[inline]
    pub fn store(&self, v: *mut T) {
        *self.write() = v;
    }
}

// unsafe impl<const B: isize, T> Send for AtomicPtrSpinSeqLockEx<B, T> {}
// unsafe impl<const B: isize, T> Sync for AtomicPtrSpinSeqLockEx<B, T> {}

impl<const B: isize, T> SpinSeqLockAtomicPtrEx<B, T> {
    #[inline]
    pub fn swap(&self, other: &mut *mut T) {
        mem::swap(&mut *self.write(), other)
    }
    #[inline]
    pub fn replace_mut(&mut self, other: *mut T) -> *mut T {
        mem::replace(self.get_mut(), other)
    }
    #[inline]
    pub fn replace(&self, other: *mut T) -> *mut T {
        mem::replace(&mut *self.write(), other)
    }
    #[inline]
    pub const fn new(val: *mut T) -> Self {
        Self {
            ptr: AtomicPtr::new(val),
            version: AtomicUsize::new(INIT_UNLOCKED),
        }
    }
    #[inline]
    pub fn into_inner(self) -> *mut T {
        self.ptr.into_inner()
    }
    #[inline]
    pub fn get_mut(&mut self) -> &mut *mut T {
        self.ptr.get_mut()
    }
}
// impl<const B: isize, T> AtomicPtrSpinSeqLockEx<B, T> {
//     #[inline]
//     pub fn take(&self) -> AtomicPtr<T> {
//         mem::take(&mut self.write())
//     }
//     #[inline]
//     pub fn take_mut(&mut self) -> *mut T {
//         mem::take(&mut self.get_mut())
//     }
// }
impl<const B: isize, T: Default> Default for SpinSeqLockAtomicPtrEx<B, T> {
    #[inline]
    fn default() -> Self {
        Self::new(ptr::null_mut())
    }
}
impl<const B: isize, T> Clone for SpinSeqLockAtomicPtrEx<B, T> {
    #[inline]
    fn clone(&self) -> Self {
        let data = self.load();
        Self::new(data)
    }
}
impl<const B: isize, T: core::fmt::Debug + Copy> core::fmt::Debug for SpinSeqLockAtomicPtrEx<B, T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("AtomicCellOpt")
            .field("data", &self.load())
            .finish()
    }
}
impl<const B: isize, T: PartialEq + Copy> PartialEq for SpinSeqLockAtomicPtrEx<B, T> {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.load() == other.load()
    }
}
impl<const B: isize, T: Eq + Copy> Eq for SpinSeqLockAtomicPtrEx<B, T> {}
