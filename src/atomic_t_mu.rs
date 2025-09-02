use core::{fmt, marker::PhantomData, mem, mem::MaybeUninit, sync::atomic::Ordering};

use atomic_maybe_uninit::AtomicMaybeUninit;

macro_rules! impl_atomic_t {
  ($($struct_name:ident, $atomic:ty, $int:ty);*;) => {
    $(
    pub struct $struct_name<T: Copy> {
      data: $atomic,
      _pd:  PhantomData<T>,
    }

    impl<T: Copy> $struct_name<T> {
      const fn transmute_to_t(value: MaybeUninit<$int>) -> T {
        $crate::const_type_assert!(
          T,
          mem::size_of::<T>() == mem::size_of::<$int>(),
          "Size of T must be same as in the name of the container",
        );
        $crate::const_type_assert!(
          T,
          mem::align_of::<T>() <= mem::align_of::<$int>(),
          "Align of T must be <= than the name of the container",
        );

        // Safety:
        // As long as `value` was produced by transmuting T -> $int, inverse direction is OK
        unsafe { mem::transmute_copy(&value.assume_init()) }
      }
      const fn transmute_to_u(value: T) -> MaybeUninit<$int> {
        $crate::const_type_assert!(
          T,
          mem::size_of::<T>() == mem::size_of::<$int>(),
          "Size of T must be same as in the name of the container",
        );
        $crate::const_type_assert!(
          T,
          mem::align_of::<T>() <= mem::align_of::<$int>(),
          "Align of T must be <= than the name of the container",
        );

        // Safety:
        // Transmuting to MaybeUninit<$int> of same size and alignment is safe
        unsafe { mem::transmute_copy(&value) }
      }


      #[inline]
      pub const fn new(value: T) -> Self {
        let data = Self::transmute_to_u(value);
        Self { data: <$atomic>::new(data), _pd: PhantomData }
      }
      #[inline]
      pub fn get_mut(&mut self) -> &mut T { unsafe { &mut *(self.data.get_mut().assume_init_mut() as *mut $int as *mut T) } }

      #[inline]
      pub fn load(&self, ordering: Ordering) -> T { Self::transmute_to_t(self.data.load(ordering)) }
      #[inline]
      pub fn store(&self, value: T, ordering: Ordering) {
        self.data.store(Self::transmute_to_u(value), ordering)
      }

      #[inline]
      pub fn into_inner(self) -> T { Self::transmute_to_t(self.data.into_inner()) }
      #[inline]
      pub fn swap(&self, value: T, order: Ordering) -> T {
        Self::transmute_to_t(self.data.swap(Self::transmute_to_u(value), order))
      }
      #[inline]
      pub fn swap_mut(&mut self,value: T) -> T {
        mem::replace(self.get_mut(), value)
      }

      #[inline]
      pub fn compare_exchange(
        &self, current: T, new: T, success: Ordering, failure: Ordering,
      ) -> Result<T, T> {
        self
          .data
          .compare_exchange(Self::transmute_to_u(current), Self::transmute_to_u(new), success, failure)
          .map(|s| Self::transmute_to_t(s))
          .map_err(|e| Self::transmute_to_t(e))
      }
      #[inline]
      pub fn compare_exchange_weak(
        &self, current: T, new: T, success: Ordering, failure: Ordering,
      ) -> Result<T, T> {
        self
          .data
          .compare_exchange_weak(Self::transmute_to_u(current), Self::transmute_to_u(new), success, failure)
          .map(|s| Self::transmute_to_t(s))
          .map_err(|e| Self::transmute_to_t(e))
      }
      #[inline]
      pub fn fetch_update(
        &self, set_order: Ordering, fetch_order: Ordering, mut f: impl FnMut(T) -> Option<T>,
      ) -> Result<T, T> {
        self
          .data
          .fetch_update(set_order, fetch_order, |u| {
            f(Self::transmute_to_t(u)).map(|r| Self::transmute_to_u(r))
          })
          .map(|s| Self::transmute_to_t(s))
          .map_err(|e| Self::transmute_to_t(e))
      }
      #[inline]
      pub fn as_ptr(&self) -> *mut T { self.data.as_ptr() as *mut T }
    }

    impl<T: Default + Copy> $struct_name<T> {
    //   fn default() -> Self { Self::new(T::default()) }
      #[inline]
      pub fn take(&self, order:Ordering)->T{
        self.swap(T::default(), order)
      }
    }
    impl<T: Default + Copy> Default for $struct_name<T> {
      fn default() -> Self { Self::new(T::default()) }
    }
    impl<T: fmt::Debug + Copy> fmt::Debug for $struct_name<T> {
      fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.load(Ordering::Relaxed), f)
      }
    }
    impl<T:PartialEq + Copy> PartialEq for $struct_name<T>{
      #[inline]
      fn eq(&self, other: &Self) -> bool {
        self.load(Ordering::Relaxed) == other.load(Ordering::Relaxed)
      }
    }
    impl< T:Copy> From<T> for $struct_name<T>{
      #[inline]
      fn from(value: T) -> Self { Self::new(value) }
    }
    )*
  };
}
type AMUu8 = AtomicMaybeUninit<u8>;
type AMUu16 = AtomicMaybeUninit<u16>;
type AMUu32 = AtomicMaybeUninit<u32>;
type AMUu64 = AtomicMaybeUninit<u64>;
type AMUusize = AtomicMaybeUninit<usize>;
impl_atomic_t! {
  AtomicT8, AMUu8, u8;
  AtomicT16, AMUu16, u16;
  AtomicT32, AMUu32, u32;
  AtomicT64, AMUu64, u64;
  AtomicTUsize, AMUusize, usize;
}

