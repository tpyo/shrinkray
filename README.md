# shrinkray

[![Release image](https://github.com/tpyo/shrinkray/actions/workflows/tag-image.yml/badge.svg)](https://github.com/tpyo/shrinkray/actions/workflows/tag-image.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)


Shrinkray is a lightweight, high-performance image proxy written in Rust.

### Features

- On-the-fly resizing, cropping and format conversion
- Configurable routing for file, HTTP or S3 backends
- Optional HMAC signatures to secure image URLs
- Prometheus metrics and OpenTelemetry tracing

### Use Cases

- Offload image resizing logic from your app server
- Serve responsive images without storing multiple variants
- Optimize external images at request time
- Use as a backend to a CDN for on-demand image transformation

### Inspiration

Shrinkray draws inspiration from other great image processing services, including:

- [dali](https://github.com/olxgroup-oss/dali) – a flexible image server  
- [imgproxy](https://github.com/imgproxy/imgproxy) – a high-performance Golang image proxy  
- [imgix](https://www.imgix.com) – a commercial image optimization platform  

## Running shrinkray

### Kubernetes Deployment

Download and edit the Kubernetes [config file](https://github.com/tpyo/shrinkray/blob/main/kubernetes/config.yaml):

```bash
curl -O https://github.com/tpyo/shrinkray/blob/main/kubernetes/config.yaml
```

Apply the Kubernetes manifests:

```bash
kubectl apply -f config.yaml
kubectl apply -f https://github.com/tpyo/shrinkray/blob/main/kubernetes/deployment.yaml
```

### Local Development

Run `docker-compose up` to start a development instance listening on http://localhost:9090.

Jaeger tracing is available at http://localhost:16686.

### Example URL parameters

#### Resize with crop fit
http://localhost:9090/samples/02.jpg?w=400&h=400&dpr=2&fit=crop
- Resizes to 800×800 at double device pixel ratio (for retina screens)
- Fits by cropping to fill the dimensions

#### Resize with clip fit
http://localhost:9090/samples/04.jpg?w=1024&h=768
- Fits within 1024×768 without cropping

#### Resize to aspect ratio
http://localhost:9090/samples/03.jpg?ar=4:3&w=400
- Resizes to 400x300 with cropping

#### Trim whitespace
http://localhost:9090/samples/trim.jpg?trim=auto
- Trim colour can be set with `trim=colour` and `trim-colour=ffffff` 

#### Rotatation
http://localhost:9090/samples/01.jpg?rot=180

#### Monochrome filter
http://localhost:9090/samples/08.jpg?monochrome=100

#### Blur
http://localhost:9090/samples/08.jpg?blur=100


## Parameters

| Parameter     | Description                                              |
| ------------- | -------------------------------------------------------- |
| `w`           | Width in pixels                                          |
| `h`           | Height in pixels                                         |
| `bg`          | Background colour used when padding or flattening        |
| `ar`          | Aspect ratio (e.g. `16:9`)                               |
| `q`           | Output quality (default: 75)                             |
| `dpr`         | Device pixel ratio multiplier                            |
| `rot`         | Rotation in degrees (`90`, `180` or `270`)               |
| `fit`         | Resizing mode (`clip`, `crop`, `max`) (default: `clip`)  |
| `fm`          | Output format (`jpeg`, `webp`, `png`, `avif`)            |
| `dl`          | Download filename for the response                       |
| `lossless`    | Enable lossless encoding when available                  |
| `trim`        | Trim borders automatically (`auto`, `colour`)            |
| `trim-colour` | Set the trim colour for the `trim` parameter             |
| `sharpen`     | Adjust sharpness (0-100)                                 |
| `blur`        | Apply a blur (0-100)                                     |
| `kodachrome`  | Filter application (0-100)                               |
| `vintage`     | Filter application (0-100)                               |
| `polaroid`    | Filter application (0-100)                               |
| `technicolor` | Filter application (0-100)                               |
| `sepia`       | Filter application (0-100)                               |
| `monochrome`  | Filter application (0-100)                               |
| `sig`         | HMAC signature used by `sign()` for request verification |


## Management service

- http://localhost:9091/metrics - Prometheus metrics endpoint
- http://localhost:9091/healthz - Health endpoint
