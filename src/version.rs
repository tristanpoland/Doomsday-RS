pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn version() -> String {
    let git_hash = option_env!("GIT_HASH").unwrap_or("unknown");
    let dirty = if option_env!("GIT_DIRTY").unwrap_or("false") == "true" {
        "+"
    } else {
        ""
    };
    
    format!("{}-{}{}", VERSION, git_hash, dirty)
}