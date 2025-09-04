#![allow(warnings)]

mod enum_simple_test;
mod enum_union_test;
mod from_instance_test;
mod instance_test;
mod integration_test;
mod json_deserialize;
mod rename_test;
mod special_types_test;
mod struct_test;
mod vec_string_test;

#[cfg(feature = "generic-derive")]
mod generic_test;

#[cfg(feature = "generic-derive")]
mod generic_with_model_test;

#[cfg(feature = "generic-derive")]
mod simple_generic_test;

#[cfg(feature = "generic-derive")]
mod demo_generic_test;

#[cfg(feature = "generic-derive")]
mod generic_works_test;

#[cfg(feature = "generic-derive")]
mod generic_syntax_test;

#[cfg(feature = "generic-derive")]
mod generic_basic_test;

#[cfg(not(feature = "generic-derive"))]
mod generic_error_test;
