use schemas::{Dita12, SchemaBundle};
fn main() {
    Dita12::write_to_directory(std::path::Path::new("/tmp/dita12_check")).unwrap();
}
