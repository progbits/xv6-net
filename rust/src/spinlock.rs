use core::cell::UnsafeCell;
use core::sync::atomic::{AtomicBool, Ordering};
use core::{
    ops::{Deref, DerefMut},
};

/// A simple spinlock implementation.
pub struct Spinlock<T> {
    // The current state of the lock.
    lock: AtomicBool,
    // The data protected by the lock.
    data: UnsafeCell<T>,
}

unsafe impl<T> Sync for Spinlock<T> {}

impl<T> Spinlock<T> {
    #[inline(always)]
    pub const fn new(data: T) -> Self {
        Spinlock {
            lock: AtomicBool::new(false),
            data: UnsafeCell::new(data),
        }
    }

    /// Try and acquire the lock, spinning while it is unavaliable.
    /// TODO: Disable interrupts.
    pub fn lock(&self) -> SpinlockGuard<T> {
        loop {
            if !self.lock.compare_and_swap(false, true, Ordering::Acquire) {
                return SpinlockGuard { spinlock: self };
            }
        }
    }
}

/// RAII wrapper.
pub struct SpinlockGuard<'a, T: 'a> {
    spinlock: &'a Spinlock<T>,
}

/// Unlock on drop.
impl<'a, T> Drop for SpinlockGuard<'a, T> {
    fn drop(&mut self) {
        self.spinlock.lock.store(false, Ordering::Release);
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
