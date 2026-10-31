#![allow(clippy::cmp_owned)]

use std::collections::HashMap;

// CRATES
use crate::body::Body;
use crate::server::ResponseExt;
use crate::subreddit::join_until_size_limit;
use crate::utils::{deflate_decompress, redirect, template, Preferences};
use askama::Template;
use cookie::Cookie;
use hyper::{Request, Response};
use time::{Duration, OffsetDateTime};
use tokio::time::timeout;
use url::form_urlencoded;

// STRUCTS
#[derive(Template)]
#[template(path = "settings.html")]
struct SettingsTemplate {
	prefs: Preferences,
	url: String,
	return_url: String,
}

// CONSTANTS

const PREFS: [&str; 23] = [
	"theme",
	"front_page",
	"layout",
	"wide",
	"comment_sort",
	"post_sort",
	"blur_spoiler",
	"show_nsfw",
	"blur_nsfw",
	"use_hls",
	"hide_hls_notification",
	"autoplay_videos",
	"hide_sidebar_and_summary",
	"fixed_navbar",
	"hide_awards",
	"hide_score",
	"disable_visit_reddit_confirmation",
	"video_quality",
	"remove_default_feeds",
	"geo_filter",
	"clean_urls",
	"posts_per_page",
	"max_comment_thread_depth",
];

pub static GEO_FILTERS: [(&str, &str); 250] = [
	("GLOBAL", "🌐 Global"),
	("AF", "🇦🇫 Afghanistan"),
	("AX", "🇦🇽 Åland Islands"),
	("AL", "🇦🇱 Albania"),
	("DZ", "🇩🇿 Algeria"),
	("AS", "🇦🇸 American Samoa"),
	("AD", "🇦🇩 Andorra"),
	("AO", "🇦🇴 Angola"),
	("AI", "🇦🇮 Anguilla"),
	("AQ", "🇦🇶 Antarctica"),
	("AG", "🇦🇬 Antigua and Barbuda"),
	("AR", "🇦🇷 Argentina"),
	("AM", "🇦🇲 Armenia"),
	("AW", "🇦🇼 Aruba"),
	("AU", "🇦🇺 Australia"),
	("AT", "🇦🇹 Austria"),
	("AZ", "🇦🇿 Azerbaijan"),
	("BS", "🇧🇸 Bahamas"),
	("BH", "🇧🇭 Bahrain"),
	("BD", "🇧🇩 Bangladesh"),
	("BB", "🇧🇧 Barbados"),
	("BY", "🇧🇾 Belarus"),
	("BE", "🇧🇪 Belgium"),
	("BZ", "🇧🇿 Belize"),
	("BJ", "🇧🇯 Benin"),
	("BM", "🇧🇲 Bermuda"),
	("BT", "🇧🇹 Bhutan"),
	("BO", "🇧🇴 Bolivia"),
	("BA", "🇧🇦 Bosnia and Herzegovina"),
	("BW", "🇧🇼 Botswana"),
	("BV", "🇧🇻 Bouvet Island"),
	("BR", "🇧🇷 Brazil"),
	("IO", "🇮🇴 British Indian Ocean Territory"),
	("VG", "🇻🇬 British Virgin Islands"),
	("BN", "🇧🇳 Brunei"),
	("BG", "🇧🇬 Bulgaria"),
	("BF", "🇧🇫 Burkina Faso"),
	("BI", "🇧🇮 Burundi"),
	("KH", "🇰🇭 Cambodia"),
	("CM", "🇨🇲 Cameroon"),
	("CA", "🇨🇦 Canada"),
	("CV", "🇨🇻 Cape Verde"),
	("BQ", "🇧🇶 Caribbean Netherlands"),
	("KY", "🇰🇾 Cayman Islands"),
	("CF", "🇨🇫 Central African Republic"),
	("TD", "🇹🇩 Chad"),
	("CL", "🇨🇱 Chile"),
	("CN", "🇨🇳 China"),
	("CX", "🇨🇽 Christmas Island"),
	("CC", "🇨🇨 Cocos (Keeling) Islands"),
	("CO", "🇨🇴 Colombia"),
	("KM", "🇰🇲 Comoros"),
	("CG", "🇨🇬 Congo"),
	("CK", "🇨🇰 Cook Islands"),
	("CR", "🇨🇷 Costa Rica"),
	("CI", "🇨🇮 Côte d'Ivoire"),
	("HR", "🇭🇷 Croatia"),
	("CU", "🇨🇺 Cuba"),
	("CW", "🇨🇼 Curaçao"),
	("CY", "🇨🇾 Cyprus"),
	("CZ", "🇨🇿 Czechia"),
	("DK", "🇩🇰 Denmark"),
	("DJ", "🇩🇯 Djibouti"),
	("DM", "🇩🇲 Dominica"),
	("DO", "🇩🇴 Dominican Republic"),
	("CD", "🇨🇩 DR Congo"),
	("EC", "🇪🇨 Ecuador"),
	("EG", "🇪🇬 Egypt"),
	("SV", "🇸🇻 El Salvador"),
	("GQ", "🇬🇶 Equatorial Guinea"),
	("ER", "🇪🇷 Eritrea"),
	("EE", "🇪🇪 Estonia"),
	("SZ", "🇸🇿 Eswatini"),
	("ET", "🇪🇹 Ethiopia"),
	("FK", "🇫🇰 Falkland Islands"),
	("FO", "🇫🇴 Faroe Islands"),
	("FJ", "🇫🇯 Fiji"),
	("FI", "🇫🇮 Finland"),
	("FR", "🇫🇷 France"),
	("GF", "🇬🇫 French Guiana"),
	("PF", "🇵🇫 French Polynesia"),
	("TF", "🇹🇫 French Southern Territories"),
	("GA", "🇬🇦 Gabon"),
	("GM", "🇬🇲 Gambia"),
	("GE", "🇬🇪 Georgia"),
	("DE", "🇩🇪 Germany"),
	("GH", "🇬🇭 Ghana"),
	("GI", "🇬🇮 Gibraltar"),
	("GR", "🇬🇷 Greece"),
	("GL", "🇬🇱 Greenland"),
	("GD", "🇬🇩 Grenada"),
	("GP", "🇬🇵 Guadeloupe"),
	("GU", "🇬🇺 Guam"),
	("GT", "🇬🇹 Guatemala"),
	("GG", "🇬🇬 Guernsey"),
	("GN", "🇬🇳 Guinea"),
	("GW", "🇬🇼 Guinea-Bissau"),
	("GY", "🇬🇾 Guyana"),
	("HT", "🇭🇹 Haiti"),
	("HM", "🇭🇲 Heard & McDonald Islands"),
	("HN", "🇭🇳 Honduras"),
	("HK", "🇭🇰 Hong Kong"),
	("HU", "🇭🇺 Hungary"),
	("IS", "🇮🇸 Iceland"),
	("IN", "🇮🇳 India"),
	("ID", "🇮🇩 Indonesia"),
	("IR", "🇮🇷 Iran"),
	("IQ", "🇮🇶 Iraq"),
	("IE", "🇮🇪 Ireland"),
	("IM", "🇮🇲 Isle of Man"),
	("IL", "🇮🇱 Israel"),
	("IT", "🇮🇹 Italy"),
	("JM", "🇯🇲 Jamaica"),
	("JP", "🇯🇵 Japan"),
	("JE", "🇯🇪 Jersey"),
	("JO", "🇯🇴 Jordan"),
	("KZ", "🇰🇿 Kazakhstan"),
	("KE", "🇰🇪 Kenya"),
	("KI", "🇰🇮 Kiribati"),
	("KW", "🇰🇼 Kuwait"),
	("KG", "🇰🇬 Kyrgyzstan"),
	("LA", "🇱🇦 Laos"),
	("LV", "🇱🇻 Latvia"),
	("LB", "🇱🇧 Lebanon"),
	("LS", "🇱🇸 Lesotho"),
	("LR", "🇱🇷 Liberia"),
	("LY", "🇱🇾 Libya"),
	("LI", "🇱🇮 Liechtenstein"),
	("LT", "🇱🇹 Lithuania"),
	("LU", "🇱🇺 Luxembourg"),
	("MO", "🇲🇴 Macao"),
	("MG", "🇲🇬 Madagascar"),
	("MW", "🇲🇼 Malawi"),
	("MY", "🇲🇾 Malaysia"),
	("MV", "🇲🇻 Maldives"),
	("ML", "🇲🇱 Mali"),
	("MT", "🇲🇹 Malta"),
	("MH", "🇲🇭 Marshall Islands"),
	("MQ", "🇲🇶 Martinique"),
	("MR", "🇲🇷 Mauritania"),
	("MU", "🇲🇺 Mauritius"),
	("YT", "🇾🇹 Mayotte"),
	("MX", "🇲🇽 Mexico"),
	("FM", "🇫🇲 Micronesia"),
	("MD", "🇲🇩 Moldova"),
	("MC", "🇲🇨 Monaco"),
	("MN", "🇲🇳 Mongolia"),
	("ME", "🇲🇪 Montenegro"),
	("MS", "🇲🇸 Montserrat"),
	("MA", "🇲🇦 Morocco"),
	("MZ", "🇲🇿 Mozambique"),
	("MM", "🇲🇲 Myanmar"),
	("NA", "🇳🇦 Namibia"),
	("NR", "🇳🇷 Nauru"),
	("NP", "🇳🇵 Nepal"),
	("NL", "🇳🇱 Netherlands"),
	("NC", "🇳🇨 New Caledonia"),
	("NZ", "🇳🇿 New Zealand"),
	("NI", "🇳🇮 Nicaragua"),
	("NE", "🇳🇪 Niger"),
	("NG", "🇳🇬 Nigeria"),
	("NU", "🇳🇺 Niue"),
	("NF", "🇳🇫 Norfolk Island"),
	("KP", "🇰🇵 North Korea"),
	("MK", "🇲🇰 North Macedonia"),
	("MP", "🇲🇵 Northern Mariana Islands"),
	("NO", "🇳🇴 Norway"),
	("OM", "🇴🇲 Oman"),
	("PK", "🇵🇰 Pakistan"),
	("PW", "🇵🇼 Palau"),
	("PS", "🇵🇸 Palestine"),
	("PA", "🇵🇦 Panama"),
	("PG", "🇵🇬 Papua New Guinea"),
	("PY", "🇵🇾 Paraguay"),
	("PE", "🇵🇪 Peru"),
	("PH", "🇵🇭 Philippines"),
	("PN", "🇵🇳 Pitcairn"),
	("PL", "🇵🇱 Poland"),
	("PT", "🇵🇹 Portugal"),
	("PR", "🇵🇷 Puerto Rico"),
	("QA", "🇶🇦 Qatar"),
	("RE", "🇷🇪 Réunion"),
	("RO", "🇷🇴 Romania"),
	("RU", "🇷🇺 Russia"),
	("RW", "🇷🇼 Rwanda"),
	("BL", "🇧🇱 Saint Barthélemy"),
	("SH", "🇸🇭 Saint Helena"),
	("KN", "🇰🇳 Saint Kitts and Nevis"),
	("LC", "🇱🇨 Saint Lucia"),
	("MF", "🇲🇫 Saint Martin"),
	("PM", "🇵🇲 Saint Pierre and Miquelon"),
	("VC", "🇻🇨 Saint Vincent and the Grenadines"),
	("WS", "🇼🇸 Samoa"),
	("SM", "🇸🇲 San Marino"),
	("ST", "🇸🇹 Sao Tome and Principe"),
	("SA", "🇸🇦 Saudi Arabia"),
	("SN", "🇸🇳 Senegal"),
	("RS", "🇷🇸 Serbia"),
	("SC", "🇸🇨 Seychelles"),
	("SL", "🇸🇱 Sierra Leone"),
	("SG", "🇸🇬 Singapore"),
	("SX", "🇸🇽 Sint Maarten"),
	("SK", "🇸🇰 Slovakia"),
	("SI", "🇸🇮 Slovenia"),
	("SB", "🇸🇧 Solomon Islands"),
	("SO", "🇸🇴 Somalia"),
	("ZA", "🇿🇦 South Africa"),
	("GS", "🇬🇸 South Georgia and the South Sandwich Islands"),
	("KR", "🇰🇷 South Korea"),
	("SS", "🇸🇸 South Sudan"),
	("ES", "🇪🇸 Spain"),
	("LK", "🇱🇰 Sri Lanka"),
	("SD", "🇸🇩 Sudan"),
	("SR", "🇸🇷 Suriname"),
	("SJ", "🇸🇯 Svalbard and Jan Mayen"),
	("SE", "🇸🇪 Sweden"),
	("CH", "🇨🇭 Switzerland"),
	("SY", "🇸🇾 Syria"),
	("TW", "🇹🇼 Taiwan"),
	("TJ", "🇹🇯 Tajikistan"),
	("TZ", "🇹🇿 Tanzania"),
	("TH", "🇹🇭 Thailand"),
	("TL", "🇹🇱 Timor-Leste"),
	("TG", "🇹🇬 Togo"),
	("TK", "🇹🇰 Tokelau"),
	("TO", "🇹🇴 Tonga"),
	("TT", "🇹🇹 Trinidad and Tobago"),
	("TN", "🇹🇳 Tunisia"),
	("TR", "🇹🇷 Türkiye"),
	("TM", "🇹🇲 Turkmenistan"),
	("TC", "🇹🇨 Turks and Caicos Islands"),
	("TV", "🇹🇻 Tuvalu"),
	("UM", "🇺🇲 U.S. Outlying Islands"),
	("VI", "🇻🇮 U.S. Virgin Islands"),
	("UG", "🇺🇬 Uganda"),
	("UA", "🇺🇦 Ukraine"),
	("AE", "🇦🇪 United Arab Emirates"),
	("GB", "🇬🇧 United Kingdom"),
	("US", "🇺🇸 United States"),
	("UY", "🇺🇾 Uruguay"),
	("UZ", "🇺🇿 Uzbekistan"),
	("VU", "🇻🇺 Vanuatu"),
	("VA", "🇻🇦 Vatican City"),
	("VE", "🇻🇪 Venezuela"),
	("VN", "🇻🇳 Vietnam"),
	("WF", "🇼🇫 Wallis and Futuna"),
	("EH", "🇪🇭 Western Sahara"),
	("YE", "🇾🇪 Yemen"),
	("ZM", "🇿🇲 Zambia"),
	("ZW", "🇿🇼 Zimbabwe"),
];

// FUNCTIONS

/// Retrieve cookies from request "Cookie" header
pub async fn get(req: Request<Body>) -> Result<Response<Body>, String> {
	let url = req.uri().to_string();
	let form = url::form_urlencoded::parse(req.uri().query().unwrap_or_default().as_bytes()).collect::<HashMap<_, _>>();
	Ok(template(&SettingsTemplate {
		prefs: Preferences::new(&req),
		url,
		return_url: match form.get("return_to") {
			Some(url) => url.clone().to_string(),
			None => String::new(),
		},
	}))
}

/// Set cookies using response "Set-Cookie" header
pub async fn set(req: Request<Body>) -> Result<Response<Body>, String> {
	// Split the body into parts
	let (parts, body) = req.into_parts();

	// Grab existing cookies
	let _cookies: Vec<Cookie<'_>> = parts
		.headers
		.get_all("Cookie")
		.iter()
		.filter_map(|header| Cookie::parse(header.to_str().unwrap_or_default()).ok())
		.collect();

	// Aggregate the body...
	let body_bytes = body.collect_bytes().await.map_err(|e| e.to_string())?;

	let form = url::form_urlencoded::parse(&body_bytes).collect::<HashMap<_, _>>();

	let mut response = match form.get("return_to") {
		Some(value) => {
			if value.is_empty() {
				redirect("/settings")
			} else {
				redirect(value)
			}
		}
		None => redirect("/settings"),
	};

	for &name in &PREFS {
		match form.get(name) {
			Some(value) => response.insert_cookie(
				Cookie::build((name.to_owned(), value.clone()))
					.path("/")
					.http_only(true)
					.expires(OffsetDateTime::now_utc() + Duration::weeks(52))
					.into(),
			),
			None => response.remove_cookie(name.to_string()),
		};
	}

	Ok(response)
}

fn set_cookies_method(req: Request<Body>, remove_cookies: bool) -> Response<Body> {
	// Split the body into parts
	let (parts, _) = req.into_parts();

	// Grab existing cookies
	let _cookies: Vec<Cookie<'_>> = parts
		.headers
		.get_all("Cookie")
		.iter()
		.filter_map(|header| Cookie::parse(header.to_str().unwrap_or_default()).ok())
		.collect();

	let query = parts.uri.query().unwrap_or_default().as_bytes();

	let form = url::form_urlencoded::parse(query).collect::<HashMap<_, _>>();

	let path = match form.get("redirect") {
		Some(value) => {
			let value = value.replace("%26", "&").replace("%23", "#");
			if value.starts_with('/') {
				value
			} else {
				format!("/{value}")
			}
		}
		None => "/".to_string(),
	};

	let mut response = redirect(&path);

	for name in PREFS {
		match form.get(name) {
			Some(value) => response.insert_cookie(
				Cookie::build((name.to_owned(), value.clone()))
					.path("/")
					.http_only(true)
					.expires(OffsetDateTime::now_utc() + Duration::weeks(52))
					.into(),
			),
			None => {
				if remove_cookies {
					response.remove_cookie(name.to_string());
				}
			}
		};
	}

	// Get subscriptions/filters to restore from query string
	let subscriptions = form.get("subscriptions");
	let filters = form.get("filters");

	// We can't search through the cookies directly like in subreddit.rs, so instead we have to make a string out of the request's headers to search through
	let cookies_string = parts
		.headers
		.get("cookie")
		.map(|hv| hv.to_str().unwrap_or("").to_string()) // Return String
		.unwrap_or_else(String::new); // Return an empty string if None

	// If there are subscriptions to restore set them and delete any old subscriptions cookies, otherwise delete them all
	if let Some(subscriptions) = subscriptions {
		let sub_list: Vec<String> = subscriptions.split('+').map(str::to_string).collect();

		// Start at 0 to keep track of what number we need to start deleting old subscription cookies from
		let mut subscriptions_number_to_delete_from = 0;

		// Starting at 0 so we handle the subscription cookie without a number first
		for (subscriptions_number, list) in join_until_size_limit(&sub_list).into_iter().enumerate() {
			let subscriptions_cookie = if subscriptions_number == 0 {
				"subscriptions".to_string()
			} else {
				format!("subscriptions{subscriptions_number}")
			};

			response.insert_cookie(
				Cookie::build((subscriptions_cookie, list))
					.path("/")
					.http_only(true)
					.expires(OffsetDateTime::now_utc() + Duration::weeks(52))
					.into(),
			);

			subscriptions_number_to_delete_from += 1;
		}

		// While subscriptionsNUMBER= is in the string of cookies add a response removing that cookie
		while cookies_string.contains(&format!("subscriptions{subscriptions_number_to_delete_from}=")) {
			// Remove that subscriptions cookie
			response.remove_cookie(format!("subscriptions{subscriptions_number_to_delete_from}"));

			// Increment subscriptions cookie number
			subscriptions_number_to_delete_from += 1;
		}
	} else if remove_cookies {
		// Remove unnumbered subscriptions cookie
		response.remove_cookie("subscriptions".to_string());

		// Starts at one to deal with the first numbered subscription cookie and onwards
		let mut subscriptions_number_to_delete_from = 1;

		// While subscriptionsNUMBER= is in the string of cookies add a response removing that cookie
		while cookies_string.contains(&format!("subscriptions{subscriptions_number_to_delete_from}=")) {
			// Remove that subscriptions cookie
			response.remove_cookie(format!("subscriptions{subscriptions_number_to_delete_from}"));

			// Increment subscriptions cookie number
			subscriptions_number_to_delete_from += 1;
		}
	}

	// If there are filters to restore set them and delete any old filters cookies, otherwise delete them all
	if let Some(filters) = filters {
		let filters_list: Vec<String> = filters.split('+').map(str::to_string).collect();

		// Start at 0 to keep track of what number we need to start deleting old subscription cookies from
		let mut filters_number_to_delete_from = 0;

		// Starting at 0 so we handle the subscription cookie without a number first
		for (filters_number, list) in join_until_size_limit(&filters_list).into_iter().enumerate() {
			let filters_cookie = if filters_number == 0 {
				"filters".to_string()
			} else {
				format!("filters{filters_number}")
			};

			response.insert_cookie(
				Cookie::build((filters_cookie, list))
					.path("/")
					.http_only(true)
					.expires(OffsetDateTime::now_utc() + Duration::weeks(52))
					.into(),
			);

			filters_number_to_delete_from += 1;
		}

		// While filtersNUMBER= is in the string of cookies add a response removing that cookie
		while cookies_string.contains(&format!("filters{filters_number_to_delete_from}=")) {
			// Remove that filters cookie
			response.remove_cookie(format!("filters{filters_number_to_delete_from}"));

			// Increment filters cookie number
			filters_number_to_delete_from += 1;
		}
	} else if remove_cookies {
		// Remove unnumbered filters cookie
		response.remove_cookie("filters".to_string());

		// Starts at one to deal with the first numbered subscription cookie and onwards
		let mut filters_number_to_delete_from = 1;

		// While filtersNUMBER= is in the string of cookies add a response removing that cookie
		while cookies_string.contains(&format!("filters{filters_number_to_delete_from}=")) {
			// Remove that sfilters cookie
			response.remove_cookie(format!("filters{filters_number_to_delete_from}"));

			// Increment filters cookie number
			filters_number_to_delete_from += 1;
		}
	}

	response
}

/// Set cookies using response "Set-Cookie" header
pub async fn restore(req: Request<Body>) -> Result<Response<Body>, String> {
	Ok(set_cookies_method(req, true))
}

pub async fn update(req: Request<Body>) -> Result<Response<Body>, String> {
	Ok(set_cookies_method(req, false))
}

pub async fn encoded_restore(req: Request<Body>) -> Result<Response<Body>, String> {
	let body = req.into_body().collect_bytes().await.map_err(|e| format!("Failed to get bytes from request body: {e}"))?;

	if body.len() > 1024 * 1024 {
		return Err("Request body too large".to_string());
	}

	let encoded_prefs = form_urlencoded::parse(&body)
		.find(|(key, _)| key == "encoded_prefs")
		.map(|(_, value)| value)
		.ok_or_else(|| "encoded_prefs parameter not found in request body".to_string())?;

	let bytes = base2048::decode(&encoded_prefs).ok_or_else(|| "Failed to decode base2048 encoded preferences".to_string())?;

	let out = timeout(std::time::Duration::from_secs(1), async { deflate_decompress(bytes) })
		.await
		.map_err(|e| format!("Failed to decompress bytes: {e}"))??;

	let mut prefs: Preferences = timeout(std::time::Duration::from_secs(1), async { Preferences::from_bincode(&out) })
		.await
		.map_err(|e| format!("Failed to deserialize preferences: {e}"))?
		.map_err(|e| format!("Failed to deserialize bytes into Preferences struct: {e}"))?;

	prefs.available_themes = vec![];

	let url = format!("/settings/restore/?{}", prefs.to_urlencoded()?);

	Ok(redirect(&url))
}
