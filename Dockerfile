FROM rustlang/rust:nightly-buster
COPY src src
COPY Cargo.toml Cargo.toml
COPY Cargo.lock Cargo.lock
RUN cargo build
COPY Settings.toml Settings.toml
CMD [ "cargo", "run" ]
