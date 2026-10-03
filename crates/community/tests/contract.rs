//! The written contract (`contracts/community/v1`) read into this crate's types: every example in
//! the contract's folder, as the schema entry named in `examples/index.json`. The account
//! service tests its own answers against the same files, so if these tests pass, Plenipo and the
//! service mean the same thing by every body.
//!
//! - A **request** (what Plenipo sends, and what travels sealed) must refuse a field the contract
//!   does not name, and must write back exactly what was read, nulls and all.
//! - An **answer** (what the service sends) must ignore a field it does not know, and must not
//!   lose any field it was given.

use std::path::{Path, PathBuf};

use plenipo_community::wire::*;
use serde::{de::DeserializeOwned, Serialize};
use serde_json::{json, Map, Value};

fn contract_folder() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../contracts/community/v1")
}

fn examples_folder() -> PathBuf {
    contract_folder().join("examples")
}

fn read_json(path: &Path) -> Value {
    let text = std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("{} can't be read: {e}", path.display()));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("{} is not JSON: {e}", path.display()))
}

fn read_example(file: &str) -> Value {
    read_json(&examples_folder().join(file))
}

fn schema() -> Value {
    read_json(&contract_folder().join("schema/community.schema.json"))
}

/// Each example file and the schema entry it passes, in file-name order.
fn index() -> Vec<(String, String)> {
    let index = read_example("index.json");
    let examples = index["examples"]
        .as_object()
        .expect("`examples` in index.json is an object");
    examples
        .iter()
        .map(|(file, name)| {
            let name = name
                .as_str()
                .unwrap_or_else(|| panic!("index.json: {file} does not name a schema entry"));
            (file.clone(), name.to_owned())
        })
        .collect()
}

/// How a body travels, and so how strictly it is read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Way {
    /// Plenipo sends it, or it travels sealed: a field the contract does not name is refused.
    Request,
    /// Sent and read back (the profile): unknown fields are ignored, and it writes back exactly.
    Both,
    /// The service sends it: unknown fields are ignored.
    Answer,
}

/// Reads an example into a type, and gives back what that type writes.
type Reads = fn(&Value) -> Result<Value, String>;

fn reads<T: DeserializeOwned + Serialize>(example: &Value) -> Result<Value, String> {
    let value: T = serde_json::from_value(example.clone()).map_err(|e| e.to_string())?;
    serde_json::to_value(&value).map_err(|e| e.to_string())
}

struct Arm {
    way: Way,
    reads: Reads,
}

fn arm<T: DeserializeOwned + Serialize>(way: Way) -> Arm {
    Arm {
        way,
        reads: reads::<T>,
    }
}

/// The Rust type for each schema entry the index names.
fn arm_for(name: &str) -> Option<Arm> {
    use Way::{Answer, Both, Request};
    Some(match name {
        "Error" => arm::<Error>(Answer),
        "Empty" => arm::<Empty>(Answer),
        "Open" => arm::<Open>(Answer),
        "SignInStart" => arm::<SignInStart>(Request),
        "SignInStarted" => arm::<SignInStarted>(Answer),
        "SignInToken" => arm::<SignInToken>(Request),
        "SignedIn" => arm::<SignedIn>(Answer),
        "Me" => arm::<Me>(Answer),
        "Join" => arm::<Join>(Request),
        "NameChange" => arm::<NameChange>(Request),
        "TermsAccept" => arm::<TermsAccept>(Request),
        "Profile" => arm::<Profile>(Both),
        "PictureUpload" => arm::<PictureUpload>(Request),
        "Presence" => arm::<Presence>(Request),
        "Card" => arm::<Card>(Answer),
        "CardPage" => arm::<CardPage>(Answer),
        "RequestOnly" => arm::<RequestOnly>(Answer),
        "Devices" => arm::<Devices>(Answer),
        "Contacts" => arm::<Contacts>(Answer),
        "ItemSend" => arm::<ItemSend>(Request),
        "ItemSent" => arm::<ItemSent>(Answer),
        "Inbox" => arm::<Inbox>(Answer),
        "Ack" => arm::<Ack>(Request),
        "StampPayload" => arm::<StampPayload>(Answer),
        "SealedContent" => arm::<SealedContent>(Request),
        "ItemPayload" => arm::<ItemPayload>(Request),
        "Blocks" => arm::<Blocks>(Answer),
        "LinkAsk" => arm::<LinkAsk>(Request),
        "LinkAccept" => arm::<LinkAccept>(Request),
        "LinkPause" => arm::<LinkPause>(Request),
        "Link" => arm::<Link>(Answer),
        "Links" => arm::<Links>(Answer),
        "CollabInvite" => arm::<CollabInvite>(Request),
        "Collaboration" => arm::<Collaboration>(Answer),
        "Collaborations" => arm::<Collaborations>(Answer),
        "Report" => arm::<Report>(Request),
        "ReportMade" => arm::<ReportMade>(Answer),
        "Thanks" => arm::<Thanks>(Request),
        "Points" => arm::<Points>(Answer),
        "Leaderboard" => arm::<Leaderboard>(Answer),
        "InviteEmail" => arm::<InviteEmail>(Request),
        _ => return None,
    })
}

/// Each example with its arm. Fails, naming the file, when the index names an entry with no arm.
fn examples() -> Vec<(String, String, Arm, Value)> {
    let mut found = Vec::new();
    let mut missing = Vec::new();
    for (file, name) in index() {
        match arm_for(&name) {
            Some(arm) => {
                let example = read_example(&file);
                found.push((file, name, arm, example));
            }
            None => missing.push(format!("{file}: no arm for `{name}`")),
        }
    }
    assert!(
        missing.is_empty(),
        "the index names schema entries this test has no arm for:\n{}",
        missing.join("\n")
    );
    found
}

fn nothing_failed(what: &str, failed: Vec<String>) {
    assert!(failed.is_empty(), "{what}:\n{}", failed.join("\n"));
}

/// The first place where `written` lost or changed something `sent` had. A key written as `null`
/// that was not sent is fine: it is a notice's field that does not apply.
fn first_difference(sent: &Value, written: &Value, at: &str) -> Option<String> {
    match (sent, written) {
        (Value::Object(sent), Value::Object(written)) => {
            for (key, value) in sent {
                let Some(ours) = written.get(key) else {
                    return Some(format!("{at}/{key} was lost"));
                };
                if let Some(found) = first_difference(value, ours, &format!("{at}/{key}")) {
                    return Some(found);
                }
            }
            written
                .iter()
                .find(|(key, value)| !sent.contains_key(*key) && !value.is_null())
                .map(|(key, _)| format!("{at}/{key} was added"))
        }
        (Value::Array(sent), Value::Array(written)) => {
            if sent.len() != written.len() {
                return Some(format!(
                    "{at} had {} entries and now has {}",
                    sent.len(),
                    written.len()
                ));
            }
            sent.iter()
                .zip(written)
                .enumerate()
                .find_map(|(i, (s, w))| first_difference(s, w, &format!("{at}/{i}")))
        }
        _ if sent == written => None,
        _ => Some(format!("{at} was {sent} and is now {written}")),
    }
}

/// The same JSON with `later_field` in the top object, or in every object.
fn with_later_field(value: &Value, everywhere: bool) -> Value {
    match value {
        Value::Object(map) => {
            let mut map: Map<String, Value> = if everywhere {
                map.iter()
                    .map(|(k, v)| (k.clone(), with_later_field(v, true)))
                    .collect()
            } else {
                map.clone()
            };
            map.insert("later_field".to_owned(), json!(1));
            Value::Object(map)
        }
        Value::Array(items) if everywhere => Value::Array(
            items
                .iter()
                .map(|item| with_later_field(item, true))
                .collect(),
        ),
        other => other.clone(),
    }
}

#[test]
fn every_example_reads_into_its_type() {
    let mut failed = Vec::new();
    for (file, name, arm, example) in examples() {
        if let Err(why) = (arm.reads)(&example) {
            failed.push(format!("{file} (as {name}): {why}"));
        }
    }
    nothing_failed("these examples do not read into their types", failed);
}

#[test]
fn every_example_file_is_in_the_index_and_every_index_name_is_in_the_schema() {
    let schema = schema();
    let named: Vec<(String, String)> = index();
    let mut problems = Vec::new();
    for (file, name) in &named {
        if schema.pointer(&format!("/$defs/{name}")).is_none() {
            problems.push(format!(
                "{file}: `{name}` is not a $defs entry in the schema"
            ));
        }
        if !examples_folder().join(file).is_file() {
            problems.push(format!("{file}: in the index, but not in the folder"));
        }
    }
    let entries = std::fs::read_dir(examples_folder()).expect("the examples folder");
    for entry in entries {
        let file = entry
            .expect("a folder entry")
            .file_name()
            .to_string_lossy()
            .into_owned();
        // The index is not an example, and the worked vectors are not bodies. The worked item has
        // its own test below.
        let skipped = file == "index.json" || file.starts_with("vector-");
        if !skipped && !named.iter().any(|(listed, _)| *listed == file) {
            problems.push(format!("{file}: in the folder, but not in the index"));
        }
    }
    nothing_failed("the index and the folder disagree", problems);
}

#[test]
fn requests_write_back_exactly_what_was_read() {
    let mut failed = Vec::new();
    for (file, name, arm, example) in examples() {
        if arm.way == Way::Answer {
            continue;
        }
        match (arm.reads)(&example) {
            Ok(written) if written == example => {}
            Ok(written) => failed.push(format!(
                "{file} (as {name}) writes back as:\n  {written}\nbut the example is:\n  {example}"
            )),
            Err(why) => failed.push(format!("{file} (as {name}): {why}")),
        }
    }
    nothing_failed("these requests do not round-trip", failed);
}

#[test]
fn answers_lose_nothing_they_were_given() {
    let mut failed = Vec::new();
    for (file, name, arm, example) in examples() {
        if arm.way != Way::Answer {
            continue;
        }
        match (arm.reads)(&example) {
            Ok(written) => {
                if let Some(found) = first_difference(&example, &written, "") {
                    failed.push(format!("{file} (as {name}): {found}"));
                }
            }
            Err(why) => failed.push(format!("{file} (as {name}): {why}")),
        }
    }
    nothing_failed("these answers lose fields on the way through", failed);
}

#[test]
fn answers_ignore_fields_they_do_not_know() {
    let mut failed = Vec::new();
    for (file, name, arm, example) in examples() {
        if arm.way == Way::Request {
            continue;
        }
        for (everywhere, where_) in [(false, "at the top"), (true, "in every object")] {
            let later = with_later_field(&example, everywhere);
            if let Err(why) = (arm.reads)(&later) {
                failed.push(format!(
                    "{file} (as {name}) is refused with `later_field` {where_}: {why}"
                ));
            }
        }
    }
    nothing_failed(
        "these answers should ignore a field they do not know",
        failed,
    );
}

#[test]
fn requests_refuse_fields_they_do_not_know() {
    let mut failed = Vec::new();
    for (file, name, arm, example) in examples() {
        if arm.way != Way::Request {
            continue;
        }
        let later = with_later_field(&example, false);
        if (arm.reads)(&later).is_ok() {
            failed.push(format!(
                "{file} (as {name}) is read even with `later_field` at the top"
            ));
        }
    }
    nothing_failed(
        "these requests should refuse a field they do not know",
        failed,
    );
}

#[test]
fn the_parts_of_a_request_refuse_unknown_fields_too() {
    // A copy inside `ItemSend`, and an item inside `Report`.
    let mut send = read_example("item-send.json");
    send["copies"][0]["later_field"] = json!(1);
    assert!(serde_json::from_value::<ItemSend>(send).is_err());

    let mut report = read_example("report.json");
    report["items"][0]["later_field"] = json!(1);
    assert!(serde_json::from_value::<Report>(report).is_err());
}

#[test]
fn the_worked_item_writes_back_byte_for_byte() {
    let vector = read_example("vector-item.json");

    // The item, as signed: field order is the contract's, so the bytes are the same.
    let payload_json = vector["payload_json"].as_str().expect("payload_json");
    let payload: ItemPayload<MessageBody> =
        serde_json::from_str(payload_json).expect("payload_json reads as a message");
    assert_eq!(payload.kind, ItemKind::Message);
    assert_eq!(
        serde_json::to_string(&payload).expect("JSON"),
        payload_json,
        "payload_json does not write back byte for byte"
    );

    // The stamp's payload, the same way.
    let stamp_json = vector["stamp_payload_json"]
        .as_str()
        .expect("stamp_payload_json");
    let stamp: StampPayload =
        serde_json::from_str(stamp_json).expect("stamp_payload_json reads as a stamp payload");
    assert_eq!(
        serde_json::to_string(&stamp).expect("JSON"),
        stamp_json,
        "stamp_payload_json does not write back byte for byte"
    );
    assert_eq!(stamp.item_id, payload.item_id);
    assert_eq!(stamp.kind, payload.kind);
}

/// Reads a body as the type for its kind, and gives back what that type writes.
fn body_reads(kind: ItemKind, body: &Value) -> Result<Value, String> {
    match kind {
        ItemKind::Message => reads::<MessageBody>(body),
        ItemKind::Reaction => reads::<ReactionBody>(body),
        ItemKind::LinkNote => reads::<LinkNoteBody>(body),
        ItemKind::Objective => reads::<ObjectiveBody>(body),
        ItemKind::ObjectiveState => reads::<ObjectiveStateBody>(body),
        ItemKind::Answer => reads::<AnswerBody>(body),
        ItemKind::CollabNote => reads::<CollabNoteBody>(body),
    }
}

#[test]
fn the_body_in_each_payload_example_is_the_body_of_its_kind() {
    let mut failed = Vec::new();
    for file in [
        "item-payload-message.json",
        "item-payload-objective.json",
        "item-payload-collab-note.json",
    ] {
        let example = read_example(file);
        let payload: ItemPayload = serde_json::from_value(example).expect(file);
        match body_reads(payload.kind, &payload.body) {
            Ok(written) if written == payload.body => {}
            Ok(written) => failed.push(format!(
                "{file}: the body writes back as {written}, not {}",
                payload.body
            )),
            Err(why) => failed.push(format!("{file}: {why}")),
        }
        // A body of some other kind is not accepted for this one.
        let other = if payload.kind == ItemKind::Message {
            ItemKind::Objective
        } else {
            ItemKind::Message
        };
        if body_reads(other, &payload.body).is_ok() {
            failed.push(format!(
                "{file}: the body is also read as a {}",
                other.as_str()
            ));
        }
    }
    nothing_failed("these payload bodies are not read right", failed);
}

#[test]
fn every_kind_of_body_reads_writes_back_and_refuses_extras() {
    let item = "ci_01JC0D1E2F3G4H5J6K7M8N9P0Q";
    let samples = [
        (
            ItemKind::Message,
            json!({"text": "Hi!", "gif": null, "sticker": null, "reply_to": null}),
        ),
        (
            ItemKind::Message,
            json!({
                "text": null,
                "gif": {"library": "example", "id": "abc123"},
                "sticker": null,
                "reply_to": item,
            }),
        ),
        (
            ItemKind::Message,
            json!({
                "text": "Look",
                "gif": null,
                "sticker": {"set": "classic", "name": "thumbs-up"},
                "reply_to": null,
            }),
        ),
        (ItemKind::Reaction, json!({"item": item, "emoji": "👍"})),
        (ItemKind::Reaction, json!({"item": item, "emoji": null})),
        (
            ItemKind::LinkNote,
            json!({"org_name": "Acme", "note": null}),
        ),
        (
            ItemKind::Objective,
            json!({"org_name": "Acme", "text": "Please renew the domain."}),
        ),
        (
            ItemKind::ObjectiveState,
            json!({"objective": item, "state": "refused", "why": "Not now."}),
        ),
        (
            ItemKind::Answer,
            json!({"objective": item, "text": "Done."}),
        ),
        (
            ItemKind::CollabNote,
            json!({"org_name": "Acme", "role": "viewer", "part": "whole", "owner_only": []}),
        ),
        (
            ItemKind::CollabNote,
            json!({
                "org_name": "Acme",
                "role": "manager",
                "part": [
                    {"kind": "department", "name": "Sales"},
                    {"kind": "project", "name": "Website"},
                ],
                "owner_only": ["dns", "payments"],
            }),
        ),
    ];
    let mut failed = Vec::new();
    for (kind, body) in samples {
        let name = kind.as_str();
        match body_reads(kind, &body) {
            Ok(written) if written == body => {}
            Ok(written) => failed.push(format!("{name}: {body} writes back as {written}")),
            Err(why) => failed.push(format!("{name}: {body} is not read: {why}")),
        }
        if body_reads(kind, &with_later_field(&body, false)).is_ok() {
            failed.push(format!(
                "{name}: {body} is read with `later_field` at the top"
            ));
        }
    }

    // Inside a collaboration's `part`, too, and a word that is not `whole`.
    let mut part = json!({
        "org_name": "Acme", "role": "viewer", "owner_only": [],
        "part": [{"kind": "project", "name": "Website", "later_field": 1}],
    });
    if body_reads(ItemKind::CollabNote, &part).is_ok() {
        failed.push("a part with `later_field` inside is read".to_owned());
    }
    part["part"] = json!("everything");
    if body_reads(ItemKind::CollabNote, &part).is_ok() {
        failed.push("a part that is `everything` is read".to_owned());
    }
    part["part"] = json!([]);
    part["owner_only"] = json!(["not_an_approval"]);
    if body_reads(ItemKind::CollabNote, &part).is_ok() {
        failed.push("an owner-only approval the contract does not name is read".to_owned());
    }
    nothing_failed("these bodies are not read right", failed);
}

/// The words in a schema property: its `const`, or what is in its `enum` (a `null` is left out).
fn words_in(property: &Value) -> Vec<String> {
    let mut words = Vec::new();
    if let Some(word) = property.get("const").and_then(Value::as_str) {
        words.push(word.to_owned());
    }
    if let Some(list) = property.get("enum").and_then(Value::as_array) {
        words.extend(list.iter().filter_map(Value::as_str).map(str::to_owned));
    }
    words
}

/// The words in the schema at `pointer`.
fn words_at(schema: &Value, pointer: &str) -> Vec<String> {
    let property = schema
        .pointer(pointer)
        .unwrap_or_else(|| panic!("the schema has nothing at {pointer}"));
    words_in(property)
}

/// The words of one field across every shape a notice can take.
fn notice_words(schema: &Value, field: &str) -> Vec<String> {
    let shapes = schema
        .pointer("/$defs/Notice/oneOf")
        .and_then(Value::as_array)
        .expect("Notice has shapes");
    shapes
        .iter()
        .filter_map(|shape| shape.pointer(&format!("/properties/{field}")))
        .flat_map(words_in)
        .collect()
}

/// Each word must read into `T` and write back as the same word, and a word the contract does
/// not have must not read.
fn word_problems<T: DeserializeOwned + Serialize>(what: &str, words: &[String]) -> Vec<String> {
    let mut problems = Vec::new();
    if words.is_empty() {
        problems.push(format!("{what}: the schema has no words to check"));
    }
    for word in words {
        match serde_json::from_value::<T>(json!(word)) {
            Ok(value) => match serde_json::to_value(&value) {
                Ok(written) if written == json!(word) => {}
                other => problems.push(format!("{what}: `{word}` writes back as {other:?}")),
            },
            Err(why) => problems.push(format!("{what}: `{word}` is not read: {why}")),
        }
    }
    if serde_json::from_value::<T>(json!("not_in_the_contract")).is_ok() {
        problems.push(format!("{what}: a word the contract does not have is read"));
    }
    problems
}

/// The same, for a list of words in an answer that a newer account service may add to: a word
/// the contract does not have reads as `unknown`, so the rest of the answer still reads.
fn answer_word_problems<T: DeserializeOwned + Serialize>(
    what: &str,
    words: &[String],
) -> Vec<String> {
    let mut problems = Vec::new();
    if words.is_empty() {
        problems.push(format!("{what}: the schema has no words to check"));
    }
    for word in words {
        match serde_json::from_value::<T>(json!(word)) {
            Ok(value) => match serde_json::to_value(&value) {
                Ok(written) if written == json!(word) => {}
                other => problems.push(format!("{what}: `{word}` writes back as {other:?}")),
            },
            Err(why) => problems.push(format!("{what}: `{word}` is not read: {why}")),
        }
    }
    match serde_json::from_value::<T>(json!("not_in_the_contract")).map(|v| serde_json::to_value(v))
    {
        Ok(Ok(written)) if written == json!("unknown") => {}
        other => problems.push(format!(
            "{what}: a newer word reads as {other:?}, not unknown"
        )),
    }
    problems
}

#[test]
fn every_word_in_the_schema_is_spelled_as_the_schema_spells_it() {
    let schema = schema();
    let at = |pointer: &str| words_at(&schema, pointer);
    let mut problems = Vec::new();
    let mut check = |found: Vec<String>| problems.extend(found);

    check(word_problems::<ItemKind>(
        "ItemKind",
        &at("/$defs/ItemKind"),
    ));
    check(answer_word_problems::<Badge>("Badge", &at("/$defs/Badge")));
    check(word_problems::<BusinessKind>(
        "BusinessKind",
        &at("/$defs/BusinessKind"),
    ));
    check(answer_word_problems::<HiddenPart>(
        "HiddenPart",
        &at("/$defs/HiddenPart"),
    ));
    check(answer_word_problems::<PointsReason>(
        "PointsReason",
        &at("/$defs/PointsReason"),
    ));
    check(word_problems::<ProfileStatus>(
        "Profile.status",
        &at("/$defs/Profile/properties/status"),
    ));
    check(word_problems::<Mood>(
        "Profile.mood",
        &at("/$defs/Profile/properties/mood"),
    ));
    check(answer_word_problems::<CardStatus>(
        "Card.status",
        &at("/$defs/Card/properties/status"),
    ));
    check(word_problems::<AgeGroup>(
        "Member.age_group",
        &at("/$defs/Member/properties/age_group"),
    ));
    check(word_problems::<Standing>(
        "Member.standing",
        &at("/$defs/Member/properties/standing"),
    ));
    check(answer_word_problems::<ContactState>(
        "Contact.state",
        &at("/$defs/Contact/properties/state"),
    ));
    check(word_problems::<AskedBy>(
        "Link.asked_by",
        &at("/$defs/Link/properties/asked_by"),
    ));
    check(word_problems::<LinkState>(
        "Link.state",
        &at("/$defs/Link/properties/state"),
    ));
    check(word_problems::<CollabState>(
        "Collaboration.state",
        &at("/$defs/Collaboration/properties/state"),
    ));
    check(word_problems::<ObjectiveState>(
        "ObjectiveStateBody.state",
        &at("/$defs/ObjectiveStateBody/properties/state"),
    ));
    check(word_problems::<CollabRole>(
        "CollabNoteBody.role",
        &at("/$defs/CollabNoteBody/properties/role"),
    ));
    check(word_problems::<OwnerOnly>(
        "CollabNoteBody.owner_only",
        &at("/$defs/CollabNoteBody/properties/owner_only/items"),
    ));
    check(word_problems::<PartKind>(
        "CollabNoteBody.part",
        &at("/$defs/CollabNoteBody/properties/part/oneOf/1/items/properties/kind"),
    ));
    check(word_problems::<ReportReason>(
        "Report.reason",
        &at("/$defs/Report/properties/reason"),
    ));
    check(word_problems::<ReportWhat>(
        "Report.what",
        &at("/$defs/Report/properties/what"),
    ));
    check(word_problems::<ThanksFor>(
        "Thanks.for",
        &at("/$defs/Thanks/properties/for"),
    ));
    check(word_problems::<LeaderboardPeriod>(
        "Leaderboard.period",
        &at("/$defs/Leaderboard/properties/period"),
    ));
    check(answer_word_problems::<NoticeType>(
        "Notice.type",
        &notice_words(&schema, "type"),
    ));
    check(word_problems::<NoticeStanding>(
        "Notice.standing",
        &notice_words(&schema, "standing"),
    ));
    check(word_problems::<ReportOutcome>(
        "Notice.outcome",
        &notice_words(&schema, "outcome"),
    ));

    // `InboxItem.kind` is an item kind, or `notice`.
    let mut kinds = at("/$defs/ItemKind");
    kinds.push("notice".to_owned());
    check(word_problems::<InboxKind>("InboxItem.kind", &kinds));

    nothing_failed(
        "some words are not spelled as the schema spells them",
        problems,
    );
}

#[test]
fn every_notice_in_the_contract_reads() {
    let member = "cm_01JA2B3C4D5E6F7G8H9J0K1M2N";
    let link = "cl_01JD5E6F7G8H9J0K1M2N3P4Q5R";
    let collab = "cc_01JE9F0G1H2J3K4M5N6P7Q8R9S";
    let report = "cr_01JF1G2H3J4K5M6N7P8Q9R0S1T";
    let notices = [
        (
            NoticeType::ContactAccepted,
            json!({"type": "contact_accepted", "member_id": member}),
        ),
        (
            NoticeType::LinkRequested,
            json!({"type": "link_requested", "link_id": link, "member_id": member}),
        ),
        (
            NoticeType::LinkAccepted,
            json!({"type": "link_accepted", "link_id": link}),
        ),
        (
            NoticeType::LinkPaused,
            json!({"type": "link_paused", "link_id": link, "paused": true}),
        ),
        (
            NoticeType::LinkEnded,
            json!({"type": "link_ended", "link_id": link}),
        ),
        (
            NoticeType::CollabInvited,
            json!({"type": "collab_invited", "collab_id": collab, "member_id": member}),
        ),
        (
            NoticeType::CollabAccepted,
            json!({"type": "collab_accepted", "collab_id": collab}),
        ),
        (
            NoticeType::CollabEnded,
            json!({"type": "collab_ended", "collab_id": collab}),
        ),
        (
            NoticeType::StandingChanged,
            json!({"type": "standing_changed", "standing": "warned", "paused_until": null}),
        ),
        (
            NoticeType::StandingChanged,
            json!({"type": "standing_changed", "standing": "paused", "paused_until": 1790100000}),
        ),
        (
            NoticeType::ReportClosed,
            json!({"type": "report_closed", "report_id": report, "outcome": "no_action"}),
        ),
        (
            NoticeType::MyDevicesChanged,
            json!({"type": "my_devices_changed"}),
        ),
        (
            NoticeType::ProfileHidden,
            json!({"type": "profile_hidden", "parts": ["picture", "company"]}),
        ),
    ];
    let mut failed = Vec::new();
    for (kind, example) in notices {
        // Each notice also comes inside an inbox item, which is how it arrives.
        let item = json!({
            "item_id": "ci_01JC0D1E2F3G4H5J6K7M8N9P0R",
            "kind": "notice",
            "from": null,
            "ref": null,
            "sealed": null,
            "tag": null,
            "stamp": null,
            "accepted_at": 1790000005,
            "request": false,
            "notice": example,
        });
        match serde_json::from_value::<InboxItem>(item.clone()) {
            Ok(read) => {
                let notice = read.notice.as_ref().expect("a notice");
                if read.kind != InboxKind::Notice || notice.kind != kind {
                    failed.push(format!("{example}: read as the wrong kind"));
                }
                let written = serde_json::to_value(&read).expect("JSON");
                if let Some(found) = first_difference(&item, &written, "") {
                    failed.push(format!("{example}: {found}"));
                }
            }
            Err(why) => failed.push(format!("{example}: {why}")),
        }
    }
    nothing_failed("these notices are not read right", failed);
}

#[test]
fn an_inbox_item_is_an_item_kind_or_a_notice() {
    assert_eq!(
        serde_json::from_value::<InboxKind>(json!("notice")).expect("notice"),
        InboxKind::Notice
    );
    assert_eq!(
        serde_json::from_value::<InboxKind>(json!("link_note")).expect("link_note"),
        InboxKind::Item(ItemKind::LinkNote)
    );
    assert_eq!(
        serde_json::to_value(InboxKind::Item(ItemKind::ObjectiveState)).expect("JSON"),
        json!("objective_state")
    );
    assert!(serde_json::from_value::<InboxKind>(json!("bulletin")).is_err());
    assert!(serde_json::from_value::<InboxKind>(json!(7)).is_err());
}

#[test]
fn an_error_code_the_contract_does_not_have_yet_still_reads() {
    let later: Error =
        serde_json::from_value(json!({"error": "some_new_code", "message": "Try again."}))
            .expect("a new code");
    assert_eq!(later.error, "some_new_code");
    assert_eq!(later.item, None);
    // With no `item`, none is written; the contract has none there.
    assert_eq!(
        serde_json::to_value(&later).expect("JSON"),
        json!({"error": "some_new_code", "message": "Try again."})
    );
}

#[test]
fn a_gif_search_answer_reads() {
    let example = json!({
        "library": "example",
        "attribution": "Powered by Example",
        "results": [
            {
                "id": "abc123",
                "width": 480,
                "height": 270,
                "preview": "https://media.example.com/abc123/preview.gif",
            },
        ],
        "next_offset": 50,
    });
    let read: Gifs = serde_json::from_value(example.clone()).expect("Gifs");
    assert_eq!(read.results[0].width, 480);
    assert_eq!(read.next_offset, Some(50));
    assert_eq!(serde_json::to_value(&read).expect("JSON"), example);

    let last =
        json!({"library": "example", "attribution": "x", "results": [], "next_offset": null});
    let read: Gifs = serde_json::from_value(last.clone()).expect("the last page");
    assert_eq!(read.next_offset, None);
    assert_eq!(serde_json::to_value(&read).expect("JSON"), last);
}

#[test]
fn the_checks_themselves_notice_a_problem() {
    // A lost field, a field added with something in it, a changed value, and a changed count.
    let sent = json!({"a": 1, "b": {"c": [1, 2]}});
    assert_eq!(first_difference(&sent, &sent, ""), None);
    assert!(first_difference(&sent, &json!({"b": {"c": [1, 2]}}), "").is_some());
    assert!(first_difference(&sent, &json!({"a": 1, "b": {"c": [1, 2]}, "d": 0}), "").is_some());
    assert!(first_difference(&sent, &json!({"a": 2, "b": {"c": [1, 2]}}), "").is_some());
    assert!(first_difference(&sent, &json!({"a": 1, "b": {"c": [1]}}), "").is_some());
    // A `null` that was not sent is fine.
    assert_eq!(
        first_difference(&sent, &json!({"a": 1, "b": {"c": [1, 2]}, "d": null}), ""),
        None
    );

    // A wrong word, and a word that writes back differently, are found.
    assert!(!word_problems::<AskedBy>("AskedBy", &["you".to_owned()]).is_empty());
    assert!(!word_problems::<AskedBy>("AskedBy", &[]).is_empty());
    assert!(word_problems::<AskedBy>("AskedBy", &["me".to_owned()]).is_empty());
    assert!(
        !word_problems::<Badge>("Badge", &["top_helper".to_owned()]).is_empty(),
        "lenient"
    );
    assert!(answer_word_problems::<Badge>("Badge", &["top_helper".to_owned()]).is_empty());
    assert!(
        !answer_word_problems::<AskedBy>("AskedBy", &["me".to_owned()]).is_empty(),
        "strict"
    );

    // The later field goes in the top object only, or in every object.
    let nested = json!({"a": [{"b": 1}]});
    assert_eq!(
        with_later_field(&nested, false),
        json!({"a": [{"b": 1}], "later_field": 1})
    );
    assert_eq!(
        with_later_field(&nested, true),
        json!({"a": [{"b": 1, "later_field": 1}], "later_field": 1})
    );
}
