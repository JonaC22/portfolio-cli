FROM rustlang/rust:nightly-buster-slim
COPY src src
COPY Cargo.toml Cargo.toml
COPY Cargo.lock Cargo.lock
COPY Settings.toml Settings.toml
RUN apt update && apt install libssl-dev libc-dev gcc
RUN cargo build
CMD [ "cargo", "run" ]
