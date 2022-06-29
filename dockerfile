FROM rust:alpine

ENV USER=cross-seed
ENV GROUP=cross-seed
ENV UID=1000
ENV GID=1000

# Add user

RUN addgroup -g $GID $GROUP && \
    adduser -D -u $UID --ingroup "$GROUP" "$USER"

RUN apk add --no-cache musl-dev openssl-dev

COPY --chown=UID:GID ./ /app
WORKDIR /app
RUN cargo install --path .

USER $USER
ENTRYPOINT [ "cross-seed" ]