use crate::{FromTDBInstance, PrimitiveValue, ToTDBInstance, ToTDBSchema};

/// marker trait
pub trait Primitive : Into<PrimitiveValue>{}


impl<T: Primitive> !ToTDBSchema for T {}
impl<T: Primitive> !FromTDBInstance for T {}
impl<T: ToTDBSchema> !Primitive for T {}