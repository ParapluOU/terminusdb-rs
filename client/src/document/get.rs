use crate::TerminusDBModel;

#[derive(Clone, Debug)]
pub struct GetOpts {
    pub unfold: bool,
    pub as_list: bool,
    /// Skip a certain number of documents (for pagination)
    pub skip: Option<usize>,
    /// Number of documents to retrieve (for pagination)
    pub count: Option<usize>,
    /// Filter documents by type (e.g., "Person")
    pub type_filter: Option<String>,
}

impl Default for GetOpts {
    fn default() -> Self {
        Self {
            unfold: false,
            as_list: false,
            skip: None,
            count: None,
            type_filter: None,
        }
    }
}

impl GetOpts {
    /// Create a new GetOpts with pagination settings
    pub fn paginated(skip: usize, count: usize) -> Self {
        Self {
            skip: Some(skip),
            count: Some(count),
            ..Default::default()
        }
    }

    /// Create a new GetOpts with type filtering
    pub fn filtered_by_type<T: TerminusDBModel>() -> Self {
        Self {
            type_filter: Some(T::to_schema().class_name().to_string()),
            ..Default::default()
        }
    }

    /// Set skip for chaining
    pub fn with_skip(mut self, skip: usize) -> Self {
        self.skip = Some(skip);
        self
    }

    /// Set count for chaining
    pub fn with_count(mut self, count: usize) -> Self {
        self.count = Some(count);
        self
    }

    /// Set type filter for chaining
    pub fn with_type_filter<T: TerminusDBModel>(mut self) -> Self {
        self.type_filter = Some(T::to_schema().class_name().to_string());
        self
    }

    /// Set type filter for chaining using a string (for cases where the type is not known at compile time)
    pub fn with_type_filter_string(mut self, type_name: &str) -> Self {
        self.type_filter = Some(type_name.to_string());
        self
    }

    /// Set unfold for chaining
    pub fn with_unfold(mut self, unfold: bool) -> Self {
        self.unfold = unfold;
        self
    }

    /// Set as_list for chaining
    pub fn with_as_list(mut self, as_list: bool) -> Self {
        self.as_list = as_list;
        self
    }
}
