use schemas::{Dita13, SchemaBundle};
fn main() {
    Dita13::write_to_directory(std::path::Path::new("/tmp/dita13_check")).unwrap();
}
