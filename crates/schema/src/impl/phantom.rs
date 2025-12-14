use crate::{InstanceProperty, PrimitiveValue, Schema, ToInstanceProperty, FromInstanceProperty};
use std::marker::PhantomData;
use anyhow::Result;

impl<T, Parent> ToInstanceProperty<Parent> for PhantomData<T> {
    fn to_property(self, field_name: &str, parent: &Schema) -> InstanceProperty {
        InstanceProperty::Primitive(PrimitiveValue::Unit)
    }
}

impl<T> FromInstanceProperty for PhantomData<T> {
    fn from_property(prop: &InstanceProperty) -> Result<Self> {
        Ok(PhantomData)
    }
    
    fn from_maybe_property(prop: &Option<InstanceProperty>) -> Result<Self> {
        Ok(PhantomData)
    }
}
