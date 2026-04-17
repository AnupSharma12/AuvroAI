pub const SUPABASE_URL: &str = env!("SUPABASE_URL");
pub const SUPABASE_PUBLISHABLE_KEY: &str = env!("SUPABASE_PUBLISHABLE_KEY");
pub const AUVRO_API_KEY: &str = env!("AUVRO_API_KEY");
pub const AUVRO_ENDPOINT: &str = env!("AUVRO_ENDPOINT");
pub const AUVRO_MODEL: &str = env!("AUVRO_MODEL");
pub const OPENROUTER_API_KEY: &str = env!("OPENROUTER_API_KEY");
pub const OPENROUTER_BASE_URL: &str = env!("OPENROUTER_BASE_URL");
pub const OPENROUTER_MODEL: &str = env!("OPENROUTER_MODEL");

pub fn supabase_rest_url() -> String {
	format!("{}/rest/v1", SUPABASE_URL.trim_end_matches('/'))
}

pub fn supabase_auth_url() -> String {
	format!("{}/auth/v1", SUPABASE_URL.trim_end_matches('/'))
}

pub fn supabase_url() -> &'static str {
	SUPABASE_URL
}
