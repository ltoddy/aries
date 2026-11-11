use std::io;

fn main() -> io::Result<()> {
    let mut config = tonic_prost_build::Config::new();
    config.protoc_executable(protoc_bin_vendored::protoc_bin_path().map_err(io::Error::other)?);
    tonic_prost_build::configure().compile_with_config(config, &["proto/service.proto"], &["proto"])
}
