// Special wrapper type for Vec<f32> deserialization
#[derive(Debug, Clone)]
pub struct F32Vector(pub Vec<f32>);

// Allow for easy conversion from F32Vector to Vec<f32>
impl From<F32Vector> for Vec<f32> {
    fn from(vector: F32Vector) -> Self {
        vector.0
    }
}
