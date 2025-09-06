//! Basic utils for concurrent programming. Backoff, spinlocks, seqlock, atomic type wrappers.
#![cfg_attr(not(feature = "std"), no_std)]
pub mod atomic_t;
pub mod atomic_t_mu;
pub mod backoff;
pub mod spin_mutex;
pub mod spin_rwlock;
pub mod spin_seqlock;
pub mod atomic_spin_seqlock;

macro_rules! const_type_assert {
    ($t:ident, $c:expr, $($arg:tt)*) => {{
        struct CompileTimeCheck<$t>($t);
        impl<$t> CompileTimeCheck<$t> {
            const CHECK: () = assert!($c, $($arg)*);
        }
        let _ = CompileTimeCheck::<$t>::CHECK;
    }}
}
pub(crate) use const_type_assert;
