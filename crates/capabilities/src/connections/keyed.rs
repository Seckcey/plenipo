//! Connections with a key the owner makes in the service and types into its card (Phase 20 part
//! 20C; ADR-071 §1): HubSpot's service key, Stripe's restricted key, and the website's
//! Application Password (with an optional WooCommerce key).
//!
//! - **Save and check:** the key is checked with one reading call through Guard's gate; only a
//!   key the service accepts is kept, in the Vault, read back to check it, and never shown,
//!   returned, or recorded again.
//! - **Calls:** the key is read from the Vault for each call and added only for the service's own
//!   host (the website: the address saved on its card). A key the service stops accepting is
//!   erased, and the card asks for a new one.

use base64::Engine as _;
use plenipo_guard::{Account, ConnectionState, Service};
use serde::Deserialize;
use serde_json::Value;
use ts_rs::TS;

use super::http::{Auth, Body, HttpError, Reply};
use super::{lock, name_of, vault_id, Connections, MAX_TOKEN_ANSWER};
use crate::vault;

/// The longest key, password, or secret taken.
pub const MAX_KEY: usize = 400;
/// Stripe's web interface version Plenipo was built for (ADR-071 §6.5): its answers keep this
/// shape whatever the account's own setting.
pub const STRIPE_VERSION: &str = "2026-08-26.dahlia";

/// The Vault ID of the website's WooCommerce key (`ck_…:cs_…`).
pub fn store_key_id(connection_id: &str) -> String {
    format!("connection-{connection_id}-store-key")
}

/// What the owner types into a key card: only the fields its service takes, each checked; the
/// rest must be left out. Values go only to the Vault.
#[derive(Clone, Default, PartialEq, Eq, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct KeyInput {
    /// HubSpot's service key, or Stripe's restricted key.
    #[serde(default)]
    #[ts(optional)]
    pub key: Option<String>,
    /// The website's address (not a secret).
    #[serde(default)]
    #[ts(optional)]
    pub site: Option<String>,
    /// The WordPress user's name (not a secret).
    #[serde(default)]
    #[ts(optional)]
    pub user: Option<String>,
    /// Its Application Password.
    #[serde(default)]
    #[ts(optional)]
    pub password: Option<String>,
    /// The WooCommerce key (`ck_…`) and its secret (`cs_…`), both or neither.
    #[serde(default)]
    #[ts(optional)]
    pub store_key: Option<String>,
    #[serde(default)]
    #[ts(optional)]
    pub store_secret: Option<String>,
}

impl std::fmt::Debug for KeyInput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let hidden = |v: &Option<String>| v.as_ref().map(|_| "(hidden)");
        f.debug_struct("KeyInput")
            .field("key", &hidden(&self.key))
            .field("site", &self.site)
            .field("user", &self.user)
            .field("password", &hidden(&self.password))
            .field("store_key", &hidden(&self.store_key))
            .field("store_secret", &hidden(&self.store_secret))
            .finish()
    }
}

/// A key as typed, checked for its shape.
enum Typed {
    Hubspot(String),
    Stripe {
        key: String,
        live: bool,
    },
    Wordpress {
        site: String,
        user: String,
        password: String,
        store: Option<(String, String)>,
    },
}

/// One secret box's value: trimmed, not empty, one piece of at most [`MAX_KEY`] characters.
fn one_value(what: &str, v: Option<&String>) -> Result<String, String> {
    let v = v.map(|s| s.trim()).unwrap_or_default();
    if v.is_empty() {
        return Err(format!("Type {what}."));
    }
    if v.chars().count() > MAX_KEY || v.chars().any(|c| c.is_control() || c.is_whitespace()) {
        return Err(format!("That does not look like {what}."));
    }
    Ok(v.to_owned())
}

fn not_for(service: Service, field: &str, given: bool) -> Result<(), String> {
    if given {
        Err(format!("{} does not take {field}.", service.label()))
    } else {
        Ok(())
    }
}

/// A key typed for `service`, checked for its shape before anything is sent anywhere.
fn typed(service: Service, input: &KeyInput) -> Result<Typed, String> {
    let wordpress_only = [
        ("a site address", input.site.is_some()),
        ("a user name", input.user.is_some()),
        ("a password", input.password.is_some()),
        (
            "a WooCommerce key",
            input.store_key.is_some() || input.store_secret.is_some(),
        ),
    ];
    match service {
        Service::Hubspot => {
            for (field, given) in wordpress_only {
                not_for(service, field, given)?;
            }
            let key = one_value("HubSpot's service key", input.key.as_ref())?;
            if key.starts_with("rk_") || key.starts_with("sk_") {
                return Err("That is a Stripe key: type it into the Stripe card.".into());
            }
            Ok(Typed::Hubspot(key))
        }
        Service::Stripe => {
            for (field, given) in wordpress_only {
                not_for(service, field, given)?;
            }
            let key = one_value("Stripe's restricted key", input.key.as_ref())?;
            let live = if key.starts_with("rk_live_") {
                true
            } else if key.starts_with("rk_test_") {
                false
            } else if key.starts_with("sk_") {
                return Err(
                    "That is a secret key, which can do everything in your Stripe \
                            account. Plenipo takes only a restricted key (it starts rk_test_ or \
                            rk_live_): the steps are in Plenipo's documentation."
                        .into(),
                );
            } else if key.starts_with("pk_") {
                return Err(
                    "That is a publishable key, which only a web page uses. Plenipo \
                            takes a restricted key (it starts rk_test_ or rk_live_)."
                        .into(),
                );
            } else {
                return Err("A Stripe restricted key starts rk_test_ or rk_live_.".into());
            };
            Ok(Typed::Stripe { key, live })
        }
        Service::Wordpress => {
            not_for(
                service,
                "a key here (type the Application Password)",
                input.key.is_some(),
            )?;
            let site = plenipo_guard::connections::site_address(
                input.site.as_deref().unwrap_or_default(),
            )?;
            let user = input.user.as_deref().map(str::trim).unwrap_or_default();
            if user.is_empty()
                || user.chars().count() > 60
                || user.chars().any(|c| c.is_control() || c == ':')
            {
                return Err("Type the WordPress user's name (as it signs in).".into());
            }
            // Shown in groups of four with spaces; WordPress takes it with or without them.
            let password: String = input
                .password
                .as_deref()
                .unwrap_or_default()
                .chars()
                .filter(|c| *c != ' ')
                .collect();
            if password.is_empty() {
                return Err("Type the Application Password.".into());
            }
            if !(16..=64).contains(&password.len())
                || !password.chars().all(|c| c.is_ascii_alphanumeric())
            {
                return Err(
                    "That does not look like an Application Password (WordPress shows \
                            24 letters and digits, in groups of four)."
                        .into(),
                );
            }
            let store =
                match (&input.store_key, &input.store_secret) {
                    (None, None) => None,
                    (Some(k), Some(s)) if k.trim().is_empty() && s.trim().is_empty() => None,
                    (Some(k), Some(s)) => {
                        let k = one_value("the WooCommerce key", Some(k))?;
                        let s = one_value("the WooCommerce key's secret", Some(s))?;
                        let shaped = |v: &str, start: &str| {
                            v.strip_prefix(start).is_some_and(|rest| {
                                (20..=64).contains(&rest.len())
                                    && rest.chars().all(|c| c.is_ascii_alphanumeric())
                            })
                        };
                        if !shaped(&k, "ck_") || !shaped(&s, "cs_") {
                            return Err("A WooCommerce key starts ck_, and its secret cs_.".into());
                        }
                        Some((k, s))
                    }
                    _ => return Err(
                        "Type both the WooCommerce key (ck_…) and its secret (cs_…), or neither."
                            .into(),
                    ),
                };
            Ok(Typed::Wordpress {
                site,
                user: user.to_owned(),
                password,
                store,
            })
        }
        _ => Err(format!(
            "{} signs in in your browser: press Connect.",
            service.label()
        )),
    }
}

/// What a checked key gave: who it is for, and what it may do, in the service's words.
struct Checked {
    account: Account,
    granted: Vec<String>,
}

/// The same account, for replacing a key while connected: HubSpot's portal, Stripe's account
/// and mode, the website's user.
fn same_account(
    before: &Account,
    after: &Account,
    granted_before: &[String],
    granted: &[String],
) -> bool {
    let mode = |g: &[String]| g.iter().find(|x| x.ends_with(" mode")).cloned();
    let same_tenant = match (&before.tenant, &after.tenant) {
        (Some(a), Some(b)) => a == b,
        _ => true,
    };
    same_tenant && before.address == after.address && mode(granted_before) == mode(granted)
}

/// Why a WordPress or WooCommerce refusal happened, in plain words (never the site's own text).
pub(crate) fn wordpress_code(reply: &Reply) -> String {
    reply.json()["code"]
        .as_str()
        .map(|c| {
            c.chars()
                .filter(|x| x.is_ascii_alphanumeric() || *x == '_')
                .take(60)
                .collect()
        })
        .unwrap_or_default()
}

/// Which credential a website call uses: the Application Password, or (the store's addresses,
/// when one is kept) the WooCommerce key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Cred {
    Site,
    Store,
}

impl Connections {
    /// Save and check a key for connection `id` (ADR-071 §1). Kept only if the service accepts
    /// it; while connected, only a key for the same account replaces the old one. The caller has
    /// asked Guard.
    pub async fn save_key(&self, id: &str, input: &KeyInput) -> Result<(), String> {
        let conn = self.guard().connection(id).map_err(|e| e.to_string())?;
        let service = conn.service;
        let typed = typed(service, input)?;
        // The website's address first: the gate lets the check reach only that host. It is not
        // a secret, and is changed only while not connected.
        if let Typed::Wordpress { site, .. } = &typed {
            self.guard()
                .set_connection_site(id, site)
                .map_err(|e| e.to_string())?;
        }
        let turn = lock(&self.state).next(id);
        let (checked, main, store) = match &typed {
            Typed::Hubspot(key) => (self.check_hubspot(key).await?, key.clone(), None),
            Typed::Stripe { key, live } => {
                (self.check_stripe(key, *live).await?, key.clone(), None)
            }
            Typed::Wordpress {
                site,
                user,
                password,
                store,
            } => (
                self.check_wordpress(site, user, password, store.as_ref())
                    .await?,
                format!("{user}:{password}"),
                store.as_ref().map(|(k, s)| format!("{k}:{s}")),
            ),
        };
        {
            let _one = lock(&self.commit);
            if lock(&self.state).turn(id) != turn {
                return Err(format!(
                    "{} was disconnected, or another key was saved, meanwhile. Nothing was kept.",
                    service.label()
                ));
            }
            let now = self.guard().connection(id).map_err(|e| e.to_string())?;
            if now.state != ConnectionState::NotConnected {
                let before = now.account.clone().unwrap_or_default();
                if !same_account(&before, &checked.account, &now.granted, &checked.granted) {
                    return Err(format!(
                        "That key is for another {} account (or the other mode). To switch, press \
                         Disconnect first, then save the new key. Nothing was kept.",
                        service.label()
                    ));
                }
            }
            let previous = vault::read(self.store.as_ref(), &vault_id(id))
                .ok()
                .flatten();
            self.keep(id, &main, previous.as_deref())?;
            let store_id = store_key_id(id);
            let kept_store = match &store {
                Some(value) => {
                    let before = vault::read(self.store.as_ref(), &store_id).ok().flatten();
                    self.keep_at(&store_id, value, before.as_deref(), "the WooCommerce key")
                }
                None => vault::erase(self.store.as_ref(), &store_id).map_err(|e| {
                    format!(
                        "Plenipo could not remove the old WooCommerce key from {} ({e}).",
                        self.store.label()
                    )
                }),
            };
            if let Err(e) = kept_store.and_then(|()| {
                self.guard()
                    .connection_connected(id, None, checked.account, &checked.granted)
                    .map(|_| ())
                    .map_err(|e| e.to_string())
            }) {
                // Nothing kept that the owner cannot see or disconnect.
                let _ = match previous {
                    Some(p) if now.state != ConnectionState::NotConnected => {
                        vault::put(self.store.as_ref(), &vault_id(id), &p)
                    }
                    _ => vault::erase(self.store.as_ref(), &vault_id(id)),
                };
                if now.state == ConnectionState::NotConnected {
                    let _ = vault::erase(self.store.as_ref(), &store_id);
                }
                return Err(e);
            }
            lock(&self.state).problem.remove(id);
        }
        self.changed();
        Ok(())
    }

    /// Hide what the owner typed into a key card in any text from now on (memory only), whether
    /// or not it was kept.
    pub fn hide_typed(&self, input: &KeyInput) {
        const MAX_TYPED: usize = 40;
        let mut forms: Vec<String> = Vec::new();
        for v in [
            &input.key,
            &input.password,
            &input.store_key,
            &input.store_secret,
        ]
        .into_iter()
        .flatten()
        {
            let t = v.trim();
            if t.chars().count() >= 8 {
                forms.push(t.to_owned());
                let joined: String = t.chars().filter(|c| *c != ' ').collect();
                if joined != t {
                    forms.push(joined);
                }
            }
        }
        if forms.is_empty() {
            return;
        }
        {
            let mut s = lock(&self.state);
            for f in forms {
                if !s.typed.contains(&f) {
                    s.typed.push(f);
                }
            }
            let over = s.typed.len().saturating_sub(MAX_TYPED);
            s.typed.drain(..over);
        }
        self.changed();
    }

    /// One reading call to check a key, before anything is kept.
    async fn check_call(
        &self,
        service: Service,
        url: &str,
        auth: Auth<'_>,
    ) -> Result<Reply, String> {
        let headers: Vec<(&'static str, &str)> = if service == Service::Stripe {
            vec![("Stripe-Version", STRIPE_VERSION)]
        } else {
            Vec::new()
        };
        self.http
            .send_with(
                service,
                reqwest::Method::GET,
                url,
                Some(auth),
                &headers,
                Body::None,
                MAX_TOKEN_ANSWER * 4,
                true,
            )
            .await
            .map_err(|e| match (service, e) {
                (Service::Wordpress, HttpError::Refused(why)) => format!(
                    "{why} Your site sent Plenipo to another address: type your site's address \
                     exactly as your browser shows it once the site has loaded (with or without \
                     www.). Nothing was kept."
                ),
                (_, e) => format!("{e}. Nothing was kept."),
            })
    }

    async fn check_hubspot(&self, key: &str) -> Result<Checked, String> {
        let reply = self
            .check_call(
                Service::Hubspot,
                &format!("{}/contacts?limit=1", super::hubspot::OBJECTS),
                Auth::Bearer(key),
            )
            .await?;
        let mut granted = Vec::new();
        match reply.status {
            200..=299 => granted.push("crm.objects.contacts.read".to_owned()),
            401 => {
                return Err(
                    "HubSpot did not accept that key. Check you copied all of it, from \
                            Development → Keys → Service keys. Nothing was kept."
                        .into(),
                )
            }
            // The key is HubSpot's, but may not read contacts.
            403 => {}
            s => return Err(format!("HubSpot answered {s}. Nothing was kept.")),
        }
        // Which HubSpot account, when the key may say (only its number; nothing else is kept).
        let mut account = Account {
            name: "HubSpot".into(),
            ..Account::default()
        };
        if let Ok(r) = self
            .check_call(Service::Hubspot, super::hubspot::ACCOUNT, Auth::Bearer(key))
            .await
        {
            if let Some(portal) = r.json()["portalId"].as_u64().filter(|_| r.ok()) {
                account.name = format!("HubSpot account {portal}");
                account.tenant = Some(portal.to_string());
            }
        }
        Ok(Checked { account, granted })
    }

    async fn check_stripe(&self, key: &str, live: bool) -> Result<Checked, String> {
        let reply = self
            .check_call(
                Service::Stripe,
                &format!("{}/balance", super::stripe::API),
                Auth::Bearer(key),
            )
            .await?;
        match reply.status {
            200..=299 => {
                if reply.json()["livemode"].as_bool() != Some(live) {
                    return Err(
                        "Stripe's answer and the key disagree about test and live mode. \
                                Nothing was kept."
                            .into(),
                    );
                }
            }
            401 => {
                return Err(
                    "Stripe did not accept that key. Check you copied all of it (Stripe \
                            shows a restricted key only once). Nothing was kept."
                        .into(),
                )
            }
            // The key is Stripe's, but may not read the balance.
            403 => {}
            s => return Err(format!("Stripe answered {s}. Nothing was kept.")),
        }
        let mode = if live { "live mode" } else { "test mode" };
        let mut account = Account {
            name: "Stripe".into(),
            organization: Some(if live { "Live mode" } else { "Test mode" }.into()),
            ..Account::default()
        };
        if let Ok(r) = self
            .check_call(
                Service::Stripe,
                &format!("{}/account", super::stripe::API),
                Auth::Bearer(key),
            )
            .await
        {
            let v = r.json();
            if r.ok() {
                let shown = [
                    v["settings"]["dashboard"]["display_name"].as_str(),
                    v["business_profile"]["name"].as_str(),
                ]
                .into_iter()
                .flatten()
                .map(str::trim)
                .find(|n| !n.is_empty())
                .map(|n| {
                    n.chars()
                        .filter(|c| !c.is_control())
                        .take(80)
                        .collect::<String>()
                });
                if let Some(n) = shown {
                    account.name = n;
                }
                account.tenant = v["id"]
                    .as_str()
                    .filter(|a| a.starts_with("acct_") && a.len() <= 40)
                    .map(str::to_owned);
            }
        }
        Ok(Checked {
            account,
            granted: vec![mode.to_owned()],
        })
    }

    async fn check_wordpress(
        &self,
        site: &str,
        user: &str,
        password: &str,
        store: Option<&(String, String)>,
    ) -> Result<Checked, String> {
        let site_auth = Auth::Basic { user, password };
        let reply = self
            .check_call(
                Service::Wordpress,
                &format!("{site}/wp-json/wp/v2/users/me?context=edit"),
                site_auth,
            )
            .await?;
        let me = reply.json();
        if !reply.ok() || !me["id"].is_u64() {
            return Err(match (reply.status, wordpress_code(&reply).as_str()) {
                (401, "incorrect_password" | "invalid_username" | "invalid_email") => {
                    "Your site did not accept that user name and Application Password. Check \
                     both (the user name is the one it signs in with). Nothing was kept."
                        .into()
                }
                (401, c) if c.starts_with("application_passwords_disabled") => {
                    "Your site has Application Passwords turned off (a security plugin or the \
                     host may do this). Turn them on for this user, then try again. Nothing was \
                     kept."
                        .into()
                }
                (401, _) => "Your site did not see the password: some hosts and security \
                             plugins remove it on the way. The steps in Plenipo's documentation \
                             say what to check. Nothing was kept."
                    .into(),
                (404, _) => format!(
                    "No WordPress answered at {site} (its /wp-json/ said \"not found\"). Check the \
                     address; if WordPress is in a folder, add it (like https://example.com/blog). \
                     Nothing was kept."
                ),
                (s, _) if (200..300).contains(&s) => format!(
                    "{site} answered with a page, not WordPress's REST interface. Check the \
                     address. Nothing was kept."
                ),
                (s, _) => format!("Your site answered {s}. Nothing was kept."),
            });
        }
        let clean = |v: &Value| -> String {
            v.as_str()
                .unwrap_or_default()
                .chars()
                .filter(|c| !c.is_control())
                .take(80)
                .collect()
        };
        let mut granted: Vec<String> = me["roles"]
            .as_array()
            .map(|r| {
                r.iter()
                    .filter_map(Value::as_str)
                    .filter(|r| {
                        r.len() <= 40 && r.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
                    })
                    .map(|r| format!("role:{r}"))
                    .collect()
            })
            .unwrap_or_default();
        let store_auth = match store {
            Some((k, s)) => Auth::Basic {
                user: k.as_str(),
                password: s.as_str(),
            },
            None => site_auth,
        };
        let orders = self
            .check_call(
                Service::Wordpress,
                &format!("{site}/wp-json/wc/v3/orders?per_page=1"),
                store_auth,
            )
            .await;
        match (store.is_some(), orders) {
            (_, Ok(r)) if r.ok() => {
                granted.push(if store.is_some() {
                    "woocommerce:key".to_owned()
                } else {
                    "woocommerce".to_owned()
                });
            }
            (true, Ok(r)) if r.status == 404 => {
                return Err(
                    "Your site has no WooCommerce store to use that key with (its \
                            /wp-json/wc/v3/ said \"not found\"). Leave the WooCommerce key out. \
                            Nothing was kept."
                        .into(),
                )
            }
            (true, Ok(r)) if r.status == 401 || r.status == 403 => {
                return Err(
                    if wordpress_code(&r) == "woocommerce_rest_authentication_error"
                        && r.status == 401
                        && String::from_utf8_lossy(&r.body).contains("read permissions")
                    {
                        "That WooCommerce key cannot read (it is Write only). Make it Read, or \
                         Read/Write. Nothing was kept."
                    } else {
                        "WooCommerce did not accept that key and secret. Check both. Nothing was \
                         kept."
                    }
                    .into(),
                )
            }
            (true, Ok(r)) => {
                return Err(format!(
                    "WooCommerce answered {}. Nothing was kept.",
                    r.status
                ))
            }
            (true, Err(e)) => return Err(e),
            // No store, or this user may not manage it: the Store part's tools will say so.
            (false, _) => {}
        }
        let name = clean(&me["name"]);
        Ok(Checked {
            account: Account {
                name: if name.is_empty() {
                    clean(&me["slug"])
                } else {
                    name
                },
                address: site.to_owned(),
                organization: None,
                tenant: me["id"].as_u64().map(|i| i.to_string()),
            },
            granted,
        })
    }

    /// One call for key connection `id` to its service, with its key (or, for the website's
    /// store with a WooCommerce key kept, that key). `retry: false` for a request that must
    /// never be sent twice. A key the service no longer accepts is erased; the card asks for a
    /// new one.
    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn key_call(
        &self,
        id: &str,
        service: Service,
        cred: Cred,
        method: reqwest::Method,
        url: &str,
        headers: &[(&'static str, &str)],
        body: Body,
        limit: usize,
        retry: bool,
    ) -> Result<Reply, String> {
        let name = name_of(id);
        let conn = self.guard().connection(id).map_err(|e| e.to_string())?;
        match conn.state {
            ConnectionState::Connected => {}
            ConnectionState::NeedsSignIn => {
                return Err(format!(
                    "{name} needs a new key from the owner (Settings → Connections)."
                ))
            }
            ConnectionState::NotConnected => return Err(format!("{name} is not connected.")),
        }
        let turn = lock(&self.state).turn(id);
        let site_value = vault::read(self.store.as_ref(), &vault_id(id))
            .map_err(|e| format!("Plenipo could not read {name}'s key ({e})."))?
            .ok_or_else(|| format!("{name}'s key is missing from the Vault."))?;
        let store_value = match (service, cred) {
            (Service::Wordpress, Cred::Store) => {
                vault::read(self.store.as_ref(), &store_key_id(id)).map_err(|e| {
                    format!("Plenipo could not read {name}'s WooCommerce key ({e}).")
                })?
            }
            _ => None,
        };
        let (value, using_store) = match &store_value {
            Some(v) => (v.as_str(), true),
            None => (site_value.as_str(), false),
        };
        let auth = match service {
            Service::Wordpress => {
                let (user, password) = value
                    .rsplit_once(':')
                    .ok_or_else(|| format!("{name}'s key in the Vault is not whole."))?;
                Auth::Basic { user, password }
            }
            _ => Auth::Bearer(value),
        };
        let mut all: Vec<(&'static str, &str)> = headers.to_vec();
        if service == Service::Stripe {
            all.push(("Stripe-Version", STRIPE_VERSION));
        }
        let reply = self
            .http
            .send_with(service, method, url, Some(auth), &all, body, limit, retry)
            .await
            .map_err(|e| match e {
                HttpError::TooBig => format!("{name}'s answer was too big to use here."),
                other => other.to_string(),
            })?;
        if let Some(reason) = key_gone(service, using_store, &reply) {
            {
                let _commit = lock(&self.commit);
                // A key saved meanwhile stays.
                if lock(&self.state).turn(id) == turn {
                    let _ = vault::erase(self.store.as_ref(), &store_key_id(id));
                    self.forget_sign_in(id, &reason);
                }
            }
            self.changed();
            return Err(format!(
                "{name} needs a new key from the owner (Settings → Connections)."
            ));
        }
        Ok(reply)
    }
}

/// Whether the service's answer means it no longer accepts the key at all (not a missing
/// permission), and why in plain words.
fn key_gone(service: Service, store_key: bool, reply: &Reply) -> Option<String> {
    if reply.status != 401 {
        return None;
    }
    let name = service.label();
    match service {
        Service::Hubspot | Service::Stripe => Some(format!(
            "{name} no longer accepts the key (it was deleted, replaced, or expired)."
        )),
        Service::Wordpress => {
            let code = wordpress_code(reply);
            let body = String::from_utf8_lossy(&reply.body);
            let gone = if store_key {
                code == "woocommerce_rest_authentication_error"
                    && (body.contains("Consumer key is invalid")
                        || body.contains("Consumer secret is invalid"))
            } else {
                matches!(
                    code.as_str(),
                    "incorrect_password"
                        | "invalid_username"
                        | "invalid_email"
                        | "application_passwords_disabled"
                        | "application_passwords_disabled_for_user"
                )
            };
            gone.then(|| {
                if store_key {
                    "The website no longer accepts the WooCommerce key (it was revoked).".to_owned()
                } else {
                    "The website no longer accepts the Application Password (it was revoked, or \
                     Application Passwords were turned off)."
                        .to_owned()
                }
            })
        }
        _ => None,
    }
}

/// Every form of a kept key to hide in text: the value, its parts, and how it looks inside a
/// `Basic` sign-in (so a key is hidden even written the way it is sent).
pub(crate) fn key_forms(value: &str) -> Vec<String> {
    let mut out = vec![value.to_owned()];
    if let Some((user, secret)) = value.rsplit_once(':') {
        if secret.len() >= 8 {
            out.push(secret.to_owned());
            // An Application Password as WordPress shows it: in groups of four.
            if secret.len() == 24 && secret.chars().all(|c| c.is_ascii_alphanumeric()) {
                let grouped: Vec<String> = secret
                    .as_bytes()
                    .chunks(4)
                    .map(|c| String::from_utf8_lossy(c).into_owned())
                    .collect();
                out.push(grouped.join(" "));
            }
        }
        if user.starts_with("ck_") {
            out.push(user.to_owned());
        }
        out.push(base64::engine::general_purpose::STANDARD.encode(value));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(key: &str) -> KeyInput {
        KeyInput {
            key: Some(key.into()),
            ..KeyInput::default()
        }
    }

    #[test]
    fn keys_are_checked_for_their_shape_before_anything_is_sent() {
        assert!(matches!(
            typed(Service::Hubspot, &input(" plenipo-test-hubspot-typed ")),
            Ok(Typed::Hubspot(k)) if k == "plenipo-test-hubspot-typed"
        ));
        assert!(matches!(
            typed(Service::Stripe, &input("rk_test_abc123")),
            Ok(Typed::Stripe { live: false, .. })
        ));
        assert!(matches!(
            typed(Service::Stripe, &input("rk_live_abc123")),
            Ok(Typed::Stripe { live: true, .. })
        ));
        for (service, key, why) in [
            (Service::Stripe, "sk_live_abc", "only a restricted key"),
            (Service::Stripe, "pk_test_abc", "publishable key"),
            (Service::Stripe, "whsec_abc", "starts rk_test_"),
            (Service::Stripe, "", "Type Stripe's"),
            (Service::Stripe, "rk_test_ab c", "does not look like"),
            (Service::Hubspot, "rk_test_abc", "Stripe card"),
            (Service::Google, "x", "press Connect"),
        ] {
            let Err(err) = typed(service, &input(key)) else {
                panic!("{key} must be refused");
            };
            assert!(err.contains(why), "{key}: {err}");
        }
        // A field the service does not take is refused, not ignored.
        let mut with_site = input("pat-na1-x");
        with_site.site = Some("https://example.com".into());
        assert!(typed(Service::Hubspot, &with_site).is_err());
        let wp = KeyInput {
            site: Some("https://Shop.Example.com/".into()),
            user: Some("plenipo".into()),
            password: Some("abcd EFGH 1234 ijkl MNOP 5678".into()),
            ..KeyInput::default()
        };
        let Ok(Typed::Wordpress {
            site,
            password,
            store,
            ..
        }) = typed(Service::Wordpress, &wp)
        else {
            panic!("a whole website sign-in");
        };
        assert_eq!(site, "https://shop.example.com");
        assert_eq!(password, "abcdEFGH1234ijklMNOP5678");
        assert!(store.is_none());
        let half = KeyInput {
            store_key: Some(format!("ck_{}", "a".repeat(40))),
            ..wp.clone()
        };
        assert!(typed(Service::Wordpress, &half)
            .err()
            .unwrap()
            .contains("or neither"));
        let bad_site = KeyInput {
            site: Some("http://shop.example.com".into()),
            ..wp.clone()
        };
        assert!(typed(Service::Wordpress, &bad_site).is_err());
        let with_key = KeyInput {
            key: Some("x".into()),
            ..wp
        };
        assert!(typed(Service::Wordpress, &with_key).is_err());
    }

    #[test]
    fn a_kept_key_is_hidden_in_every_form_it_may_take() {
        let forms = key_forms("plenipo:abcdEFGH1234ijklMNOP5678");
        assert!(forms.contains(&"abcdEFGH1234ijklMNOP5678".to_owned()));
        assert!(forms.contains(&"abcd EFGH 1234 ijkl MNOP 5678".to_owned()));
        assert!(forms.contains(
            &base64::engine::general_purpose::STANDARD.encode("plenipo:abcdEFGH1234ijklMNOP5678")
        ));
        let store = format!("ck_{}:cs_{}", "1".repeat(40), "2".repeat(40));
        let forms = key_forms(&store);
        assert!(forms.contains(&format!("ck_{}", "1".repeat(40))));
        assert!(forms.contains(&format!("cs_{}", "2".repeat(40))));
        assert_eq!(key_forms("rk_test_abc"), ["rk_test_abc"]);
        assert!(!format!("{:?}", input("rk_live_secretvalue")).contains("secretvalue"));
    }
}
