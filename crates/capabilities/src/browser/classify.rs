//! Which clicks and key presses in Plenipo's browser are sensitive (Phase 10, ADR-020):
//! submitting a form or sending something, buying, and signing in always wait for the owner's
//! approval. Plenipo reads the control from the page (its kind, its words, its form) and errs
//! on the side of asking. What a page's own scripts do is caught by the network check in the
//! tab (data sent right after a worker's action also waits for approval).

use plenipo_guard::SensitiveKind;
use serde::Deserialize;

/// What the page helper reports about a control (`page.js`, `facts`).
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ElementFacts {
    pub found: bool,
    pub tag: String,
    pub r#type: String,
    pub role: String,
    /// Its words: label, text, or value.
    pub name: String,
    pub href: String,
    /// A form's submit button.
    pub submit: bool,
    /// A password, one-time-code, or card field.
    pub secret: bool,
    pub password: bool,
    pub autocomplete: String,
    pub editable: bool,
    pub disabled: bool,
    pub visible: bool,
    /// Part of a CAPTCHA (a check's own widget counts, with its checkbox as the click point).
    pub captcha: bool,
    /// The CAPTCHA it is part of is passed already: the website has its answer (ADR-032).
    pub solved: bool,
    pub form: Option<FormFacts>,
    /// Its center, in the page's viewport.
    pub x: f64,
    pub y: f64,
    /// Nothing covers its center (a click there reaches it).
    pub clear: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct FormFacts {
    pub has_password: bool,
    pub method: String,
    pub action: String,
    /// The words of its submit buttons.
    pub buttons: Vec<String>,
}

const SIGN_IN: &[&str] = &[
    "sign in", "signin", "log in", "login", "log on", "logon", "sign on",
];
const PAYMENT: &[&str] = &[
    "buy",
    "purchase",
    "place order",
    "place your order",
    "complete order",
    "complete purchase",
    "confirm order",
    "confirm purchase",
    "confirm payment",
    "pay",
    "pay now",
    "checkout",
    "check out",
    "subscribe",
    "donate",
    "book now",
    "add payment",
    "add card",
];
const OUTBOUND: &[&str] = &[
    "submit",
    "send",
    "post",
    "publish",
    "share",
    "reply",
    "comment",
    "tweet",
    "upload",
    "apply",
    "sign up",
    "signup",
    "register",
    "create account",
    "confirm",
    "save",
    "delete",
    "remove",
    "unsubscribe",
    "invite",
    "request",
    "transfer",
];
/// Parts of a link's or form's address that mean buying or signing in.
const PAYMENT_PATHS: &[&str] = &["checkout", "payment", "/pay", "purchase"];
const SIGN_IN_PATHS: &[&str] = &["login", "signin", "sign-in", "log-in", "/auth", "oauth"];

/// `text` contains `word` as whole words ("pay" in "Pay now", not in "Payload").
fn has_word(text: &str, word: &str) -> bool {
    let text = text.to_lowercase();
    let mut from = 0;
    while let Some(i) = text[from..].find(word) {
        let start = from + i;
        let end = start + word.len();
        let before = text[..start].chars().next_back();
        let after = text[end..].chars().next();
        let boundary = |c: Option<char>| c.is_none_or(|c| !c.is_alphanumeric());
        if boundary(before) && boundary(after) {
            return true;
        }
        from = end;
    }
    false
}

fn any_word(text: &str, words: &'static [&'static str]) -> Option<&'static str> {
    words.iter().copied().find(|w| has_word(text, w))
}

fn path_has(address: &str, parts: &[&str]) -> bool {
    let a = address.to_lowercase();
    // Only the path and query, not the host ("pay" in "paypal.com" is a site, not buying).
    let path = a.split_once("://").map_or(a.as_str(), |(_, rest)| {
        rest.find('/').map_or("", |i| &rest[i..])
    });
    parts.iter().any(|p| path.contains(p))
}

/// Why clicking this control is sensitive, if it is: the kind and a plain reason.
pub fn click(f: &ElementFacts) -> Option<(SensitiveKind, String)> {
    let words = f.name.as_str();
    let quoted = if words.is_empty() {
        format!("this {}", if f.tag == "a" { "link" } else { "button" })
    } else {
        format!("\"{words}\"")
    };
    if f.submit && f.form.as_ref().is_some_and(|x| x.has_password) {
        return Some((
            SensitiveKind::SignIn,
            format!("{quoted} sends a form with a password: it signs in"),
        ));
    }
    let form_action = f.form.as_ref().map_or("", |x| x.action.as_str());
    if let Some(w) = any_word(words, PAYMENT) {
        return Some((
            SensitiveKind::Payment,
            format!("{quoted} looks like buying or paying (\"{w}\")"),
        ));
    }
    if (!f.href.is_empty() && path_has(&f.href, PAYMENT_PATHS))
        || (f.submit && path_has(form_action, PAYMENT_PATHS))
    {
        return Some((
            SensitiveKind::Payment,
            format!("{quoted} leads to a checkout or payment page"),
        ));
    }
    if let Some(w) = any_word(words, SIGN_IN) {
        return Some((
            SensitiveKind::SignIn,
            format!("{quoted} looks like signing in (\"{w}\")"),
        ));
    }
    if f.submit && path_has(form_action, SIGN_IN_PATHS) {
        return Some((
            SensitiveKind::SignIn,
            format!("{quoted} sends a sign-in form"),
        ));
    }
    if f.submit {
        return Some((
            SensitiveKind::Outbound,
            format!("{quoted} submits a form: it sends what the form holds to the website"),
        ));
    }
    let is_control = matches!(f.tag.as_str(), "button" | "input") || f.role == "button";
    if is_control {
        if let Some(w) = any_word(words, OUTBOUND) {
            return Some((
                SensitiveKind::Outbound,
                format!("{quoted} looks like sending or changing something (\"{w}\")"),
            ));
        }
    }
    None
}

/// Why submitting the form a control belongs to (pressing Enter in it, or typing with
/// "submit") is sensitive: always, since it sends what the form holds; the kind says what it
/// looks like.
pub fn submit(f: &ElementFacts) -> (SensitiveKind, String) {
    let form = f.form.clone().unwrap_or_default();
    if form.has_password {
        return (
            SensitiveKind::SignIn,
            "it submits a form with a password: it signs in".into(),
        );
    }
    let buttons = form.buttons.join(" / ");
    if let Some(w) = any_word(&buttons, PAYMENT) {
        return (
            SensitiveKind::Payment,
            format!("it submits a form whose button says \"{w}\": it looks like buying"),
        );
    }
    if path_has(&form.action, PAYMENT_PATHS) {
        return (
            SensitiveKind::Payment,
            "it submits a checkout or payment form".into(),
        );
    }
    if any_word(&buttons, SIGN_IN).is_some() || path_has(&form.action, SIGN_IN_PATHS) {
        return (SensitiveKind::SignIn, "it submits a sign-in form".into());
    }
    (
        SensitiveKind::Outbound,
        "it submits a form: it sends what the form holds to the website".into(),
    )
}

/// Why pressing Enter on this control is sensitive, if it is: in a form it submits it; on a
/// button or link it clicks it.
pub fn enter(f: &ElementFacts) -> Option<(SensitiveKind, String)> {
    if !f.found {
        return None;
    }
    if f.editable && f.tag != "textarea" && f.form.is_some() {
        return Some(submit(f));
    }
    click(f)
}

/// Why an action whose stated purpose reads like signing in, buying, or sending is sensitive
/// (the screen tools: Plenipo cannot see what a click on the desktop does, so it reads the
/// worker's own words, and errs on the side of asking).
pub fn purpose(text: &str) -> Option<(SensitiveKind, String)> {
    let quoted = format!("\"{}\"", text.trim());
    if let Some(w) = any_word(text, PAYMENT) {
        return Some((
            SensitiveKind::Payment,
            format!("its purpose {quoted} looks like buying or paying (\"{w}\")"),
        ));
    }
    if let Some(w) = any_word(text, SIGN_IN) {
        return Some((
            SensitiveKind::SignIn,
            format!("its purpose {quoted} looks like signing in (\"{w}\")"),
        ));
    }
    if let Some(w) = any_word(text, OUTBOUND) {
        return Some((
            SensitiveKind::Outbound,
            format!("its purpose {quoted} looks like sending or changing something (\"{w}\")"),
        ));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn button(name: &str) -> ElementFacts {
        ElementFacts {
            found: true,
            tag: "button".into(),
            r#type: "button".into(),
            name: name.into(),
            ..ElementFacts::default()
        }
    }

    fn submit_in(name: &str, form: FormFacts) -> ElementFacts {
        ElementFacts {
            submit: true,
            r#type: "submit".into(),
            form: Some(form),
            ..button(name)
        }
    }

    fn kind(f: &ElementFacts) -> Option<SensitiveKind> {
        click(f).map(|(k, _)| k)
    }

    #[test]
    fn words_match_whole_words_only() {
        assert!(has_word("Pay now", "pay"));
        assert!(has_word("Proceed to CHECKOUT", "checkout"));
        assert!(!has_word("Payload viewer", "pay"));
        assert!(!has_word("Postcode lookup", "post"));
        assert!(has_word("Log in", "log in"));
    }

    #[test]
    fn buying_signing_in_and_sending_are_recognized() {
        assert_eq!(kind(&button("Buy now")), Some(SensitiveKind::Payment));
        assert_eq!(kind(&button("Place order")), Some(SensitiveKind::Payment));
        assert_eq!(kind(&button("Sign in")), Some(SensitiveKind::SignIn));
        assert_eq!(kind(&button("Send message")), Some(SensitiveKind::Outbound));
        assert_eq!(
            kind(&button("Delete account")),
            Some(SensitiveKind::Outbound)
        );
        // Harmless controls go ahead.
        assert_eq!(kind(&button("Next page")), None);
        assert_eq!(kind(&button("Show details")), None);
        assert_eq!(kind(&button("Search")), None);
        // Any form's submit button sends it; a form with a password signs in.
        let form = FormFacts {
            method: "post".into(),
            action: "https://shop.test/contact".into(),
            ..FormFacts::default()
        };
        assert_eq!(
            kind(&submit_in("Continue", form.clone())),
            Some(SensitiveKind::Outbound)
        );
        let login = FormFacts {
            has_password: true,
            ..form.clone()
        };
        assert_eq!(
            kind(&submit_in("Continue", login)),
            Some(SensitiveKind::SignIn)
        );
        let checkout = FormFacts {
            action: "https://shop.test/checkout/confirm".into(),
            ..form
        };
        assert_eq!(
            kind(&submit_in("Continue", checkout)),
            Some(SensitiveKind::Payment)
        );
        // Links: by where they lead, not the website's name.
        let link = |href: &str| ElementFacts {
            found: true,
            tag: "a".into(),
            name: "Continue".into(),
            href: href.into(),
            ..ElementFacts::default()
        };
        assert_eq!(
            kind(&link("https://shop.test/checkout")),
            Some(SensitiveKind::Payment)
        );
        assert_eq!(kind(&link("https://paypal.com/about")), None);
        assert_eq!(kind(&link("https://shop.test/display/orders")), None);
        assert_eq!(kind(&link("https://example.com/docs")), None);
    }

    #[test]
    fn a_stated_purpose_is_read_too() {
        assert_eq!(
            purpose("click Send in the mail app").unwrap().0,
            SensitiveKind::Outbound
        );
        assert_eq!(purpose("press Buy").unwrap().0, SensitiveKind::Payment);
        assert_eq!(
            purpose("log in to the portal").unwrap().0,
            SensitiveKind::SignIn
        );
        assert_eq!(purpose("open the File menu"), None);
    }

    #[test]
    fn enter_in_a_form_submits_it() {
        let field = ElementFacts {
            found: true,
            tag: "input".into(),
            r#type: "text".into(),
            editable: true,
            form: Some(FormFacts {
                buttons: vec!["Pay $20".into()],
                ..FormFacts::default()
            }),
            ..ElementFacts::default()
        };
        assert_eq!(enter(&field).unwrap().0, SensitiveKind::Payment);
        let search = ElementFacts {
            form: Some(FormFacts {
                buttons: vec!["Search".into()],
                method: "get".into(),
                ..FormFacts::default()
            }),
            ..field.clone()
        };
        assert_eq!(enter(&search).unwrap().0, SensitiveKind::Outbound);
        let outside = ElementFacts {
            form: None,
            ..field
        };
        assert_eq!(
            enter(&outside),
            None,
            "Enter outside a form submits nothing"
        );
        assert_eq!(enter(&ElementFacts::default()), None);
    }
}
