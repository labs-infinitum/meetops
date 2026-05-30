fn main() -> Result<(), Box<dyn std::error::Error>> {
    unsafe {
        std::env::set_var("PROTOC", protoc_bin_vendored::protoc_bin_path()?);
    }
    tonic_build::configure()
        .build_server(true)
        .build_client(false)
        .compile_protos(
            &[
                "../../packages/proto/notetaker/v1/auth.proto",
                "../../packages/proto/notetaker/v1/calendar.proto",
                "../../packages/proto/notetaker/v1/meeting.proto",
                "../../packages/proto/notetaker/v1/recording.proto",
            ],
            &["../../packages/proto"],
        )?;
    Ok(())
}
