use crate::{FromTDBInstance, PrimitiveValue, ToTDBInstance, ToTDBSchema};

/// marker trait
pub trait Primitive: Into<PrimitiveValue> {}

/// Marker trait for compile-time primitive detection in derive macros
/// Types implementing Primitive automatically get this marker
pub trait PrimitiveMarker {}

// Blanket implementation: all Primitive types are marked
impl<T: Primitive> PrimitiveMarker for T {}

/// Trait to check if a type is primitive at compile time
/// This is used by derive macros to determine the correct deserialization path
pub trait MaybeIsPrimitive {
    fn is_primitive() -> bool;
}

// Default implementation for all types - not primitive
impl<T> MaybeIsPrimitive for T {
    default fn is_primitive() -> bool {
        false
    }
}

// Specialized implementation for types that implement Primitive
impl<T: Primitive> MaybeIsPrimitive for T {
    fn is_primitive() -> bool {
        true
    }
}

impl<T: Primitive> !ToTDBSchema for T {}
impl<T: Primitive> !FromTDBInstance for T {}
impl<T: ToTDBSchema> !Primitive for T {}
