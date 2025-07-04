use crate::{EntityIDFor, FromTDBInstance, InstanceFromJson, ToTDBInstance};

pub trait TerminusDBModel : ToTDBInstance + FromTDBInstance + InstanceFromJson + std::fmt::Debug {
    fn instance_id(&self) -> Option<EntityIDFor<Self>> {
        match self.to_instance(None).gen_id() {
            None => {None}
            Some(id) => {
                EntityIDFor::new(&id).unwrap().into()
            }
        }
    }
}

impl<T> TerminusDBModel for T where T: ToTDBInstance + FromTDBInstance + InstanceFromJson + std::fmt::Debug {

}