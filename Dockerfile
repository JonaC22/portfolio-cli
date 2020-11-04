FROM rustlang/rust:nightly-buster-slim
RUN apt-get update && apt-get install libssl-dev libc-dev gcc -y
COPY src src
COPY Cargo.toml Cargo.toml
COPY Cargo.lock Cargo.lock
RUN cargo build
COPY Settings.toml Settings.toml
CMD [ "cargo", "run" ]
