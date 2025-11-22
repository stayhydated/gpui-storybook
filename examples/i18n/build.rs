pub fn main() {
    if let Err(e) = es_fluent_build::FluentBuilder::new()
        .mode(es_fluent_build::FluentParseMode::Conservative)
        .build()
    {
        log::error!("Error building FTL files: {}", e);
    }
}
