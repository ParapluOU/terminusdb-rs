use crate::*;

#[derive(Clone, Copy, Debug /*TerminusDBSchema*/)]
pub enum Order {
    asc,
    desc,
}

impl ToRESTQuery for Order {
    fn to_rest_query_json(&self) -> serde_json::Value {
        // self.to_instance_document(None).to_json()
        todo!()
    }
}
