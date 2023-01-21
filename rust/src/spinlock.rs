use core::cell::UnsafeCell;
use core::ops::{Deref, DerefMut};
use core::sync::atomic::{spin_loop_hint, AtomicBool, Ordering};

use crate::kernel::{popcli, pushcli};

/// A simple spinlock implementation.
pub struct Spinlock<T> {
    // The current state of the lock.
    lock: AtomicBool,
    // The data protected by the lock.
    data: UnsafeCell<T>,
}

unsafe impl<T: Send> Sync for Spinlock<T> {}
unsafe impl<T: Send> Send for Spinlock<T> {}

impl<T> Spinlock<T> {
    #[inline(always)]
    pub const fn new(data: T) -> Self {
        Spinlock {
            lock: AtomicBool::new(false),
            data: UnsafeCell::new(data),
        }
    }

    #[inline(always)]
    pub fn is_locked(&self) -> bool {
        self.lock.load(Ordering::Relaxed)
    }

    #[inline(always)]
    pub fn lock(&self) -> SpinlockGuard<T> {
        loop {
            if !self.lock.compare_and_swap(false, true, Ordering::Acquire) {
                self.on_lock();
                return SpinlockGuard { spinlock: self };
            }
            spin_loop_hint();
        }
    }

    #[inline(always)]
    fn on_lock(&self) {
        unsafe {
            pushcli();
        }
    }

    #[inline(always)]
    fn on_unlock(&self) {
        unsafe {
            popcli();
        }
    }
}

pub struct SpinlockGuard<'a, T: 'a> {
    spinlock: &'a Spinlock<T>,
}

impl<'a, T> Drop for SpinlockGuard<'a, T> {
    fn drop(&mut self) {
        self.spinlock.lock.store(false, Ordering::Release);
        self.spinlock.on_unlock();
    }
}

impl<'a, T> Deref for SpinlockGuard<'a, T> {
    type Target = T;
    fn deref(&self) -> &T {
        unsafe { &*self.spinlock.data.get() }
    }
}

impl<'a, T> DerefMut for SpinlockGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.spinlock.data.get() }
    }
}
