FROM rust:1.64.0-alpine as builder

ENV RUSTFLAGS="-C target-feature=-crt-static"
RUN apk --no-cache add openssl musl-dev openssl-dev build-base perl

WORKDIR /opt
# cache dependencies separately
RUN USER=root cargo new --bin sataddress
WORKDIR /opt/sataddress
# COPY ./Cargo.lock ./Cargo.lock
COPY ./Cargo.toml ./Cargo.toml
RUN sed -i '/default-run/d' ./Cargo.toml

RUN cargo build --release
RUN rm ./src/*.rs
RUN rm ./target/release/deps/sataddress*

# actual sources build
ADD ./src ./src
ADD ./assets ./assets
ADD ./templates ./templates

RUN cargo build --release
RUN ls -al /opt/sataddress/target/release/

# FROM scratch # TODO
FROM alpine:latest

ARG APP=/opt/sataddress
ENV TZ=Etc/UTC \
    APP_USER=appuser

RUN addgroup -S $APP_USER \
    && adduser -S -g $APP_USER $APP_USER

RUN apk update \
    && apk add --no-cache ca-certificates tzdata libgcc \
    && rm -rf /var/cache/apk/*

WORKDIR ${APP}
COPY --from=builder /opt/sataddress/target/release/server .
COPY --from=builder /opt/sataddress/target/release/cli .

COPY ./assets ./assets
COPY ./templates ./templates

RUN chown -R $APP_USER:$APP_USER ${APP}
USER $APP_USER

EXPOSE 3030
CMD ["./server"]
