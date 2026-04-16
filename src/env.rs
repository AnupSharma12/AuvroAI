pub const SUPABASE_URL: &str = match option_env!("SUPABASE_URL") {
	Some(value) => value,
	None => "",
};
pub const SUPABASE_PUBLISHABLE_KEY: &str = match option_env!("SUPABASE_PUBLISHABLE_KEY") {
	Some(value) => value,
	None => "",
};

pub const AUVRO_API_KEY: &str = match option_env!("AUVRO_API_KEY") {
	Some(value) => value,
	None => "",
};
pub const AUVRO_ENDPOINT: &str = match option_env!("AUVRO_ENDPOINT") {
	Some(value) => value,
	None => "",
};
pub const AUVRO_MODEL: &str = match option_env!("AUVRO_MODEL") {
	Some(value) => value,
	None => "",
};

pub const OPENROUTER_API_KEY: &str = match option_env!("OPENROUTER_API_KEY") {
	Some(value) => value,
	None => "",
};
pub const OPENROUTER_BASE_URL: &str = match option_env!("OPENROUTER_BASE_URL") {
	Some(value) => value,
	None => "",
};
pub const OPENROUTER_MODEL: &str = match option_env!("OPENROUTER_MODEL") {
	Some(value) => value,
	None => "",
};

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
