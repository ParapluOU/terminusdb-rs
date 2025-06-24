/// when the object is a primitive, this method is called
/// to determine to built-in schema type
pub trait ToSchemaClass {
    fn to_class() -> &'static str;
}

// impl<T: ToTDBSchema> ToSchemaClass for PhantomData<T> {
//     fn to_class() -> &'static str {
//         UNIT
//     }
// }

// impl<T: ToSchemaClass> ToSchemaClass for Box<T> {
//     fn to_class() -> &'static str {
//         T::to_class()
//     }
// }

// impl<T: ToTDBInstance> !ToSchemaClass for T {}