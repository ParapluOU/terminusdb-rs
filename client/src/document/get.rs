#[derive(Clone, Copy, Debug)]
pub struct GetOpts {
    pub unfold: bool,
    pub as_list: bool,
}

impl Default for GetOpts {
    fn default() -> Self {
        Self {
            unfold: false,
            as_list: false,
        }
    }
}
