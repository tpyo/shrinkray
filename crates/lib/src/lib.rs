pub mod config;
pub mod image;
pub mod options;

use config::ImageConfig;
use libvips::Result as VipsResult;
use libvips::VipsApp;
use once_cell::sync::OnceCell;
use options::{
    AspectRatio, Colour, DuotoneColours, Fit, Format, ImageOptions, Percentage, Rotation, Trim,
};

pub fn create_vips_app() -> &'static VipsApp {
    // libvips requires global initialization and assumes there is only
    // one global VipsApp per process. Creating multiple instances of
    // VipsApp::new(...) in the same test binary (even across different
    // tests) will lead to undefined behavior.
    static VIPS: OnceCell<VipsApp> = OnceCell::new();
    VIPS.get_or_init(|| {
        let app = VipsApp::new("shrinkray", false).expect("failed to initialize libvips");
        app.cache_set_max(0);
        app.cache_set_max_mem(0);
        app
    })
}

/// A processed image result containing the output bytes and metadata
pub struct ProcessedImage {
    bytes: Vec<u8>,
    format: Format,
}

impl ProcessedImage {
    /// Get the processed image bytes
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Consume and return the image bytes
    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }

    /// Get the format of the processed image
    pub fn format(&self) -> Format {
        self.format
    }

    /// Get the MIME type string for the processed image
    pub fn mime_type(&self) -> &'static str {
        self.format.content_type()
    }
}

/// A fluent builder for processing images with a user-friendly API
///
/// # Example
///
/// ```no_run
/// use shrinkray::ImageProcessor;
/// use shrinkray::options::{Format, Fit};
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let image_bytes = std::fs::read("input.jpg")?;
///
/// let result = ImageProcessor::new(&image_bytes)
///     .resize(800, 600)
///     .quality(85)
///     .format(Format::Webp)
///     .fit(Fit::Crop)
///     .sharpen(50)
///     .process()?;
///
/// std::fs::write("output.webp", result.bytes())?;
/// # Ok(())
/// # }
/// ```
pub struct ImageProcessor<'a> {
    bytes: &'a [u8],
    options: ImageOptions,
    config: ImageConfig,
}

impl<'a> ImageProcessor<'a> {
    /// Create a new ImageProcessor with the given image bytes
    pub fn new(bytes: &'a [u8]) -> Self {
        Self {
            bytes,
            options: ImageOptions::default(),
            config: ImageConfig::default(),
        }
    }

    /// Set the output width in pixels
    pub fn width(mut self, width: i32) -> Self {
        self.options.width = Some(width);
        self
    }

    /// Set the output height in pixels
    pub fn height(mut self, height: i32) -> Self {
        self.options.height = Some(height);
        self
    }

    /// Set both width and height in one call
    pub fn resize(mut self, width: i32, height: i32) -> Self {
        self.options.width = Some(width);
        self.options.height = Some(height);
        self
    }

    /// Create a thumbnail with the given size
    pub fn thumbnail(mut self, size: i32) -> Self {
        self.options.width = Some(size);
        self.options.height = Some(size);
        self.options.fit = Some(Fit::Crop);
        self
    }

    /// Set the output quality (1-100)
    pub fn quality(mut self, quality: i32) -> Self {
        self.options.quality = Some(quality);
        self
    }

    /// Set the output format
    pub fn format(mut self, format: Format) -> Self {
        self.options.format = Some(format);
        self
    }

    /// Set the fit mode for resizing
    pub fn fit(mut self, fit: Fit) -> Self {
        self.options.fit = Some(fit);
        self
    }

    /// Set the device pixel ratio
    pub fn device_pixel_ratio(mut self, dpr: i32) -> Self {
        self.options.device_pixel_ratio = Some(dpr);
        self
    }

    /// Set the aspect ratio (e.g., 16, 9 for 16:9)
    pub fn aspect_ratio(mut self, x: i32, y: i32) -> Self {
        self.options.aspect_ratio = Some(AspectRatio::from_dimensions(x, y));
        self
    }

    /// Set the background colour for flattening transparent images
    pub fn background(mut self, r: u8, g: u8, b: u8) -> Self {
        self.options.background = Some(Colour { r, g, b });
        self
    }

    /// Set the background colour using a Colour type
    pub fn with_background(mut self, colour: Colour) -> Self {
        self.options.background = Some(colour);
        self
    }

    /// Rotate the image by the specified angle (90, 180, or 270 degrees)
    pub fn rotate(mut self, angle: u16) -> Self {
        self.options.rotate = Some(Rotation(angle));
        self
    }

    /// Set rotation using a Rotation type
    pub fn with_rotation(mut self, rotation: Rotation) -> Self {
        self.options.rotate = Some(rotation);
        self
    }

    /// Enable lossless compression
    pub fn lossless(mut self, lossless: bool) -> Self {
        self.options.lossless = Some(lossless);
        self
    }

    /// Enable automatic trimming of whitespace/transparent borders
    pub fn trim(mut self) -> Self {
        self.options.trim = Some(Trim::Auto);
        self
    }

    /// Trim borders matching the specified colour
    pub fn trim_colour(mut self, r: u8, g: u8, b: u8) -> Self {
        self.options.trim = Some(Trim::Colour);
        self.options.trim_colour = Some(Colour { r, g, b });
        self
    }

    /// Apply sharpening (1-100, where higher values mean more sharpening)
    pub fn sharpen(mut self, amount: u8) -> Self {
        self.options.sharpen = Some(Percentage(amount));
        self
    }

    /// Apply blur (1-100, where higher values mean more blur)
    pub fn blur(mut self, amount: u8) -> Self {
        self.options.blur = Some(Percentage(amount));
        self
    }

    /// Apply kodachrome filter (1-100 for intensity)
    pub fn kodachrome(mut self, intensity: u8) -> Self {
        self.options.kodachrome = Some(Percentage(intensity));
        self
    }

    /// Apply technicolor filter (1-100 for intensity)
    pub fn technicolor(mut self, intensity: u8) -> Self {
        self.options.technicolor = Some(Percentage(intensity));
        self
    }

    /// Apply vintage filter (1-100 for intensity)
    pub fn vintage(mut self, intensity: u8) -> Self {
        self.options.vintage = Some(Percentage(intensity));
        self
    }

    /// Apply polaroid filter (1-100 for intensity)
    pub fn polaroid(mut self, intensity: u8) -> Self {
        self.options.polaroid = Some(Percentage(intensity));
        self
    }

    /// Apply sepia filter (1-100 for intensity)
    pub fn sepia(mut self, intensity: u8) -> Self {
        self.options.sepia = Some(Percentage(intensity));
        self
    }

    /// Apply monochrome/grayscale filter (1-100 for intensity)
    pub fn monochrome(mut self, intensity: u8) -> Self {
        self.options.monochrome = Some(Percentage(intensity));
        self
    }

    /// Shorthand for monochrome filter
    pub fn grayscale(self) -> Self {
        self.monochrome(100)
    }

    /// Apply tint with the specified colour
    pub fn tint(mut self, r: u8, g: u8, b: u8) -> Self {
        self.options.tint = Some(Colour { r, g, b });
        self
    }

    /// Apply tint using a validated Colour type
    pub fn with_tint(mut self, colour: Colour) -> Self {
        self.options.tint = Some(colour);
        self
    }

    /// Apply duotone effect with shadow and highlight colours
    pub fn duotone(
        mut self,
        shadow_r: u8,
        shadow_g: u8,
        shadow_b: u8,
        highlight_r: u8,
        highlight_g: u8,
        highlight_b: u8,
    ) -> Self {
        self.options.duotone = Some(DuotoneColours {
            shadow: Colour {
                r: shadow_r,
                g: shadow_g,
                b: shadow_b,
            },
            highlight: Colour {
                r: highlight_r,
                g: highlight_g,
                b: highlight_b,
            },
        });
        self
    }

    /// Apply duotone effect using validated DuotoneColours type
    pub fn with_duotone(mut self, duotone: DuotoneColours) -> Self {
        self.options.duotone = Some(duotone);
        self
    }

    /// Set the opacity/alpha for the duotone effect (1-100)
    pub fn duotone_alpha(mut self, alpha: u8) -> Self {
        self.options.duotone_alpha = Some(Percentage(alpha));
        self
    }

    /// Set the maximum megapixels allowed for input images
    pub fn max_megapixels(mut self, megapixels: f64) -> Self {
        self.config.max_megapixels = Some(megapixels);
        self
    }

    /// Set the maximum resolution (largest dimension) allowed for output images
    pub fn max_output_resolution(mut self, resolution: u32) -> Self {
        self.config.max_output_resolution = Some(resolution);
        self
    }

    /// Process the image with the configured options
    ///
    /// This method initializes the VipsApp (if not already initialized),
    /// processes the image according to the configured options, and returns
    /// a ProcessedImage containing the output bytes and metadata.
    pub fn process(mut self) -> VipsResult<ProcessedImage> {
        // Ensure VipsApp is initialized
        let _vips = create_vips_app();

        // Create a tracing span for the operation
        let span = tracing::span!(tracing::Level::INFO, "image_processing");

        // Process the image
        let vips_image = image::process_image(self.bytes, &mut self.options, &self.config, span)?;

        // Output the image
        let output = image::output(&vips_image, &mut self.options)?;

        Ok(ProcessedImage {
            bytes: output.bytes,
            format: output.content_type,
        })
    }

    /// Get a reference to the underlying ImageOptions
    pub fn options(&self) -> &ImageOptions {
        &self.options
    }

    /// Get a mutable reference to the underlying ImageOptions
    pub fn options_mut(&mut self) -> &mut ImageOptions {
        &mut self.options
    }

    /// Get a reference to the underlying ImageConfig
    pub fn config(&self) -> &ImageConfig {
        &self.config
    }

    /// Get a mutable reference to the underlying ImageConfig
    pub fn config_mut(&mut self) -> &mut ImageConfig {
        &mut self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_image_processor_basic() {
        let result = ImageProcessor::new(include_bytes!("../tests/sources/test.jpg"))
            .resize(300, 200)
            .quality(80)
            .format(Format::Jpeg)
            .process()
            .expect("unable to process image");

        assert!(!result.bytes().is_empty());
        assert_eq!(result.format(), Format::Jpeg);
        assert_eq!(result.mime_type(), "image/jpeg");
    }

    #[test]
    fn test_image_processor_thumbnail() {
        let result = ImageProcessor::new(include_bytes!("../tests/sources/test.jpg"))
            .thumbnail(200)
            .process()
            .expect("unable to process image");

        assert!(!result.bytes().is_empty());
    }

    #[test]
    fn test_image_processor_filters() {
        let result = ImageProcessor::new(include_bytes!("../tests/sources/test.jpg"))
            .vintage(80)
            .sharpen(40)
            .blur(20)
            .process()
            .expect("unable to process image");

        assert!(!result.bytes().is_empty());
    }

    #[test]
    fn test_image_processor_grayscale() {
        let result = ImageProcessor::new(include_bytes!("../tests/sources/test.jpg"))
            .grayscale()
            .process()
            .expect("unable to process image");

        assert!(!result.bytes().is_empty());
    }

    #[test]
    fn test_image_processor_duotone() {
        let result = ImageProcessor::new(include_bytes!("../tests/sources/test.jpg"))
            .duotone(0, 50, 100, 255, 165, 0)
            .duotone_alpha(75)
            .process()
            .expect("unable to process image");

        assert!(!result.bytes().is_empty());
    }

    #[test]
    fn test_image_processor_rotate() {
        let result = ImageProcessor::new(include_bytes!("../tests/sources/test.jpg"))
            .rotate(90)
            .process()
            .expect("unable to process image");

        assert!(!result.bytes().is_empty());
    }

    #[test]
    fn test_image_processor_background() {
        let result = ImageProcessor::new(include_bytes!("../tests/sources/flatten.png"))
            .background(255, 255, 255)
            .process()
            .expect("unable to process image");

        assert!(!result.bytes().is_empty());
    }

    #[test]
    fn test_image_processor_complex_chain() {
        let result = ImageProcessor::new(include_bytes!("../tests/sources/test.jpg"))
            .resize(800, 600)
            .fit(Fit::Crop)
            .quality(85)
            .format(Format::Jpeg)
            .sharpen(50)
            .vintage(60)
            .process()
            .expect("unable to process image");

        assert!(!result.bytes().is_empty());
        assert_eq!(result.format(), Format::Jpeg);
    }

    #[test]
    fn test_processed_image_into_bytes() {
        let result = ImageProcessor::new(include_bytes!("../tests/sources/test.jpg"))
            .resize(100, 100)
            .process()
            .expect("unable to process image");

        let bytes = result.into_bytes();
        assert!(!bytes.is_empty());
    }

    #[test]
    fn test_processed_image_content_type() {
        let result = ImageProcessor::new(include_bytes!("../tests/sources/test.jpg"))
            .format(Format::Png)
            .process()
            .expect("unable to process image");

        assert_eq!(result.format(), Format::Png);
        assert_eq!(result.mime_type(), "image/png");
    }

    #[test]
    fn test_image_processor_width_height() {
        let result = ImageProcessor::new(include_bytes!("../tests/sources/test.jpg"))
            .width(400)
            .height(300)
            .process()
            .expect("unable to process image");

        assert!(!result.bytes().is_empty());
    }

    #[test]
    fn test_image_processor_device_pixel_ratio() {
        let result = ImageProcessor::new(include_bytes!("../tests/sources/test.jpg"))
            .resize(200, 150)
            .device_pixel_ratio(2)
            .process()
            .expect("unable to process image");

        assert!(!result.bytes().is_empty());
    }

    #[test]
    fn test_image_processor_aspect_ratio() {
        let result = ImageProcessor::new(include_bytes!("../tests/sources/test.jpg"))
            .aspect_ratio(16, 9)
            .width(400)
            .process()
            .expect("unable to process image");

        assert!(!result.bytes().is_empty());
    }

    #[test]
    fn test_image_processor_with_background() {
        let colour = Colour {
            r: 128,
            g: 128,
            b: 128,
        };
        let result = ImageProcessor::new(include_bytes!("../tests/sources/flatten.png"))
            .with_background(colour)
            .process()
            .expect("unable to process image");

        assert!(!result.bytes().is_empty());
    }

    #[test]
    fn test_image_processor_with_rotation() {
        let rotation = Rotation(180);
        let result = ImageProcessor::new(include_bytes!("../tests/sources/test.jpg"))
            .with_rotation(rotation)
            .process()
            .expect("unable to process image");

        assert!(!result.bytes().is_empty());
    }

    #[test]
    fn test_image_processor_lossless() {
        let result = ImageProcessor::new(include_bytes!("../tests/sources/test.jpg"))
            .format(Format::Webp)
            .lossless(true)
            .process()
            .expect("unable to process image");

        assert!(!result.bytes().is_empty());
    }

    #[test]
    fn test_image_processor_trim() {
        let result = ImageProcessor::new(include_bytes!("../tests/sources/test.jpg"))
            .trim()
            .process()
            .expect("unable to process image");

        assert!(!result.bytes().is_empty());
    }

    #[test]
    fn test_image_processor_trim_colour() {
        let result = ImageProcessor::new(include_bytes!("../tests/sources/test.jpg"))
            .trim_colour(255, 255, 255)
            .process()
            .expect("unable to process image");

        assert!(!result.bytes().is_empty());
    }

    #[test]
    fn test_image_processor_kodachrome() {
        let result = ImageProcessor::new(include_bytes!("../tests/sources/test.jpg"))
            .kodachrome(70)
            .process()
            .expect("unable to process image");

        assert!(!result.bytes().is_empty());
    }

    #[test]
    fn test_image_processor_technicolor() {
        let result = ImageProcessor::new(include_bytes!("../tests/sources/test.jpg"))
            .technicolor(60)
            .process()
            .expect("unable to process image");

        assert!(!result.bytes().is_empty());
    }

    #[test]
    fn test_image_processor_polaroid() {
        let result = ImageProcessor::new(include_bytes!("../tests/sources/test.jpg"))
            .polaroid(50)
            .process()
            .expect("unable to process image");

        assert!(!result.bytes().is_empty());
    }

    #[test]
    fn test_image_processor_sepia() {
        let result = ImageProcessor::new(include_bytes!("../tests/sources/test.jpg"))
            .sepia(75)
            .process()
            .expect("unable to process image");

        assert!(!result.bytes().is_empty());
    }

    #[test]
    fn test_image_processor_tint() {
        let result = ImageProcessor::new(include_bytes!("../tests/sources/test.jpg"))
            .tint(100, 150, 200)
            .process()
            .expect("unable to process image");

        assert!(!result.bytes().is_empty());
    }

    #[test]
    fn test_image_processor_with_tint() {
        let colour = Colour {
            r: 200,
            g: 100,
            b: 50,
        };
        let result = ImageProcessor::new(include_bytes!("../tests/sources/test.jpg"))
            .with_tint(colour)
            .process()
            .expect("unable to process image");

        assert!(!result.bytes().is_empty());
    }

    #[test]
    fn test_image_processor_with_duotone() {
        let duotone = DuotoneColours {
            shadow: Colour {
                r: 50,
                g: 50,
                b: 100,
            },
            highlight: Colour {
                r: 255,
                g: 200,
                b: 100,
            },
        };
        let result = ImageProcessor::new(include_bytes!("../tests/sources/test.jpg"))
            .with_duotone(duotone)
            .process()
            .expect("unable to process image");

        assert!(!result.bytes().is_empty());
    }

    #[test]
    fn test_image_processor_max_megapixels() {
        let result = ImageProcessor::new(include_bytes!("../tests/sources/test.jpg"))
            .max_megapixels(10.0)
            .resize(200, 200)
            .process()
            .expect("unable to process image");

        assert!(!result.bytes().is_empty());
    }

    #[test]
    fn test_image_processor_max_output_resolution() {
        let result = ImageProcessor::new(include_bytes!("../tests/sources/test.jpg"))
            .max_output_resolution(2000)
            .resize(300, 300)
            .process()
            .expect("unable to process image");

        assert!(!result.bytes().is_empty());
    }

    #[test]
    fn test_image_processor_options_accessor() {
        let processor = ImageProcessor::new(include_bytes!("../tests/sources/test.jpg"))
            .resize(400, 300)
            .quality(90);

        let options = processor.options();
        assert_eq!(options.width, Some(400));
        assert_eq!(options.height, Some(300));
        assert_eq!(options.quality, Some(90));
    }

    #[test]
    fn test_image_processor_options_mut_accessor() {
        let mut processor = ImageProcessor::new(include_bytes!("../tests/sources/test.jpg"));

        processor.options_mut().width = Some(500);
        processor.options_mut().height = Some(400);

        assert_eq!(processor.options().width, Some(500));
        assert_eq!(processor.options().height, Some(400));
    }

    #[test]
    fn test_image_processor_config_accessor() {
        let processor =
            ImageProcessor::new(include_bytes!("../tests/sources/test.jpg")).max_megapixels(5.0);

        let config = processor.config();
        assert_eq!(config.max_megapixels, Some(5.0));
    }

    #[test]
    fn test_image_processor_config_mut_accessor() {
        let mut processor = ImageProcessor::new(include_bytes!("../tests/sources/test.jpg"));

        processor.config_mut().max_megapixels = Some(8.0);
        processor.config_mut().max_output_resolution = Some(3000);

        assert_eq!(processor.config().max_megapixels, Some(8.0));
        assert_eq!(processor.config().max_output_resolution, Some(3000));
    }
}
