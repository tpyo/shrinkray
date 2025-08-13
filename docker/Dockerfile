FROM public.ecr.aws/tpyo/shrinkray/base:latest AS build

ENV PKG_CONFIG_PATH="$PKG_CONFIG_PATH:/opt/shrinkray/lib/pkgconfig"
ENV LD_LIBRARY_PATH="$LD_LIBRARY_PATH:/opt/shrinkray/lib"

WORKDIR /build

COPY Cargo.toml .
COPY Cargo.lock .
COPY build.rs .
COPY ./src ./src

RUN cargo build --config net.git-fetch-with-cli=true --release

FROM debian:bookworm-slim AS final

COPY --from=build /opt/shrinkray /opt/shrinkray

RUN apt-get update -qq && \
    apt-get -y install --no-install-recommends \
    libjemalloc-dev \
    libglib2.0 \
    libexpat1 \
    libde265-dev \
    libfftw3-dev \
    liblcms2-dev \
    libexif-dev \
    libwebp-dev \
    libjpeg62-turbo-dev \
    librsvg2-dev \
    libhwy-dev \
    liborc-dev \
    libx265-dev \
    libpng-dev \
    ca-certificates

COPY --from=build /build/target/release/shrinkray /opt/shrinkray/bin/server

ENV PKG_CONFIG_PATH="\$PKG_CONFIG_PATH:/opt/shrinkray/lib/pkgconfig"
ENV LD_LIBRARY_PATH="\$LD_LIBRARY_PATH:/opt/shrinkray/lib"
ENV LD_PRELOAD="/usr/lib/x86_64-linux-gnu/libjemalloc.so.2"
ENV PATH="$PATH:/opt/shrinkray/bin"

ENTRYPOINT ["/opt/shrinkray/bin/server"]
