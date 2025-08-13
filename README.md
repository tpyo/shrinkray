# shrinkray
[![Release image](https://github.com/tpyo/shrinkray/actions/workflows/tag-image.yml/badge.svg)](https://github.com/tpyo/shrinkray/actions/workflows/tag-image.yml)

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

### Local Development

Run `docker-compose up` to start a development instance listening on port 9090.

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


## Image Options


| Parameter  | Description                                                  |
| ---------- | ------------------------------------------------------------ |
| `w`           | Width in pixels                                           |
| `h`           | Height in pixels                                          |
| `bg`          | Background colour used when padding or flattening         |
| `ar`          | Aspect ratio (e.g. `16:9`)                                |
| `q`           | Output quality (default: 75)                              |
| `dpr`         | Device pixel ratio multiplier                             |
| `rot`         | Rotation in degrees (`90`, `180` or `270`)                |
| `fit`         | Resizing mode (`clip`, `clamp`, `crop`, `max`)            |
| `fm`          | Output format (`jpeg`, `webp`, `png`, `avif`)             |
| `dl`          | Download filename for the response                        |
| `lossless`    | Enable lossless encoding when available                   |
| `trim`        | Trim borders automatically                                |
| `trim-colour` | Set the trim colour for the `trim` parameter              |
| `sharpen`     | Adjust sharpness (0-100)                                  |
| `blur`        | Apply a blur (0-100)                                      |
| `kodachrome`  | Filter application (0-100)                                | 
| `vintage`     | Filter application (0-100)                                | 
| `polaroid`    | Filter application (0-100)                                | 
| `technicolor` | Filter application (0-100)                                | 
| `sepia`       | Filter application (0-100)                                | 
| `monochrome`  | Filter application (0-100)                                | 
| `sig`         | HMAC signature used by `sign()` for request verification  |      

Example request:

```
https://img.example.com/photos/dog.jpg?w=800&h=600&fit=crop&fm=webp&q=80&dpr=2&bg=ffffff&ar=16:9&rot=90&lossless=true&sharpen=10&sepia=30&dl=dog.webp&sig=abcd1234
```

## Management service

- **http://localhost:9091/metrics** - Prometheus metrics endpoint
- **http://localhost:9091/healthz** - Health endpoint
