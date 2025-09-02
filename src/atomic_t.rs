use core::{
    fmt,
    marker::PhantomData,
    mem,
    sync::atomic::{AtomicU8, AtomicU16, AtomicU32, AtomicU64, AtomicUsize, Ordering},
};

macro_rules! impl_atomic_t {
  ($($struct_name:ident, $atomic:ty, $int:ty);*;) => {
    $(

    pub struct $struct_name<T: Copy> {
      data: $atomic,
      _pd:  PhantomData<T>,
    }

    #[cfg(feature = "bytemuck")]
    impl <T:bytemuck::NoUninit+Copy> $struct_name<T>{
      #[inline]
      pub const fn new_no_uninit(value:T)->Self{
        // # Safety: bytemuck::NoUninit guarantees no padding
        unsafe{
          Self::new(value)
        }
      }
    }
    impl<T: Copy> $struct_name<T> {
      const fn transmute_to_t(value: $int) -> T {
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
        unsafe { mem::transmute_copy(&value) }
      }
      const fn transmute_to_u(value: T) -> $int {
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
        // As long as T does not contain any padding bytes, this transmute is OK
        unsafe { mem::transmute_copy(&value) }
      }

      /// # Safety
      /// `T` cannot have any padding bytes
      #[inline]
      pub const unsafe fn new(value: T) -> Self {
        let data = Self::transmute_to_u(value);

        Self { data: <$atomic>::new(data), _pd: PhantomData }
      }
       #[inline]
      pub fn get_mut(&mut self) -> &mut T { unsafe { &mut *(self.data.get_mut() as *mut $int as *mut T) } }

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
      // pub fn compare_and_swap(&self, current: T, new: T, order: Ordering) -> T {
      //   Self::transmute_to_t(self.data.compare_and_swap(
      //     Self::transmute_to_u(current),
      //     Self::transmute_to_u(new),
      //     order,
      //   ))
      // }
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
    // impl<T: Copy> From<T> for $struct_name<T> {
    //   fn from(value: T) -> Self { Self::new(value) }
    // }
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
    #[cfg(feature = "bytemuck")]
    impl< T:bytemuck::NoUninit> From<T> for $struct_name<T>{
      #[inline]
      fn from(value: T) -> Self { Self::new_no_uninit(value) }
    }
    )*
  };
}
impl_atomic_t! {
  AtomicT8, AtomicU8, u8;
  AtomicT16, AtomicU16, u16;
  AtomicT32, AtomicU32, u32;
  AtomicT64, AtomicU64, u64;
  AtomicTUsize, AtomicUsize, usize;
}

