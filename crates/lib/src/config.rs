use serde::{Deserialize, Serialize};

/// Configuration for image processing limits
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ImageConfig {
    /// Maximum megapixels allowed for input images
    pub max_megapixels: Option<f64>,
    /// Maximum resolution (largest dimension) allowed for output images
    pub max_output_resolution: Option<u32>,
}
