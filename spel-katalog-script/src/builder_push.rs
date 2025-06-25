//! Macro to generate push methods for builders.

macro_rules! builder_push {
    ($builder:ident $({ $field:ident $(, $single:ident)?: $ty:ty $( => $expr:expr)?})* $(items {$($item:item)*})?) => {
        ::paste::paste! {

        impl<S: [< $builder:snake >]::State> $builder<S> {
            $( $crate::builder_push::builder_push!(@impl $field $(, $single)*: $ty $( => $expr )*); )*
            $($($item)*)*
        }


        }
    };
    (@impl $field:ident: $ty:ty) => {
        ::paste::paste! {

        #[doc = "Push a value to `" $field "`."]
        pub fn $field(self, $field: $ty) -> Self {
            self.[< $field _extend >]([$field])
        }
        #[doc = "Push multiple values to `" $field "`."]
        pub fn [< $field _extend >](mut self, $field: impl IntoIterator<Item = $ty>) -> Self {
            self.$field.extend($field);
            self
        }

        }
    };
    (@impl $field:ident, $single:ident: $ty:ty => $expr:expr) => {
        ::paste::paste! {

        #[doc = "Push a value to `" $field "`."]
        pub fn $single(self, $single: $ty) -> Self {
            self.[< $field _extend >]([$single])
        }
        #[doc = "Push multiple values to `" $field "`."]
        pub fn [< $field _extend >](mut self, $single: impl IntoIterator<Item = $ty>) -> Self {
            self.$field.extend($single.into_iter().map(|$single| $expr ));
            self
        }

        }
    };
    (@impl $field:ident, $single:ident: $ty:ty) => {
        ::paste::paste! {

        #[doc = "Push a value to `" $field "`."]
        pub fn $single(self, $single: $ty) -> Self {
            self.[< $field _extend >]([$single])
        }
        #[doc = "Push multiple values to `" $field "`."]
        pub fn [< $field _extend >](mut self, $single: impl IntoIterator<Item = $ty>) -> Self {
            self.$field.extend($single.into_iter());
            self
        }

        }
    };
    (@impl $field:ident: $ty:ty => $expr:expr) => {
        ::paste::paste! {

        #[doc = "Push a value to `" $field "`."]
        pub fn $field(self, $field: $ty) -> Self {
            self.[< $field _extend >]([$field])
        }
        #[doc = "Push multiple values to `" $field "`."]
        pub fn [< $field _extend >](mut self, $field: impl IntoIterator<Item = $ty>) -> Self {
            self.$field.extend($field.into_iter().map(|$field| $expr ));
            self
        }

        }
    };
}
pub(crate) use builder_push;
