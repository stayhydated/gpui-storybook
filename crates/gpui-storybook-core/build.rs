pub fn main() {
    if let Err(e) = es_fluent_build::FluentBuilder::new()
        .mode(es_fluent_build::FluentParseMode::Aggressive)
        .build()
    {
        log::error!("Error building FTL files: {}", e);
    }
}
