use std::marker::PhantomData;
use crate::{InstanceProperty, PrimitiveValue, Schema, ToInstanceProperty};

impl<T, Parent> ToInstanceProperty<Parent> for PhantomData<T> {
    fn to_property(self, field_name: &str, parent: &Schema) -> InstanceProperty {
        InstanceProperty::Primitive(PrimitiveValue::Unit)
    }
}