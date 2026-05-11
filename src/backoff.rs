/// Basic exponential backoff implementaiton.

/// - If its generic param is 0, it will always execute `thread::yield_now()`.
/// - If its generic params is positive, it will execute a number of `hint::spin_loop()` before it starts to `thread::yield_now()`.
/// - If its generic param is negative, it will just execute `hint::spin_loop()` without ever yielding.
pub struct Backoff<const SPIN_LIMIT: isize> {
    step: usize,
}
pub(crate) const DEFAULT_SPIN_LIMIT: isize = 6;
// const SPIN_LIMIT: u32 = 6;
impl<const SPIN_LIMIT: isize> Backoff<SPIN_LIMIT> {
    #[inline]
    pub fn new() -> Self {
        Self { step: 1 }
    }
    #[inline]
    pub fn snooze(&mut self) {
        if SPIN_LIMIT < 0 {
            for _ in 0..1 << (-SPIN_LIMIT - 1) {
                core::hint::spin_loop();
            }
            return;
        }

        #[cfg(feature = "std")]
        {
            if self.step <= SPIN_LIMIT as usize {
                for _ in 0..1 << self.step {
                    core::hint::spin_loop();
                }
            } else {
                std::thread::yield_now();
            }
        }
        #[cfg(not(feature = "std"))]
        {
            for _ in 0..1 << self.step {
                core::hint::spin_loop();
            }
        }

        if self.step <= SPIN_LIMIT as usize {
            self.step += 1;
        }
    }
}

pub struct LinearBackoff<const MAX_SPIN_STEPS: usize, const SPINS_PER_STEP: usize> {
    step: usize,
}
impl<const MAX_SPIN_STEPS: usize, const SPINS_PER_STEP: usize>
    LinearBackoff<MAX_SPIN_STEPS, SPINS_PER_STEP>
{
    #[inline]
    pub fn new() -> Self {
        Self { step: 0 }
    }
    #[inline]
    pub fn snooze(&mut self) {
        if self.step < MAX_SPIN_STEPS {
            for _ in 0..SPINS_PER_STEP {
                core::hint::spin_loop();
            }
            self.step += 1;
            return;
        }
        #[cfg(feature = "std")]
        {
            std::thread::yield_now();
        }
        #[cfg(not(feature = "std"))]
        {
            for _ in 0..SPINS_PER_STEP {
                core::hint::spin_loop();
            }
        }
    }
}
