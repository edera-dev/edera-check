FROM ghcr.io/edera-dev/cross-base-linux-musl:latest@sha256:87ba899ea380bd85c22f194ab2f4f2cf791fc832d27ee20bb00d07ce23771975 AS build

ENV TARGET_LIBC=musl TARGET_VENDOR=unknown DISABLE_CROSS_RS=1

WORKDIR /usr/src/app
COPY . .
RUN ./hack/build/cargo.sh build --release --bin preflight
RUN mv ./target/$(./hack/build/target.sh)/release/preflight /usr/sbin

FROM cgr.dev/chainguard/wolfi-base:latest
ENTRYPOINT ["/usr/sbin/preflight"]
COPY --from=build /usr/sbin/preflight /usr/sbin/preflight
COPY --from=build /usr/src/app/scripts /scripts
