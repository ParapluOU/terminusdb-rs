use crate::{FromInstanceProperty, InstanceProperty, PrimitiveValue, Schema, ToInstanceProperty};
use anyhow::Result;
use std::marker::PhantomData;

impl<T, Parent> ToInstanceProperty<Parent> for PhantomData<T> {
    fn to_property(self, _field_name: &str, _parent: &Schema) -> InstanceProperty {
        InstanceProperty::Primitive(PrimitiveValue::Unit)
    }
}

impl<T> FromInstanceProperty for PhantomData<T> {
    fn from_property(_prop: &InstanceProperty) -> Result<Self> {
        Ok(PhantomData)
    }

    fn from_maybe_property(_prop: &Option<InstanceProperty>) -> Result<Self> {
        Ok(PhantomData)
    }
}
