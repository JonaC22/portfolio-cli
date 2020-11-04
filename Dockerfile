FROM rustlang/rust:nightly-buster-slim
RUN apt update && apt install libssl-dev libc-dev gcc
COPY src src
COPY Cargo.toml Cargo.toml
COPY Cargo.lock Cargo.lock
RUN cargo build
COPY Settings.toml Settings.toml
CMD [ "cargo", "run" ]
