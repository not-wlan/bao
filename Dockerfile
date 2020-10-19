FROM rust:buster

# Add the LLVM public key to verify the repos
RUN bash -c "wget -O - https://apt.llvm.org/llvm-snapshot.gpg.key|apt-key add -"
# Add the LLVM repos
RUN printf "deb http://apt.llvm.org/buster/ llvm-toolchain-buster-10 main\ndeb-src http://apt.llvm.org/buster/ llvm-toolchain-buster-10 main" > /etc/apt/sources.list.d/backports.list
# Install dependencies
RUN apt-get update && apt-get install -y curl git make build-essential libllvm10 llvm-10 llvm-10-dev llvm-10-runtime \
    cmake clang-10 clang-tools-10 libclang-common-10-dev libclang-10-dev libclang1-10 clang-format-10

WORKDIR /app
COPY ./Cargo.toml .
COPY ./Cargo.lock .
COPY ./src/ /app/src/

RUN ls -alh

RUN cargo update -p pdb_wrapper && cargo build --release

CMD ["cargo run -- --help"]