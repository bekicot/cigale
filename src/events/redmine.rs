// not using the redmine Rest api because
// 1. unless the redmine admin greenlights it, a user may be unable to get an apikey
// 2. the redmine rest api doesn't offer an activity API https://www.redmine.org/issues/14872
//    without such an API, this would be very painful and very slow
use super::events::{ConfigType, Event, EventBody, EventProvider, Result, WordWrapMode};
use crate::config::Config;
use chrono::prelude::*;
use core::time::Duration;
use std::collections::HashMap;

#[derive(serde_derive::Deserialize, serde_derive::Serialize, Clone, Debug)]
pub struct RedmineConfig {
    pub server_url: String,
    pub username: String,
    pub password: String,
}

pub struct Redmine;
const SERVER_URL_KEY: &str = "Server URL";
const USERNAME_KEY: &str = "Username";
const PASSWORD_KEY: &str = "Password";

enum ActivityData {
    Done(Vec<Event>),
    ReachedEndOfPage(Option<String>), // link to the previous page or None if no previous
}

#[derive(Debug)]
struct LocaleInfo {
    date_format: &'static str,
    today_translation: &'static str,
}

impl LocaleInfo {
    fn new(date_format: &'static str, today_translation: &'static str) -> LocaleInfo {
        LocaleInfo {
            date_format,
            today_translation,
        }
    }
}

impl Redmine {
    fn parse_date(locale_info: &LocaleInfo, date_str: &str) -> Result<Date<Local>> {
        log::debug!(
            "parse_date: parsing {}, locale: {:?}",
            date_str,
            locale_info
        );
        let iso_date_format_regex = regex::Regex::new(r"^\d{4}-\d{2}-\d{2}$").unwrap();
        if date_str.to_lowercase() == locale_info.today_translation {
            Ok(Local::today())
        } else {
            let naive = if iso_date_format_regex.is_match(date_str) {
                // for some reason on my redmine server at work, whatever
                // locale I configure, I end up with the ISO date format...
                NaiveDate::parse_from_str(date_str, "%Y-%m-%d")
            } else {
                log::debug!("Using locale-specific string {}", locale_info.date_format);
                NaiveDate::parse_from_str(date_str, locale_info.date_format)
            }?;
            let local = Local
                .from_local_date(&naive)
                .single()
                .ok_or(format!("Can't convert {} to local time", naive))?;
            Ok(local)
        }
    }

    fn parse_time(time_str: &str) -> Result<NaiveTime> {
        log::debug!("parse_time: parsing {}", time_str);
        Ok(if time_str.contains(' ') {
            NaiveTime::parse_from_str(&time_str, "%I:%M %p")?
        } else {
            NaiveTime::parse_from_str(&time_str, "%H:%M")?
        })
    }

    fn redmine_locales() -> HashMap<&'static str, LocaleInfo> {
        // the contents of this function are generated by the helpers/redmine_locales helper app
        vec![
            ("lv", LocaleInfo::new("%d.%m.%Y", "šodien")),
            ("th", LocaleInfo::new("%Y-%m-%d", "วันนี้")),
            ("zh", LocaleInfo::new("%Y-%m-%d", "今天")),
            ("da", LocaleInfo::new("%d.%m.%Y", "i dag")),
            ("pt", LocaleInfo::new("%d/%m/%Y", "hoje")),
            ("ja", LocaleInfo::new("%Y/%m/%d", "今日")),
            ("pl", LocaleInfo::new("%Y-%m-%d", "dzisiaj")),
            ("lt", LocaleInfo::new("%m/%d/%Y", "šiandien")),
            ("fa", LocaleInfo::new("%Y/%m/%d", "امروز")),
            ("gl", LocaleInfo::new("%e/%m/%Y", "hoxe")),
            ("uk", LocaleInfo::new("%Y-%m-%d", "сьогодні")),
            ("vi", LocaleInfo::new("%d-%m-%Y", "hôm nay")),
            ("mn", LocaleInfo::new("%Y/%m/%d", "өнөөдөр")),
            ("cs", LocaleInfo::new("%Y-%m-%d", "dnes")),
            ("en-GB", LocaleInfo::new("%d/%m/%Y", "today")),
            ("fr", LocaleInfo::new("%d/%m/%Y", "aujourd'hui")),
            ("sr", LocaleInfo::new("%d.%m.%Y.", "данас")),
            ("fi", LocaleInfo::new("%e. %Bta %Y", "tänään")),
            ("no", LocaleInfo::new("%d.%m.%Y", "idag")),
            ("mk", LocaleInfo::new("%d/%m/%Y", "денес")),
            ("hu", LocaleInfo::new("%Y.%m.%d.", "ma")),
            ("ro", LocaleInfo::new("%d-%m-%Y", "astăzi")),
            ("it", LocaleInfo::new("%d-%m-%Y", "oggi")),
            ("he", LocaleInfo::new("%d/%m/%Y", "היום")),
            ("es", LocaleInfo::new("%Y-%m-%d", "hoy")),
            ("en", LocaleInfo::new("%m/%d/%Y", "today")),
            ("sq", LocaleInfo::new("%m/%d/%Y", "sot")),
            ("eu", LocaleInfo::new("%Y/%m/%d", "gaur")),
            ("id", LocaleInfo::new("%d-%m-%Y", "hari ini")),
            ("de", LocaleInfo::new("%d.%m.%Y", "heute")),
            ("bg", LocaleInfo::new("%d-%m-%Y", "днес")),
            ("sv", LocaleInfo::new("%Y-%m-%d", "idag")),
            ("sk", LocaleInfo::new("%Y-%m-%d", "dnes")),
            ("ko", LocaleInfo::new("%Y/%m/%d", "오늘")),
            ("et", LocaleInfo::new("%d.%m.%Y", "täna")),
            ("hr", LocaleInfo::new("%m/%d/%Y", "danas")),
            ("el", LocaleInfo::new("%m/%d/%Y", "σήμερα")),
            ("zh-TW", LocaleInfo::new("%Y-%m-%d", "今天")),
            ("sr-YU", LocaleInfo::new("%d.%m.%Y.", "danas")),
            ("bs", LocaleInfo::new("%d.%m.%Y", "danas")),
            ("tr", LocaleInfo::new("%d.%m.%Y", "bugün")),
            ("ru", LocaleInfo::new("%d.%m.%Y", "сегодня")),
            ("es-PA", LocaleInfo::new("%Y-%m-%d", "hoy")),
            ("ar", LocaleInfo::new("%m/%d/%Y", "اليوم")),
            ("sl", LocaleInfo::new("%d.%m.%Y", "danes")),
            ("az", LocaleInfo::new("%d.%m.%Y", "bu gün")),
            ("ca", LocaleInfo::new("%d-%m-%Y", "avui")),
            ("pt-BR", LocaleInfo::new("%d/%m/%Y", "hoje")),
            ("nl", LocaleInfo::new("%d-%m-%Y", "vandaag")),
        ]
        .into_iter()
        .collect()
    }

    fn parse_events<'a>(
        redmine_config: &RedmineConfig,
        contents_elt: &scraper::element_ref::ElementRef<'a>,
    ) -> Result<Vec<Event>> {
        let description_sel = scraper::Selector::parse("span.description").unwrap();
        let link_sel = scraper::Selector::parse("dt.icon a").unwrap();
        let time_sel = scraper::Selector::parse("span.time").unwrap();
        let mut it_descriptions = contents_elt.select(&description_sel);
        let mut it_links = contents_elt.select(&link_sel);
        let mut it_times = contents_elt.select(&time_sel);
        let mut day_has_data = true;
        let mut result = vec![];
        while day_has_data {
            let next_time = it_times.next();
            day_has_data = next_time.is_some();
            if day_has_data {
                let time_elt = &next_time.unwrap();
                let time_str = time_elt.inner_html();
                let time = Self::parse_time(&time_str)?;
                let description_elt = &it_descriptions
                    .next()
                    .ok_or_else(|| "Redmine event: no description?")?;
                let link_elt = &it_links.next().ok_or_else(|| "Redmine event: no link?")?;
                result.push(Event::new(
                    "Redmine",
                    crate::icons::FONTAWESOME_TASKS_SVG,
                    time,
                    link_elt.inner_html(),
                    link_elt.inner_html(),
                    EventBody::Markup(
                        format!(
                            "<a href=\"{}{}\">Open in the browser</a>\n{}",
                            redmine_config.server_url,
                            link_elt.value().attr("href").unwrap_or(""),
                            glib::markup_escape_text(&description_elt.inner_html()),
                        ),
                        WordWrapMode::WordWrap,
                    ),
                    None,
                ));
            }
        }
        Ok(result)
    }

    fn init_client(redmine_config: &RedmineConfig) -> Result<(reqwest::blocking::Client, String)> {
        let client = reqwest::blocking::ClientBuilder::new()
            .cookie_store(true)
            .timeout(Duration::from_secs(30))
            .connect_timeout(Duration::from_secs(30))
            .connection_verbose(true)
            .build()?;

        let html = client
            .get(&redmine_config.server_url)
            .send()?
            .error_for_status()?
            .text()?;
        log::debug!("Got back html {}", html);
        let doc = scraper::Html::parse_document(&html);
        let sel = scraper::Selector::parse("input[name=authenticity_token]").unwrap();
        let auth_token_node = doc.select(&sel).next().unwrap();
        let auth_token = auth_token_node.value().attr("value").unwrap();

        let html = client
            .post(&format!("{}/login", redmine_config.server_url))
            .form(&[
                ("username", &redmine_config.username),
                ("password", &redmine_config.password),
                ("login", &"Login".to_string()),
                ("utf8", &"✓".to_string()),
                ("back_url", &redmine_config.server_url),
                ("authenticity_token", &auth_token.to_string()),
            ])
            .send()?
            .error_for_status()?
            .text()?;
        let doc = scraper::Html::parse_document(&html);
        let user_sel = scraper::Selector::parse("a.user.active").unwrap();
        let user_id = doc
            .select(&user_sel)
            .next()
            .ok_or_else(|| "Failed getting the user id#1")?
            .value()
            .attr("href")
            .ok_or_else(|| "Failed getting the user id#2")?
            .replace("/users/", "");
        Ok((client, user_id))
    }

    fn fetch_activity_html(
        config_name: &str,
        redmine_config: &RedmineConfig,
    ) -> Result<(reqwest::blocking::Client, String)> {
        let (client, user_id) = Self::init_client(redmine_config)?;

        let html = client
            .get(&format!(
                "{}/activity?user_id={}",
                redmine_config.server_url, user_id
            ))
            .send()?
            .error_for_status()?
            .text()?;
        Config::write_to_cache(&Redmine, config_name, &html)?;
        Ok((client, html))
    }

    fn parse_html(
        redmine_config: &RedmineConfig,
        redmine_locales: &HashMap<&'static str, LocaleInfo>,
        day: Date<Local>,
        activity_html: &str,
    ) -> Result<ActivityData> {
        let doc = scraper::Html::parse_document(&activity_html);
        let locale_str = doc
            .root_element()
            .value()
            .attr("lang")
            .ok_or("Can't find the language in the HTML")?;
        log::debug!("Locale str: {}", locale_str);
        let locale = redmine_locales
            .get(locale_str)
            .ok_or(format!("Unknown locale {}", locale_str))?;
        let day_sel = scraper::Selector::parse("div#content div#activity h3").unwrap();
        let day_contents_sel =
            scraper::Selector::parse("div#content div#activity h3 + dl").unwrap();
        let mut it_day = doc.select(&day_sel);
        let mut it_contents = doc.select(&day_contents_sel);
        loop {
            let next_day = it_day.next();
            let contents = it_contents.next();
            match (next_day, contents) {
                (Some(day_elt), Some(contents_elt)) => {
                    let cur_date = Self::parse_date(&locale, &day_elt.inner_html())?;
                    if cur_date < day {
                        // passed the day, won't be any events this time.
                        return Ok(ActivityData::Done(vec![]));
                    }
                    if cur_date == day {
                        return Self::parse_events(redmine_config, &contents_elt)
                            .map(ActivityData::Done);
                    }
                }
                _ => {
                    break;
                }
            }
        }
        // no matches in this page, search for the 'previous' paging link
        let previous_sel = scraper::Selector::parse("li.previous.page a").unwrap();
        let previous_url = doc
            .select(&previous_sel)
            .next()
            .and_then(|p| p.value().attr("href"));
        Ok(ActivityData::ReachedEndOfPage(
            previous_url.map(|s| redmine_config.server_url.clone() + s),
        ))
    }

    fn get_events_with_paging(
        day: Date<Local>,
        activity_html: String,
        redmine_config: &RedmineConfig,
        redmine_locales: &HashMap<&'static str, LocaleInfo>,
        client_opt: Option<reqwest::blocking::Client>,
    ) -> Result<Vec<Event>> {
        match Self::parse_html(redmine_config, redmine_locales, day, &activity_html) {
            Ok(ActivityData::Done(events)) => Ok(events),
            Err(e) => Err(e),
            Ok(ActivityData::ReachedEndOfPage(None)) => Ok(vec![]),
            Ok(ActivityData::ReachedEndOfPage(Some(new_url))) => {
                // recursively check for the previous page
                let client = match client_opt {
                    Some(c) => c,
                    None => Self::init_client(redmine_config)?.0,
                };
                println!("Fetching {}", new_url);
                let html = client.get(&new_url).send()?.error_for_status()?.text()?;
                Self::get_events_with_paging(
                    day,
                    html,
                    redmine_config,
                    redmine_locales,
                    Some(client),
                )
            }
        }
    }
}

impl EventProvider for Redmine {
    fn name(&self) -> &'static str {
        "Redmine"
    }

    fn default_icon(&self) -> &'static [u8] {
        crate::icons::FONTAWESOME_TASKS_SVG
    }

    fn get_config_names<'a>(&self, config: &'a Config) -> Vec<&'a String> {
        config.redmine.keys().collect()
    }

    fn get_config_fields(&self) -> Vec<(&'static str, ConfigType)> {
        vec![
            (SERVER_URL_KEY, ConfigType::Text("")),
            (USERNAME_KEY, ConfigType::Text("")),
            (PASSWORD_KEY, ConfigType::Password),
        ]
    }

    fn field_values(
        &self,
        _cur_values: &HashMap<&'static str, String>,
        _field_name: &'static str,
    ) -> Result<Vec<String>> {
        Ok(Vec::new())
    }

    fn get_config_values(
        &self,
        config: &Config,
        config_name: &str,
    ) -> HashMap<&'static str, String> {
        vec![
            (
                SERVER_URL_KEY,
                config.redmine[config_name].server_url.to_string(),
            ),
            (
                USERNAME_KEY,
                config.redmine[config_name].username.to_string(),
            ),
            (
                PASSWORD_KEY,
                config.redmine[config_name].password.to_string(),
            ),
        ]
        .into_iter()
        .collect()
    }

    fn add_config_values(
        &self,
        config: &mut Config,
        config_name: String,
        mut config_values: HashMap<&'static str, String>,
    ) {
        config.redmine.insert(
            config_name,
            RedmineConfig {
                server_url: config_values.remove(SERVER_URL_KEY).unwrap(),
                username: config_values.remove(USERNAME_KEY).unwrap(),
                password: config_values.remove(PASSWORD_KEY).unwrap(),
            },
        );
    }

    fn remove_config(&self, config: &mut Config, config_name: String) {
        config.redmine.remove(&config_name);
    }

    fn get_events(
        &self,
        config: &Config,
        config_name: &str,
        day: Date<Local>,
    ) -> Result<Vec<Event>> {
        log::debug!("redmine::get_events");
        let redmine_config = &config.redmine[config_name];
        let redmine_locales = Self::redmine_locales();
        let day_start = day.and_hms(0, 0, 0);
        let next_day_start = day_start + chrono::Duration::days(1);
        let (client, activity_html) =
            match Config::get_cached_contents(&Redmine, config_name, &next_day_start)? {
                Some(t) => Ok((None, t)),
                None => Self::fetch_activity_html(config_name, &redmine_config)
                    .map(|(a, b)| (Some(a), b)),
            }?;
        Self::get_events_with_paging(day, activity_html, redmine_config, &redmine_locales, client)
    }
}

#[test]
fn it_parses_us_dates_correctly() {
    let en_gb = &Redmine::redmine_locales()["en"];
    assert_eq!(
        NaiveDate::from_ymd(2020, 3, 23),
        Redmine::parse_date(&en_gb, "03/23/2020")
            .unwrap()
            .naive_local()
    );
}

#[test]
fn it_parses_slovenian_dates_correctly() {
    let sl = &Redmine::redmine_locales()["sl"];
    assert_eq!(
        NaiveDate::from_ymd(2020, 3, 23),
        Redmine::parse_date(&sl, "23.03.2020")
            .unwrap()
            .naive_local()
    );
}

#[test]
fn it_parses_iso_dates_correctly() {
    let en_gb = &Redmine::redmine_locales()["en-GB"];
    assert_eq!(
        NaiveDate::from_ymd(2020, 3, 23),
        Redmine::parse_date(&en_gb, "2020-03-23")
            .unwrap()
            .naive_local()
    );
}

#[test]
fn it_parses_us_times_correctly() {
    assert_eq!(
        NaiveTime::from_hms(13, 30, 0),
        Redmine::parse_time("01:30 PM").unwrap()
    );
}

#[test]
fn it_parses_iso_times_correctly() {
    assert_eq!(
        NaiveTime::from_hms(13, 30, 0),
        Redmine::parse_time("13:30").unwrap()
    );
}
