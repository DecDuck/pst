FROM rust:1.89.0-trixie AS build

WORKDIR /build
COPY . .
RUN cargo build --release

FROM frolvlad/alpine-glibc:latest AS run

WORKDIR /app
COPY --from=build /build/target/release/pst .

CMD ["/app/pst"]