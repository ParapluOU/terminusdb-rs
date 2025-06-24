use crate::{EntityIDFor, FromTDBInstance, ToTDBInstance};

pub trait TdbModel : ToTDBInstance + FromTDBInstance {
    fn instance_id(&self) -> Option<EntityIDFor<Self>> {
        match self.to_instance(None).gen_id() {
            None => {None}
            Some(id) => {
                EntityIDFor::new(&id).unwrap().into()
            }
        }
    }
}

impl<T> TdbModel for T where T: ToTDBInstance + FromTDBInstance {

}