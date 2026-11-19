//! The `derivation!` macro.

/// Define a cacheable computation: its parameters, its output, and its body.
///
/// ```
/// # use terminusdb_cache::{derivation, Derive};
/// # #[derive(Debug, PartialEq, serde::Serialize, serde::Deserialize)]
/// # pub struct Page { total: usize }
/// derivation! {
///     /// The quarter's overview.
///     pub struct OverviewOf { pub quarter: u32, pub limit: usize }
///     -> Page
///     |this, scope| {
///         let _ = (this.quarter, this.limit, scope.commit());
///         Ok(Page { total: 0 })
///     }
/// }
///
/// assert_eq!(<OverviewOf as Derive>::NAME, "OverviewOf");
/// // The body's own text is hashed at compile time, which is what
/// // `EpochPolicy::CodeHash` keys on.
/// assert_ne!(<OverviewOf as Derive>::CODE, 0);
/// ```
///
/// What it writes for you:
///
/// * the parameter struct, with `Debug`, `Clone` and `Serialize` — **do not add
///   `Serialize` yourself**, the key is derived from it;
/// * `NAME`, from the struct's own name;
/// * `CODE`, from `stringify!` of the body, so
///   [`EpochPolicy::CodeHash`](crate::EpochPolicy::CodeHash) can key on the text
///   of the computation;
/// * the `Derive` impl, with your body as `compute`.
///
/// `|this, scope|` names the two bindings: `this` is `&Self` (you cannot call it
/// `self`, which is a keyword) and `scope` is the [`Scope`](crate::Scope). The
/// body is an `async` block returning `anyhow::Result<Output>`, so `?` and
/// `.await` both work in it.
///
/// A unit form is accepted for a computation with no parameters:
///
/// ```
/// # use terminusdb_cache::{derivation, Derive};
/// # #[derive(serde::Serialize, serde::Deserialize)]
/// # pub struct Totals;
/// derivation! {
///     pub struct TotalsNow;
///     -> Totals
///     |_this, _scope| { Ok(Totals) }
/// }
/// # assert_eq!(<TotalsNow as Derive>::NAME, "TotalsNow");
/// ```
///
/// For generics, lifetimes, a custom key or declared dependencies, implement
/// [`Derive`](crate::Derive) by hand — it is four lines plus the body, and the
/// macro has no secrets.
#[macro_export]
macro_rules! derivation {
    // With named parameters.
    (
        $(#[$meta:meta])*
        $vis:vis struct $name:ident {
            $($(#[$field_meta:meta])* $field_vis:vis $field:ident : $ty:ty),* $(,)?
        }
        -> $output:ty
        |$this:tt, $scope:tt| $body:block
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, ::serde::Serialize)]
        $vis struct $name {
            $($(#[$field_meta])* $field_vis $field: $ty,)*
        }

        $crate::derivation_impl!($name, $output, $this, $scope, $body);
    };

    // No parameters.
    (
        $(#[$meta:meta])*
        $vis:vis struct $name:ident;
        -> $output:ty
        |$this:tt, $scope:tt| $body:block
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, ::serde::Serialize)]
        $vis struct $name;

        $crate::derivation_impl!($name, $output, $this, $scope, $body);
    };
}

/// The `Derive` impl half of [`derivation!`]. Not meant to be called directly.
#[doc(hidden)]
#[macro_export]
macro_rules! derivation_impl {
    ($name:ident, $output:ty, $this:tt, $scope:tt, $body:block) => {
        #[::async_trait::async_trait]
        impl $crate::Derive for $name {
            type Output = $output;

            const NAME: &'static str = stringify!($name);

            // The body's own token text, hashed at compile time. This is the
            // closest Rust gets to keying on the transformation itself: at run
            // time a closure is a pointer and some captures, but a macro can
            // hash what it was given. It covers *this* body only — a helper
            // function it calls can change without moving the hash, which is
            // why `EpochPolicy::Deploy` is the default.
            const CODE: u64 = $crate::key::fnv1a(stringify!($body));

            // The body arrives as a block and is emitted as the function's
            // trailing expression, so it nests one block inside another.
            #[allow(unused_braces)]
            async fn compute(&self, scope: &$crate::Scope) -> ::anyhow::Result<Self::Output> {
                let $this = self;
                let $scope = scope;
                $body
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use crate::Derive;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    pub struct Count {
        total: usize,
    }

    derivation! {
        /// Doc comments survive.
        pub struct CountOf { pub class: String, pub limit: usize }
        -> Count
        |this, scope| {
            let _ = scope.commit();
            Ok(Count { total: this.limit })
        }
    }

    derivation! {
        struct Everything;
        -> Count
        |_this, _scope| { Ok(Count { total: 0 }) }
    }

    #[test]
    fn the_name_is_the_struct_name() {
        assert_eq!(<CountOf as Derive>::NAME, "CountOf");
        assert_eq!(<Everything as Derive>::NAME, "Everything");
    }

    #[test]
    fn the_code_hash_differs_between_two_bodies() {
        assert_ne!(<CountOf as Derive>::CODE, <Everything as Derive>::CODE);
        assert_ne!(<CountOf as Derive>::CODE, 0);
    }

    #[test]
    fn the_struct_is_serializable_so_the_key_can_be_derived() {
        let key = crate::key::params_hash(&CountOf {
            class: "Person".to_owned(),
            limit: 20,
        })
        .unwrap();
        let same = crate::key::params_hash(&CountOf {
            class: "Person".to_owned(),
            limit: 20,
        })
        .unwrap();
        let different = crate::key::params_hash(&CountOf {
            class: "Person".to_owned(),
            limit: 21,
        })
        .unwrap();
        assert_eq!(key, same);
        assert_ne!(key, different);
    }

    #[test]
    fn a_unit_derivation_has_a_stable_key_too() {
        assert_eq!(
            crate::key::params_hash(&Everything).unwrap(),
            crate::key::params_hash(&Everything).unwrap()
        );
    }

    #[test]
    fn the_fields_keep_their_visibility_and_can_be_read() {
        let derivation = CountOf {
            class: "Person".to_owned(),
            limit: 7,
        };
        assert_eq!(derivation.limit, 7);
        assert_eq!(derivation.clone().class, "Person");
    }
}
