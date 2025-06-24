// use anyhow::Result;
// use terminusdb_schema::{
//     json::InstancePropertyFromJson, FromInstanceProperty, InstanceProperty, PrimitiveValue, Schema,
//     ToInstanceProperty,
// };
// use serde_json::{json, Value};
//
// // Struct that is used as the Parent type parameter for testing
// struct TestParent;
//
// // Simple trait implementations for testing
// impl ToInstanceProperty<TestParent> for String {
//     fn to_property(self, _field_name: &str, _parent: &Schema) -> InstanceProperty {
//         InstanceProperty::Primitive(PrimitiveValue::String(self))
//     }
// }
//
// impl ToInstanceProperty<TestParent> for i32 {
//     fn to_property(self, _field_name: &str, _parent: &Schema) -> InstanceProperty {
//         InstanceProperty::Primitive(PrimitiveValue::Number(self.into()))
//     }
// }
//
// impl ToInstanceProperty<TestParent> for bool {
//     fn to_property(self, _field_name: &str, _parent: &Schema) -> InstanceProperty {
//         InstanceProperty::Primitive(PrimitiveValue::Bool(self))
//     }
// }
//
// impl<T: ToInstanceProperty<TestParent>> ToInstanceProperty<TestParent> for Option<T> {
//     fn to_property(self, field_name: &str, parent: &Schema) -> InstanceProperty {
//         match self {
//             Some(v) => v.to_property(field_name, parent),
//             None => InstanceProperty::Primitive(PrimitiveValue::Null),
//         }
//     }
// }
//
// impl<T: ToInstanceProperty<TestParent>> ToInstanceProperty<TestParent> for Vec<T> {
//     fn to_property(self, _field_name: &str, _parent: &Schema) -> InstanceProperty {
//         InstanceProperty::Primitive(PrimitiveValue::Null) // Simplified for test
//     }
// }
//
// // Implement FromInstanceProperty for our test types
// impl FromInstanceProperty for String {
//     fn from_property(prop: &InstanceProperty) -> anyhow::Result<Self> {
//         match prop {
//             InstanceProperty::Primitive(PrimitiveValue::String(s)) => Result::Ok(s.clone()),
//             _ => Err(anyhow::anyhow!("Expected String primitive")),
//         }
//     }
// }
//
// impl FromInstanceProperty for i32 {
//     fn from_property(prop: &InstanceProperty) -> anyhow::Result<Self> {
//         match prop {
//             InstanceProperty::Primitive(PrimitiveValue::Number(n)) => n
//                 .as_i64()
//                 .and_then(|i| i32::try_from(i).ok())
//                 .ok_or_else(|| anyhow::anyhow!("Number cannot be converted to i32")),
//             _ => Err(anyhow::anyhow!("Expected Number primitive")),
//         }
//     }
// }
//
// impl FromInstanceProperty for bool {
//     fn from_property(prop: &InstanceProperty) -> anyhow::Result<Self> {
//         match prop {
//             InstanceProperty::Primitive(PrimitiveValue::Bool(b)) => Result::Ok(*b),
//             _ => Err(anyhow::anyhow!("Expected Boolean primitive")),
//         }
//     }
// }
//
// #[test]
// fn test_string_property_from_json() {
//     let json_value = json!("test string");
//
//     let property = <String as InstancePropertyFromJson<TestParent>>::property_from_json(json_value)
//         .expect("Failed to convert string");
//
//     assert!(
//         matches!(property, InstanceProperty::Primitive(PrimitiveValue::String(s)) if s == "test string")
//     );
// }
//
// #[test]
// fn test_number_property_from_json() {
//     let json_value = json!(42);
//
//     let property = <i32 as InstancePropertyFromJson<TestParent>>::property_from_json(json_value)
//         .expect("Failed to convert number");
//
//     assert!(matches!(
//         property,
//         InstanceProperty::Primitive(PrimitiveValue::Number(_))
//     ));
// }
//
// #[test]
// fn test_boolean_property_from_json() {
//     let json_value = json!(true);
//
//     let property = <bool as InstancePropertyFromJson<TestParent>>::property_from_json(json_value)
//         .expect("Failed to convert boolean");
//
//     assert!(matches!(property, InstanceProperty::Primitive(PrimitiveValue::Bool(b)) if b));
// }
//
// #[test]
// fn test_option_property_from_json() {
//     // Test Some value
//     let json_value = json!("test string");
//     let property =
//         <Option<String> as InstancePropertyFromJson<TestParent>>::property_from_json(json_value)
//             .expect("Failed to convert option with value");
//     assert!(matches!(
//         property,
//         InstanceProperty::Primitive(PrimitiveValue::String(_))
//     ));
//
//     // Test None value
//     let json_value = json!(null);
//     let property =
//         <Option<String> as InstancePropertyFromJson<TestParent>>::property_from_json(json_value)
//             .expect("Failed to convert null to option");
//     assert!(matches!(
//         property,
//         InstanceProperty::Primitive(PrimitiveValue::Null)
//     ));
// }
//
// #[test]
// fn test_option_property_from_maybe_json() {
//     // Test Some value
//     let json_value = Some(json!("test string"));
//     let property =
//         <Option<String> as InstancePropertyFromJson<TestParent>>::property_from_maybe_json(
//             json_value,
//         )
//         .expect("Failed to convert option with value");
//     assert!(matches!(
//         property,
//         InstanceProperty::Primitive(PrimitiveValue::String(_))
//     ));
//
//     // Test None value
//     let property =
//         <Option<String> as InstancePropertyFromJson<TestParent>>::property_from_maybe_json(None)
//             .expect("Failed to convert null to option");
//     assert!(matches!(
//         property,
//         InstanceProperty::Primitive(PrimitiveValue::Null)
//     ));
// }
//
// #[test]
// fn test_vec_property_from_json() {
//     let json_value = json!(["one", "two", "three"]);
//
//     let property =
//         <Vec<String> as InstancePropertyFromJson<TestParent>>::property_from_json(json_value)
//             .expect("Failed to convert array");
//
//     match property {
//         InstanceProperty::Primitives(values) => {
//             assert_eq!(values.len(), 3);
//             assert!(matches!(&values[0], PrimitiveValue::String(s) if s == "one"));
//             assert!(matches!(&values[1], PrimitiveValue::String(s) if s == "two"));
//             assert!(matches!(&values[2], PrimitiveValue::String(s) if s == "three"));
//         }
//         _ => panic!("Expected Primitives variant, got {:?}", property),
//     }
// }
//
// #[test]
// fn test_error_cases() {
//     // Test wrong type
//     let json_value = json!("not a number");
//     let result = <i32 as InstancePropertyFromJson<TestParent>>::property_from_json(json_value);
//     assert!(result.is_err());
//
//     // Test out of range
//     let json_value = json!(1000000000000i64); // Too big for i32
//     let result = <i32 as InstancePropertyFromJson<TestParent>>::property_from_json(json_value);
//     assert!(result.is_err());
// }
