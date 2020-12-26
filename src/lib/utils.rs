use std::path::{Path, PathBuf};

use num_traits::{clamp, AsPrimitive};

macro_rules! item_for_each {
    (
        $( ($($arg:tt)*) ),* $(,)* => { $($exp:tt)* }
    ) => {
        macro_rules! body {
            $($exp)*
        }

        $(
            body! { $($arg)* }
        )*
    };
}

pub fn is_parent_path(l: &PathBuf, r: &PathBuf) -> bool {
    if let Some(r_parent) = r.parent() {
        if l == r_parent {
            return true;
        }
    };
    false
}

pub fn shorten_path<P: AsRef<Path>>(path: P) -> PathBuf {
    let path = path.as_ref();
    dirs::home_dir()
        .and_then(|dir| path.strip_prefix(dir).ok().map(|p| Path::new("~").join(p)))
        .unwrap_or(path.to_path_buf())
}

pub trait Bounded {
    const MIN: Self;
    const MAX: Self;

    fn clamped<T>(x: T) -> T
    where
        Self: AsPrimitive<T>,
        T: 'static + PartialOrd + Copy,
    {
        clamp(x, Self::MIN.as_(), Self::MAX.as_())
    }
}

pub trait ConvertBounded: Bounded {
    fn convert_bounded<T>(x: T) -> Self
    where
        Self: AsPrimitive<T>,
        T: 'static + AsPrimitive<Self> + PartialOrd + Copy,
    {
        Self::clamped(x).as_()
    }
}

item_for_each! {
    (u8), (u16), (u32), (u64), (u128),
    (i8), (i16), (i32), (i64), (i128),
    (f32), (f64), (usize), (isize) => {
        ($num_ty:ident) => {
            impl Bounded for $num_ty {
                const MIN: Self = $num_ty::MIN;
                const MAX: Self = $num_ty::MAX;
            }

            impl ConvertBounded for $num_ty {}
        };
    }
}
