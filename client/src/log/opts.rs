#[derive(Clone, Copy, Debug, Default)]
pub struct LogOpts {
    pub offset: Option<usize>,
    pub count: Option<usize>,
    pub verbose: bool,
}
