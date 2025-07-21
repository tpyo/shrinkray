## shrinkray

Shrinkray is a lightweight image processing proxy built in Rust.

### Features

- On-the-fly resizing, cropping and format conversion
- Configurable routing for file, HTTP or S3 backends
- Optional HMAC signatures to secure image URLs
- Prometheus metrics and OpenTelemetry tracing

### Local Development

Run `docker-compose up` to start a development instance listening on port 9090.


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
