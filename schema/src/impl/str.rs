use crate::{FromInstanceProperty, InstanceProperty, PrimitiveValue};

// impl FromInstanceProperty for String {
//     fn from_property(prop: &InstanceProperty) -> anyhow::Result<Self> {
//         match prop {
//             InstanceProperty::Primitive(PrimitiveValue::String(s)) => {Ok(s.clone())}
//             _ => unimplemented!()
//         }
//     }
// }