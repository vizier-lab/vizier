use anyhow::{Result, bail};
use pyo3::Python;

/// Check if Python is available and meets minimum version requirement (3.9+)
pub fn check_python_version() -> Result<()> {
    Python::attach(|py| {
        let version = py.version_info();
        let major = version.major;
        let minor = version.minor;

        if major < 3 || (major == 3 && minor < 9) {
            bail!(
                "Python version {}.{} detected, but Python 3.9 or higher is required.\n\
                 \n\
                 Please install Python 3.9+:\n\
                 - macOS: brew install python@3.9\n\
                 - Ubuntu/Debian: sudo apt-get install python3.9 python3.9-dev\n\
                 - Windows: Download from https://www.python.org/downloads/",
                major,
                minor
            );
        }

        info!("Python version {}.{} detected", major, minor);
        Ok(())
    })
}
