#[derive(Debug, Clone)]
pub struct VizierError(pub String);

pub fn throw_vizier_error<T, E: std::error::Error>(prefix: &str, err: E) -> Result<T, VizierError> {
    tracing::error!("{}: {}", prefix, err);
    Err(VizierError(format!("{}: {}", prefix, err.to_string())))
}

impl std::fmt::Display for VizierError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for VizierError {}

impl<T> From<VizierError> for crate::Result<T> {
    fn from(value: VizierError) -> Self {
        crate::Result::Err(value)
    }
}
