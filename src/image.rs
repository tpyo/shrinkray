use crate::config::Config;
use crate::options::{self, Percentage};
use libvips::ops;
use libvips::{Result as VipsResult, VipsImage};
use std::mem::discriminant;
use tracing::{error, warn};

pub struct Image {
    pub bytes: Vec<u8>,
    pub content_type: options::ImageFormat,
}

#[tracing::instrument(skip_all)]
pub fn flatten(image: &VipsImage, colour: &options::Colour) -> VipsResult<VipsImage> {
    if image.image_hasalpha() {
        let opts = ops::FlattenOptions {
            background: colour.into(),
            ..Default::default()
        };
        ops::flatten_with_opts(image, &opts)
    } else {
        // Image does not have an alpha channel, no need to flatten
        warn!("image does not have an alpha channel, skipping flatten operation");
        ops::copy(image)
    }
}

#[tracing::instrument(skip_all)]
fn find_trim(
    image: &VipsImage,
    options: &options::ImageOptions,
) -> VipsResult<(i32, i32, i32, i32)> {
    let mut opts = ops::FindTrimOptions {
        threshold: 40.0,
        background: vec![255.0, 255.0, 255.0],
        line_art: false,
    };

    if let Some(colour) = &options.trim_colour {
        opts.background = colour.into();
    }

    // Check if the image has alpha
    if opts.background.len() == 4 {
        // Flatten the image before trimming to avoid issues with alpha channels
        let flatten_opts = ops::FlattenOptions {
            // Use magenta as the background colour
            background: vec![255.0, 0.0, 255.0],
            ..Default::default()
        };
        let copy = ops::flatten_with_opts(image, &flatten_opts)?;

        // Fetch the new background colour from the top left corner
        opts.background = ops::getpoint(&copy, 0, 0)?;

        ops::find_trim_with_opts(&copy, &opts)
    } else {
        ops::find_trim_with_opts(image, &opts)
    }
}

#[tracing::instrument(skip_all)]
fn trim(image: &VipsImage, options: &options::ImageOptions) -> VipsResult<VipsImage> {
    match find_trim(image, options) {
        Ok((left, top, width, height)) => ops::extract_area(image, left, top, width, height),
        Err(err) => {
            // If the image is not trimmed, return the original image
            error!("unable to trim image: {}", err);
            ops::copy(image)
        }
    }
}

fn percent_to_value(p: i32, min: f64, max: f64) -> f64 {
    if (max - min).abs() < f64::EPSILON {
        return min;
    }
    let percent = f64::from(p).clamp(0.0, 100.0) / 100.0;
    min + (max - min) * percent
}

#[tracing::instrument(skip_all)]
fn sharpen(image: &VipsImage, options: &mut options::ImageOptions) -> VipsResult<VipsImage> {
    let percentage = options.sharpen.unwrap_or(Percentage(1));
    // min: 0.000001, max: 10, default: 0.5
    let sigma = percent_to_value(percentage.0, 0.000_001, 10.0);
    let opts = ops::SharpenOptions {
        sigma,
        ..Default::default()
    };
    ops::sharpen_with_opts(image, &opts)
}

#[tracing::instrument(skip_all)]
fn blur(image: &VipsImage, options: &mut options::ImageOptions) -> VipsResult<VipsImage> {
    let percentage = options.blur.unwrap_or(Percentage(1));
    // min: 0, max: 1000, default: 1.5
    let sigma = percent_to_value(percentage.0, 0.0, 50.0);
    let opts = ops::GaussblurOptions {
        min_ampl: 0.001, // min: 0.001, max: 1, default: 0.2
        precision: ops::Precision::Approximate,
    };
    ops::gaussblur_with_opts(image, sigma, &opts)
}

fn colourspace_is_srgb(image: &VipsImage) -> VipsResult<bool> {
    let interp = image.get_interpretation()?;
    let srgb = ops::Interpretation::Srgb;
    Ok(discriminant(&interp) == discriminant(&srgb))
}

#[tracing::instrument(skip_all)]
fn colourspace(image: &VipsImage) -> VipsResult<VipsImage> {
    if colourspace_is_srgb(image)? {
        Ok(image.clone())
    } else {
        ops::colourspace(image, ops::Interpretation::Srgb)
    }
}

/// Check if the image needs rotation based on EXIF data
fn needs_rotation(buffer: &[u8]) -> bool {
    match rexif::parse_buffer_quiet(buffer).0 {
        Ok(data) => data.entries.into_iter().any(|e| {
            if e.tag != rexif::ExifTag::Orientation {
                return false;
            }
            if let Some(v) = e.value.to_i64(0) {
                v != 0 && v != 1
            } else {
                false
            }
        }),
        Err(_) => false,
    }
}

#[tracing::instrument(skip_all)]
fn load(bytes: &[u8], random_access: bool) -> VipsResult<VipsImage> {
    // If rotation is needed, load the image with random access
    if random_access {
        VipsImage::new_from_buffer(bytes, "[access=VIPS_ACCESS_RANDOM]")
    } else {
        VipsImage::new_from_buffer(bytes, "[access=VIPS_ACCESS_SEQUENTIAL]")
    }
}

#[tracing::instrument(skip_all, parent = span)]
pub fn process_image(
    bytes: &[u8],
    options: &mut options::ImageOptions,
    config: &Config,
    span: tracing::Span,
) -> VipsResult<Image> {
    let rotation = options.rotate.is_some() || needs_rotation(bytes);
    let random_access = rotation || options.trim.is_some();

    let mut image = load(bytes, random_access)?;

    // Check maximum megapixels limit
    if let Some(max_megapixels) = config.max_megapixels {
        let width = image.get_width() as f64;
        let height = image.get_height() as f64;
        let megapixels = (width * height) / 1_000_000.0;

        if megapixels > max_megapixels {
            return Err(libvips::error::Error::OperationError(
                "image exceeds maximum allowed megapixels",
            ));
        }
    }

    // Rotation
    if rotation {
        image = rotate(&image, options)?;
    }

    // // Trim whitespace
    if options.trim.is_some() {
        image = trim(&image, options)?;
    }

    // Tint
    if let Some(tint_colour) = &options.tint {
        image = tint(&image, tint_colour)?;
    }

    // Duotone
    if let Some(duotone_colours) = &options.duotone {
        image = duotone(
            &image,
            &duotone_colours.shadow,
            &duotone_colours.highlight,
            options.duotone_alpha,
        )?;
    }

    // Flatten alpha image
    if let Some(background) = &options.background {
        image = flatten(&image, background)?;
    }

    // Resize
    if options.width.is_some() || options.height.is_some() {
        let image_width = image.get_width();
        let image_height = image.get_height();

        // Calculate crop dimensions
        options::calculate_dimensions(options, image_width, image_height);

        // Check maximum output resolution limit before resizing
        if let Some(max_resolution) = config.max_output_resolution {
            let output_width = options.width.unwrap_or(image_width) as u32;
            let output_height = options.height.unwrap_or(image_height) as u32;
            let max_dimension = output_width.max(output_height);

            if max_dimension > max_resolution {
                return Err(libvips::error::Error::OperationError(
                    "output resolution exceeds maximum allowed resolution",
                ));
            }
        }

        image = resize(&image, options, image_width, image_height)?;
    }

    // Sharpen
    if options.sharpen.is_some() {
        image = sharpen(&image, options)?;
    }

    // Blur
    if options.blur.is_some() {
        image = blur(&image, options)?;
    }

    // Filters
    if options.kodachrome.is_some() {
        image = apply_style(&image, KODACHROME, options.kodachrome)?;
    }
    if options.technicolor.is_some() {
        image = apply_style(&image, TECHNICOLOR, options.technicolor)?;
    }
    if options.polaroid.is_some() {
        image = apply_style(&image, POLAROID, options.polaroid)?;
    }
    if options.vintage.is_some() {
        image = apply_style(&image, VINTAGE, options.vintage)?;
    }
    if options.sepia.is_some() {
        image = apply_style(&image, SEPIA, options.sepia)?;
    }
    if options.monochrome.is_some() {
        image = apply_style(&image, MONOCHROME, options.monochrome)?;
    }

    // sRGB conversion
    if !colourspace_is_srgb(&image)? {
        image = colourspace(&image)?;
    }

    // Output the image
    output(&image, options, config)
}

#[tracing::instrument(skip_all)]
fn output(
    image: &VipsImage,
    options: &mut options::ImageOptions,
    _config: &Config,
) -> VipsResult<Image> {
    let format = options.format.unwrap_or(options::ImageFormat::Jpeg);
    tracing::Span::current().record("shrinkray.format", format.to_string());

    let start = std::time::Instant::now();

    let output = match format {
        options::ImageFormat::Jpeg => Ok(Image {
            bytes: ops::jpegsave_buffer_with_opts(image, &options.into())?,
            content_type: options::ImageFormat::Jpeg,
        }),
        options::ImageFormat::Webp => Ok(Image {
            bytes: ops::webpsave_buffer_with_opts(image, &options.into())?,
            content_type: options::ImageFormat::Webp,
        }),
        options::ImageFormat::Avif => Ok(Image {
            bytes: ops::heifsave_buffer_with_opts(image, &options.into())?,
            content_type: options::ImageFormat::Avif,
        }),
        options::ImageFormat::Png => Ok(Image {
            bytes: ops::pngsave_buffer_with_opts(image, &options.into())?,
            content_type: options::ImageFormat::Png,
        }),
    };
    crate::metrics::output_duration(start.elapsed(), &format.to_string());
    output
}

#[tracing::instrument(skip_all)]
fn rotate(image: &VipsImage, options: &options::ImageOptions) -> VipsResult<VipsImage> {
    let mut image = ops::autorot(image)?;
    if let Some(angle) = &options.rotate {
        image = ops::rotate(&image, angle.into())?;
        tracing::Span::current().record("shrinkray.rotate", i64::from(angle.0));
    }
    Ok(image)
}

#[allow(clippy::cast_possible_truncation)]
#[tracing::instrument(skip_all, fields(shrinkray.width = image_width, shrinkray.height = image_height))]
fn resize(
    image: &VipsImage,
    options: &options::ImageOptions,
    image_width: i32,
    image_height: i32,
) -> VipsResult<VipsImage> {
    let scale = options.get_resize_scale(image_width, image_height);
    let mut thumbnail_options = ops::ThumbnailImageOptions {
        import_profile: "sRGB".to_string(),
        export_profile: "sRGB".to_string(),
        crop: ops::Interesting::Centre,
        linear: false,
        size: ops::Size::Both,
        ..Default::default()
    };
    if options.height.is_some() {
        thumbnail_options.height = options.height.unwrap_or(0);
    } else {
        thumbnail_options.height = (f64::from(image_height) * scale) as i32;
    }
    ops::thumbnail_image_with_opts(image, options.width.unwrap_or(0), &thumbnail_options)
}

const KODACHROME: [f64; 9] = [
    1.12855, -0.39673, -0.03992, -0.16404, 1.08352, -0.05498, -0.16786, -0.56034, 1.60148,
];
const POLAROID: [f64; 9] = [
    1.438, -0.062, -0.062, -0.122, 1.378, -0.122, -0.016, -0.016, 1.483,
];
const VINTAGE: [f64; 9] = [
    0.62793, 0.32021, -0.03965, 0.02578, 0.64411, 0.03259, 0.0466, -0.08512, 0.52416,
];
const TECHNICOLOR: [f64; 9] = [
    1.91252, -0.85453, -0.09155, -0.30878, 1.76589, -0.10601, -0.2311, -0.75018, 1.84759,
];
const MONOCHROME: [f64; 9] = [
    0.299, 0.587, 0.114, 0.299, 0.587, 0.114, 0.299, 0.587, 0.114,
];
const SEPIA: [f64; 9] = [
    0.393, 0.769, 0.189, 0.349, 0.686, 0.168, 0.272, 0.534, 0.131,
];

#[tracing::instrument(skip_all)]
fn apply_style(
    image: &VipsImage,
    array: [f64; 9],
    opacity: Option<options::Percentage>,
) -> VipsResult<VipsImage> {
    let matrix = VipsImage::image_new_matrix_from_array(3, 3, &array)?;
    let mut overlay = ops::recomb(image, &matrix)?;

    // Convert to float band format to apply the opacity
    overlay = ops::cast(&overlay, ops::BandFormat::Float)?;

    overlay = if overlay.image_hasalpha() {
        overlay
    } else {
        ops::bandjoin_const(&overlay, &mut [255.0])?
    };

    let opacity = opacity.unwrap_or(options::Percentage(100));

    let multiply = [1.0, 1.0, 1.0, f64::from(opacity.0) / 100.0];
    let addition = [0.0, 0.0, 0.0, 0.0];
    let mut multiply = multiply.to_vec();
    let mut addition = addition.to_vec();
    overlay = ops::linear(&overlay, &mut multiply, &mut addition)?;

    if opacity == options::Percentage(100) {
        // Return the overlay image without blending
        return ops::cast(&overlay, ops::BandFormat::Uchar);
    }

    overlay = ops::composite_2(image, &overlay, ops::BlendMode::Over)?;

    overlay = ops::cast(&overlay, ops::BandFormat::Uchar)?;

    let colour = &options::Colour {
        r: 255,
        g: 255,
        b: 255,
    };

    let opts = ops::FlattenOptions {
        background: colour.into(),
        ..Default::default()
    };

    ops::flatten_with_opts(&overlay, &opts)
}

#[tracing::instrument(skip_all)]
pub fn tint(image: &VipsImage, colour: &options::Colour) -> VipsResult<VipsImage> {
    let type_before_tint = image.get_interpretation()?;

    // Extract alpha channel if present
    let alpha = if image.image_hasalpha() {
        Some(ops::extract_band(image, image.get_bands() - 1)?)
    } else {
        None
    };

    // Remove alpha from image for processing if present
    let work_image = if image.image_hasalpha() {
        let bands = image.get_bands();
        let mut band_images = Vec::new();
        for i in 0..bands - 1 {
            band_images.push(ops::extract_band(image, i)?);
        }
        ops::bandjoin(&mut band_images)?
    } else {
        ops::copy(image)?
    };

    // Convert tint colour to LAB space
    let tint_rgb = VipsImage::new_from_memory(
        &[colour.r, colour.g, colour.b],
        1,
        1,
        3,
        ops::BandFormat::Uchar,
    )?;
    let tint_lab = ops::colourspace(&tint_rgb, ops::Interpretation::Lab)?;
    let tint_lab_values = ops::getpoint(&tint_lab, 0, 0)?;

    // Generate 256 LAB values where L varies from 0-100, A and B are from tint colour
    let mut lut_data = Vec::with_capacity(256 * 3);

    for i in 0..256 {
        lut_data.push((i as f64 / 255.0) * 100.0); // Convert to LAB L range (0-100)
        lut_data.push(tint_lab_values[1]); // A from tint
        lut_data.push(tint_lab_values[2]); // B from tint
    }

    // Create lookup table image in LAB space
    let lut_data: Vec<u8> = lut_data.iter().map(|&x| x.round() as u8).collect();
    let lut = VipsImage::new_from_memory(&lut_data, 256, 1, 3, ops::BandFormat::Uchar)?;

    // Set LAB interpretation on the LUT
    let result = ops::copy_with_opts(
        &lut,
        &ops::CopyOptions {
            width: 256,
            height: 1,
            bands: 3,
            interpretation: ops::Interpretation::Lab,
            ..Default::default()
        },
    )?;

    let result = ops::colourspace(&result, type_before_tint)?;

    let grayscale = ops::colourspace(&work_image, ops::Interpretation::BW)?;

    let result = ops::maplut(&grayscale, &result)?;

    // Re-attach alpha channel if it was present
    if let Some(alpha_channel) = alpha {
        return ops::bandjoin(&mut [result, alpha_channel]);
    }

    Ok(result)
}

#[tracing::instrument(skip_all)]
pub fn duotone(
    image: &VipsImage,
    shadow_colour: &options::Colour,
    highlight_colour: &options::Colour,
    opacity: Option<options::Percentage>,
) -> VipsResult<VipsImage> {
    // Convert to grayscale
    let mut grayscale = libvips::ops::colourspace(image, libvips::ops::Interpretation::BW)?;

    // Extract alpha channel if present
    let alpha = if image.image_hasalpha() {
        Some(ops::extract_band(&grayscale, grayscale.get_bands() - 1)?)
    } else {
        None
    };

    // Remove alpha from grayscale if present
    if image.image_hasalpha() {
        grayscale = ops::extract_band(&grayscale, 0)?;
    }

    // Create a lookup table for duotone mapping
    // Generate 256 values (0-255) mapping shadow to highlight colours
    let mut lut_data = Vec::with_capacity(256 * 3);

    for i in 0..256 {
        let t = i as f64 / 255.0; // Normalize to 0-1

        // Interpolate between shadow and highlight colours
        let r = (shadow_colour.r as f64 * (1.0 - t) + highlight_colour.r as f64 * t) as u8;
        let g = (shadow_colour.g as f64 * (1.0 - t) + highlight_colour.g as f64 * t) as u8;
        let b = (shadow_colour.b as f64 * (1.0 - t) + highlight_colour.b as f64 * t) as u8;

        lut_data.push(r as f64);
        lut_data.push(g as f64);
        lut_data.push(b as f64);
    }

    // Create the lookup table image
    let lut_bytes: Vec<u8> = lut_data.iter().map(|&x| x as u8).collect();
    let lut = VipsImage::new_from_memory(&lut_bytes, 256, 1, 3, ops::BandFormat::Uchar)?;

    // Apply the lookup table to map grayscale values to duotone colours
    let mut result = ops::maplut(&grayscale, &lut)?;

    // Re-attach alpha channel if it was present
    if let Some(alpha_channel) = alpha {
        result = ops::bandjoin(&mut [result, alpha_channel])?;
    }

    // Handle opacity if specified
    let opacity = opacity.unwrap_or(options::Percentage(100));

    if opacity == options::Percentage(100) {
        // Return the result without blending
        return Ok(result);
    }

    // Convert to float band format to apply the opacity
    result = ops::cast(&result, ops::BandFormat::Float)?;

    // Add alpha channel if not present
    result = if result.image_hasalpha() {
        result
    } else {
        ops::bandjoin_const(&result, &mut [255.0])?
    };

    // Apply opacity to the alpha channel
    let multiply = [1.0, 1.0, 1.0, f64::from(opacity.0) / 100.0];
    let addition = [0.0, 0.0, 0.0, 0.0];
    let mut multiply = multiply.to_vec();
    let mut addition = addition.to_vec();
    result = ops::linear(&result, &mut multiply, &mut addition)?;

    // Composite with original image
    result = ops::composite_2(image, &result, ops::BlendMode::Over)?;

    // Convert back to uchar and flatten
    result = ops::cast(&result, ops::BandFormat::Uchar)?;

    let colour = &options::Colour {
        r: 255,
        g: 255,
        b: 255,
    };

    let opts = ops::FlattenOptions {
        background: colour.into(),
        ..Default::default()
    };

    ops::flatten_with_opts(&result, &opts)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ConfigRouting;
    use crate::service::Service;

    pub fn assert_result(buffer: &[u8], path: &str) {
        let expected = format!("tests/results/{}", path);
        let img_result = VipsImage::new_from_buffer(buffer, "").expect("unable to read image");
        let img_expected = VipsImage::new_from_file(&expected).expect("unable to read image");

        let avg_result = ops::avg(&img_result).expect("failed to get image avg");
        let avg_expected = ops::avg(&img_expected).expect("failed to get image avg");

        assert!(
            (avg_result - avg_expected).abs() < 1.0,
            "average pixel values differ: result={} expected={}",
            avg_result,
            avg_expected
        );
    }

    pub fn get_service() -> Service {
        Service {
            vips_app: crate::service::create_vips_app(),
            config: get_config(),
        }
    }

    pub fn get_config() -> Config {
        Config {
            server_address: "0.0.0.0:3000".parse().unwrap(),
            management_address: "0.0.0.0:3001".parse().unwrap(),
            max_megapixels: Some(100.0),
            max_output_resolution: Some(8000),
            read_timeout: 30,
            routing: vec![ConfigRouting {
                path: "{*path}".into(),
                endpoint: "file://../tests/sources/".into(),
            }],
            proxies: vec![],
            otel_collector_endpoint: None,
            signing_secret: None,
            s3: None,
        }
    }

    #[test]
    fn test_find_trim() {
        let _svc = get_service();
        let image = load(include_bytes!("../tests/sources/trim.jpg"), false)
            .expect("unable to load trim.jpg");

        let opts = options::ImageOptions {
            trim: Some(options::Trim::Auto),
            ..Default::default()
        };

        let (left, top, width, height) =
            find_trim(&image, &opts).expect("find_trim should succeed");

        assert!(left >= 0, "left should be non-negative");
        assert!(top >= 0, "top should be non-negative");
        assert!(width > 0, "width should be positive");
        assert!(height > 0, "height should be positive");

        // Verify trimmed area fits within original image
        assert!(
            left + width <= image.get_width(),
            "trimmed area should fit within image width"
        );
        assert!(
            top + height <= image.get_height(),
            "trimmed area should fit within image height"
        );
    }

    #[test]
    fn test_trim() {
        let svc = get_service();
        let mut opts = options::ImageOptions {
            trim: Some(options::Trim::Auto),
            ..Default::default()
        };
        let span = tracing::Span::current();

        let img = process_image(
            include_bytes!("../tests/sources/trim.jpg"),
            &mut opts,
            &svc.config,
            span,
        )
        .expect("unable to process image")
        .bytes;

        assert_result(&img, "trim.jpg");
    }

    #[test]
    fn test_resize() {
        let svc = get_service();
        let mut opts = options::ImageOptions {
            width: Some(300),
            height: Some(200),
            fit: Some(options::Fit::Crop),
            ..Default::default()
        };
        let span = tracing::Span::current();

        let img = process_image(
            include_bytes!("../tests/sources/test.jpg"),
            &mut opts,
            &svc.config,
            span,
        )
        .expect("unable to process image")
        .bytes;

        assert_result(&img, "resize.jpg");
    }

    #[test]
    fn test_duotone() {
        let svc = get_service();
        let mut opts = options::ImageOptions {
            duotone: Some(options::DuotoneColours {
                shadow: options::Colour {
                    r: 0,
                    g: 50,
                    b: 100,
                },
                highlight: options::Colour {
                    r: 255,
                    g: 165,
                    b: 0,
                },
            }),
            ..Default::default()
        };
        let span = tracing::Span::current();

        let img = process_image(
            include_bytes!("../tests/sources/test.jpg"),
            &mut opts,
            &svc.config,
            span,
        )
        .expect("unable to process image")
        .bytes;

        assert_result(&img, "duotone.jpg");
    }

    #[test]
    fn test_duotone_alpha() {
        let svc = get_service();
        let mut opts = options::ImageOptions {
            duotone: Some(options::DuotoneColours {
                shadow: options::Colour {
                    r: 0,
                    g: 50,
                    b: 100,
                },
                highlight: options::Colour {
                    r: 255,
                    g: 165,
                    b: 0,
                },
            }),
            duotone_alpha: Some(options::Percentage(50)),
            ..Default::default()
        };
        let span = tracing::Span::current();

        let img = process_image(
            include_bytes!("../tests/sources/test.jpg"),
            &mut opts,
            &svc.config,
            span,
        )
        .expect("unable to process image")
        .bytes;

        assert_result(&img, "duotone-alpha.jpg");
    }

    #[test]
    fn test_blur() {
        let svc = get_service();
        let mut opts = options::ImageOptions {
            blur: Some(options::Percentage(50)),
            ..Default::default()
        };
        let span = tracing::Span::current();

        let img = process_image(
            include_bytes!("../tests/sources/test.jpg"),
            &mut opts,
            &svc.config,
            span,
        )
        .expect("unable to process image")
        .bytes;

        assert_result(&img, "blur.jpg");
    }

    #[test]
    fn test_sharpen() {
        let svc = get_service();
        let mut opts = options::ImageOptions {
            sharpen: Some(options::Percentage(50)),
            ..Default::default()
        };
        let span = tracing::Span::current();

        let img = process_image(
            include_bytes!("../tests/sources/test.jpg"),
            &mut opts,
            &svc.config,
            span,
        )
        .expect("unable to process image")
        .bytes;

        assert_result(&img, "sharpen.jpg");
    }

    #[test]
    fn test_flatten() {
        let svc = get_service();
        let mut opts = options::ImageOptions {
            background: Some(options::Colour {
                r: 255,
                g: 0,
                b: 255,
            }),
            ..Default::default()
        };
        let span = tracing::Span::current();

        let img = process_image(
            include_bytes!("../tests/sources/flatten.png"),
            &mut opts,
            &svc.config,
            span,
        )
        .expect("unable to process image")
        .bytes;

        assert_result(&img, "flatten.jpg");
    }

    #[test]
    fn test_rotate() {
        let svc = get_service();
        let mut opts = options::ImageOptions {
            rotate: Some(options::Rotation(90)),
            ..Default::default()
        };
        let span = tracing::Span::current();

        let img = process_image(
            include_bytes!("../tests/sources/test.jpg"),
            &mut opts,
            &svc.config,
            span,
        )
        .expect("unable to process image")
        .bytes;

        assert_result(&img, "rotate.jpg");
    }

    #[test]
    fn test_kodachrome() {
        let svc = get_service();
        let mut opts = options::ImageOptions {
            kodachrome: Some(options::Percentage(100)),
            ..Default::default()
        };
        let span = tracing::Span::current();

        let img = process_image(
            include_bytes!("../tests/sources/test.jpg"),
            &mut opts,
            &svc.config,
            span,
        )
        .expect("unable to process image")
        .bytes;

        assert_result(&img, "kodachrome.jpg");
    }

    #[test]
    fn test_polaroid() {
        let svc = get_service();
        let mut opts = options::ImageOptions {
            polaroid: Some(options::Percentage(100)),
            ..Default::default()
        };
        let span = tracing::Span::current();

        let img = process_image(
            include_bytes!("../tests/sources/test.jpg"),
            &mut opts,
            &svc.config,
            span,
        )
        .expect("unable to process image")
        .bytes;

        assert_result(&img, "polaroid.jpg");
    }

    #[test]
    fn test_vintage() {
        let svc = get_service();
        let mut opts = options::ImageOptions {
            vintage: Some(options::Percentage(100)),
            ..Default::default()
        };
        let span = tracing::Span::current();

        let img = process_image(
            include_bytes!("../tests/sources/test.jpg"),
            &mut opts,
            &svc.config,
            span,
        )
        .expect("unable to process image")
        .bytes;

        assert_result(&img, "vintage.jpg");
    }

    #[test]
    fn test_technicolor() {
        let svc = get_service();
        let mut opts = options::ImageOptions {
            technicolor: Some(options::Percentage(100)),
            ..Default::default()
        };
        let span = tracing::Span::current();

        let img = process_image(
            include_bytes!("../tests/sources/test.jpg"),
            &mut opts,
            &svc.config,
            span,
        )
        .expect("unable to process image")
        .bytes;

        assert_result(&img, "technicolor.jpg");
    }

    #[test]
    fn test_monochrome() {
        let svc = get_service();
        let mut opts = options::ImageOptions {
            monochrome: Some(options::Percentage(100)),
            ..Default::default()
        };
        let span = tracing::Span::current();

        let img = process_image(
            include_bytes!("../tests/sources/test.jpg"),
            &mut opts,
            &svc.config,
            span,
        )
        .expect("unable to process image")
        .bytes;

        assert_result(&img, "monochrome.jpg");
    }

    #[test]
    fn test_sepia() {
        let svc = get_service();
        let mut opts = options::ImageOptions {
            sepia: Some(options::Percentage(100)),
            ..Default::default()
        };
        let span = tracing::Span::current();

        let img = process_image(
            include_bytes!("../tests/sources/test.jpg"),
            &mut opts,
            &svc.config,
            span,
        )
        .expect("unable to process image")
        .bytes;

        assert_result(&img, "sepia.jpg");
    }

    #[test]
    fn test_tint() {
        let svc = get_service();
        let mut opts = options::ImageOptions {
            tint: Some(options::Colour { r: 255, g: 0, b: 0 }),
            ..Default::default()
        };
        let span = tracing::Span::current();

        let img = process_image(
            include_bytes!("../tests/sources/test.jpg"),
            &mut opts,
            &svc.config,
            span,
        )
        .expect("unable to process image")
        .bytes;

        assert_result(&img, "tint.jpg");
    }
}
