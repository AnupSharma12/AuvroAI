pub const SUPABASE_URL: &str = env!("SUPABASE_URL");
pub const SUPABASE_PUBLISHABLE_KEY: &str = env!("SUPABASE_PUBLISHABLE_KEY");

pub const AUVRO_API_KEY: &str = env!("AUVRO_API_KEY");
pub const AUVRO_ENDPOINT: &str = env!("AUVRO_ENDPOINT");
pub const AUVRO_MODEL: &str = env!("AUVRO_MODEL");

pub const OPENROUTER_API_KEY: &str = env!("OPENROUTER_API_KEY");
pub const OPENROUTER_BASE_URL: &str = env!("OPENROUTER_BASE_URL");
pub const OPENROUTER_MODEL: &str = env!("OPENROUTER_MODEL");

pub fn supabase_url() -> String {
	normalized_supabase_url(SUPABASE_URL)
}

pub fn supabase_publishable_key() -> &'static str {
	SUPABASE_PUBLISHABLE_KEY
}

pub fn normalized_supabase_url(input: &str) -> String {
	let mut base = input
		.trim()
		.trim_matches('"')
		.trim_matches('\'')
		.trim_end_matches('/')
		.to_owned();

	loop {
		if base.ends_with("/auth/v1") {
			let next_len = base.len() - "/auth/v1".len();
			base.truncate(next_len);
			continue;
		}
		if base.ends_with("/rest/v1") {
			let next_len = base.len() - "/rest/v1".len();
			base.truncate(next_len);
			continue;
		}
		break;
	}

	base.trim_end_matches('/').to_owned()
}

pub fn supabase_base_url() -> String {
	supabase_url()
}

pub fn supabase_auth_url() -> String {
	format!("{}/auth/v1", supabase_base_url())
}

pub fn supabase_rest_url() -> String {
	format!("{}/rest/v1", supabase_base_url())
}
