# shrinkray

[![crates.io](https://img.shields.io/crates/v/shrinkray.svg)](https://crates.io/crates/shrinkray)
[![docs.rs](https://img.shields.io/docsrs/shrinkray)](https://docs.rs/shrinkray/latest/shrinkray/)
[![Main image](https://github.com/tpyo/shrinkray/actions/workflows/main-image.yml/badge.svg)](https://github.com/tpyo/shrinkray/actions/workflows/main-image.yml)
[![Release image](https://github.com/tpyo/shrinkray/actions/workflows/tag-image.yml/badge.svg)](https://github.com/tpyo/shrinkray/actions/workflows/tag-image.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![codecov](https://codecov.io/github/tpyo/shrinkray/graph/badge.svg?token=BXFB15WSLA)](https://codecov.io/github/tpyo/shrinkray)

Shrinkray is a high-performance image processing toolkit for Rust, consisting of:

- **[shrinkray](crates/lib)** - A fast image processing library powered by libvips
- **[shrinkray-server](crates/server)** - A lightweight image proxy server for on-the-fly transformations

## Requirements

- **libvips 8.17.3** (for older 8.16.x libvips versions use shrinkray 1.0.1)
- libaom (for AVIF support via libheif)
- libheif
- libjpeg
- libpng
- libwebp

For more information on installing the required dependencies, see the [Ubuntu](https://github.com/tpyo/shrinkray/blob/main/docker/Dockerfile.base-slim-bookworm) and [Alpine](https://github.com/tpyo/shrinkray/blob/main/docker/Dockerfile.base-alpine) base images.

## Library

Use the [shrinkray library](crates/lib) to integrate image processing directly into your Rust applications:

```rust
use shrinkray::ImageProcessor;
use shrinkray::options::{Format, Fit};

let result = ImageProcessor::new(&image_bytes)
    .resize(800, 600)
    .quality(85)
    .format(Format::Webp)
    .fit(Fit::Crop)
    .process()?;
```

See the [library documentation](crates/lib) for full API details and examples.

## Server

The [shrinkray-server](crates/server) provides an HTTP service for on-the-fly image transformations:

### Features

- On-the-fly resizing, cropping and format conversion
- Configurable routing for file, HTTP or S3 backends
- Optional HMAC signatures to secure image URLs
- Prometheus metrics and OpenTelemetry tracing

### Use Cases

- Offload image resizing logic from your app server
- Serve responsive images without storing multiple variants
- Optimise external images at request time
- Use as a backend to a CDN for on-demand image transformation

### Inspiration

Shrinkray draws inspiration from other great image processing services, including:

- [dali](https://github.com/olxgroup-oss/dali) – a flexible image server
- [imgproxy](https://github.com/imgproxy/imgproxy) – a high-performance Golang image proxy
- [imgix](https://www.imgix.com) – a commercial image optimisation platform

## Running the Server

### Docker Standalone

Run the Docker image standalone with your own configuration:

```bash
docker run -p 9000:9000 -p 9001:9001 -v $(pwd)/config/config.json:/opt/shrinkray/etc/config.json ghcr.io/tpyo/shrinkray/server:latest /opt/shrinkray/etc/config.json
```

This mounts your local `config/config.json` file to the expected location in the container and starts a server listening on http://localhost:9000.

**Available image variants (arm64 and amd64):**
- `ghcr.io/tpyo/shrinkray/server:latest` - Debian slim-bookworm based image
- `ghcr.io/tpyo/shrinkray/server:latest-alpine` - Alpine Linux based image (smaller size)

### Kubernetes Deployment

Download and edit the Kubernetes [config file](https://github.com/tpyo/shrinkray/blob/main/kubernetes/config.yaml):

```bash
curl -O https://raw.githubusercontent.com/tpyo/shrinkray/refs/heads/main/kubernetes/config.yaml
```

Apply the Kubernetes manifests:

```bash
kubectl apply -f config.yaml
kubectl apply -f https://raw.githubusercontent.com/tpyo/shrinkray/refs/heads/main/kubernetes/deployment.yaml
```

### Local Development

Run `docker-compose up` to start a development instance listening on http://localhost:9000.

Grafana is available at http://localhost:3000.

### Example URL parameters

#### Resize with crop fit

http://localhost:9000/samples/02.jpg?w=400&h=400&dpr=2&fit=crop

- Resizes to 800×800 at double device pixel ratio (for retina screens)
- Fits by cropping to fill the dimensions

#### Resize with clip fit

http://localhost:9000/samples/04.jpg?w=1024&h=768

- Fits within 1024×768 without cropping

#### Resize to aspect ratio

http://localhost:9000/samples/03.jpg?ar=4:3&w=400

- Resizes to 400x300 with cropping

#### Trim whitespace

http://localhost:9000/samples/trim.jpg?trim=auto

- Trim colour can be set with `trim=colour` and `trim-colour=ffffff`

#### Rotatation

http://localhost:9000/samples/01.jpg?rot=180

#### Monochrome filter

http://localhost:9000/samples/08.jpg?monochrome=100

#### Duotone filter

http://localhost:9000/samples/08.jpg?duotone=003263,ffa600

#### Duotone filter with opacity

http://localhost:9000/samples/08.jpg?duotone=003263,ffa600&duotone-alpha=50

- Applies duotone effect at 50% opacity, blending with original image

#### Color tint

http://localhost:9000/samples/08.jpg?tint=ff0000

- Applies a red color tint to the image

#### Blur

http://localhost:9000/samples/08.jpg?blur=100

## Parameters

| Parameter       | Description                                              |
| --------------- | -------------------------------------------------------- |
| `w`             | Width in pixels                                          |
| `h`             | Height in pixels                                         |
| `bg`            | Background colour used when padding or flattening        |
| `ar`            | Aspect ratio (e.g. `16:9`)                               |
| `q`             | Output quality (default: `75`)                           |
| `dpr`           | Device pixel ratio multiplier                            |
| `rot`           | Rotation in degrees (`90`, `180` or `270`)               |
| `fit`           | Resizing mode (`clip`, `crop`, `max`) (default: `clip`)  |
| `fm`            | Output format (`jpeg`, `webp`, `png`, `avif`)            |
| `dl`            | Download filename for the response                       |
| `lossless`      | Enable lossless encoding when available                  |
| `trim`          | Trim borders automatically (`auto`, `colour`)            |
| `trim-colour`   | Set the trim colour for the `trim` parameter             |
| `sharpen`       | Adjust sharpness (0-100)                                 |
| `blur`          | Apply a blur (0-100)                                     |
| `tint`          | Apply a colour tint (e.g; `ff0000`)                      |
| `kodachrome`    | Filter application (0-100)                               |
| `vintage`       | Filter application (0-100)                               |
| `polaroid`      | Filter application (0-100)                               |
| `technicolor`   | Filter application (0-100)                               |
| `sepia`         | Filter application (0-100)                               |
| `monochrome`    | Filter application (0-100)                               |
| `duotone`       | Duotone (`shadow`,`highlight` - e.g; `003263,ffa600`)    |
| `duotone-alpha` | Duotone opacity/alpha (1-100) (default: `100`)           |
| `sig`           | HMAC signature used by `sign()` for request verification |

## Management service

- http://localhost:9001/metrics - Prometheus metrics endpoint
- http://localhost:9001/healthz - Health endpoint

## Prometheus Metrics

Shrinkray exports the following Prometheus metrics:

### Counters

| Metric                              | Description                     |
| ----------------------------------- | ------------------------------- |
| `shrinkray_http_response_200_count` | Count of successful responses   |
| `shrinkray_http_response_401_count` | Count of unauthorized responses |
| `shrinkray_http_response_404_count` | Count of not found responses    |
| `shrinkray_http_response_500_count` | Count of internal server errors |

### Histograms

| Metric                              | Labels    | Description                           |
| ----------------------------------- | --------- | ------------------------------------- |
| `shrinkray_fetch_duration_seconds`  | `backend` | Duration of image fetching operations |
| `shrinkray_output_duration_seconds` | `format`  | Duration of image encoding operations |

Both histogram metrics use buckets: `0.005, 0.01, 0.025, 0.05, 0.075, 0.1, 0.25, 0.5, 0.75, 1.0, 2.5, 5.0, 10.0` seconds.
