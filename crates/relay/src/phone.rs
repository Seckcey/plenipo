//! A phone's connection (`/plenipo/v1/phone`): its pass or its PC's pairing mailbox, then sealed
//! messages both ways until the phone or its PC leaves.

use plenipo_relay_contract::wire::{self, codes, PhoneToRelay, RelayToPhone};

use crate::hub::Hub;
use crate::limits::AddressKey;
use crate::line::{Line, Next};

/// Messages the relay could not read from one phone before it gives up on it.
const MOST_UNREADABLE: u32 = 20;

fn refuse(hub: &Hub, line: &Line, address: AddressKey, code: &str) {
    hub.refused(code, address);
    line.link.send(wire::write(&RelayToPhone::Refused {
        code: code.to_owned(),
    }));
    line.link.close();
}

pub(crate) async fn serve(hub: &Hub, line: &mut Line, address: AddressKey) {
    let Next::Text(first) = line.first(hub.config.limits.first_message).await else {
        return;
    };
    let joined = match wire::read::<PhoneToRelay>(&first) {
        Some(PhoneToRelay::Pass { pass }) => hub.join_with_pass(&pass, line.link.clone()),
        Some(PhoneToRelay::Mailbox { mailbox }) if crate::is_mailbox_name(&mailbox) => {
            hub.join_mailbox(&mailbox, line.link.clone())
        }
        _ => Err(codes::BAD_PASS),
    };
    let conn = match joined {
        Ok(conn) => conn,
        Err(code) => {
            refuse(hub, line, address, code);
            return;
        }
    };
    line.link.send(wire::write(&RelayToPhone::Ready));
    log::debug!("phone joined ({address})");

    let mut unreadable = 0u32;
    loop {
        match line.next().await {
            Next::Text(text) => match wire::read::<PhoneToRelay>(&text) {
                Some(PhoneToRelay::Data { data }) => {
                    if !crate::fits(&data) {
                        refuse(hub, line, address, codes::TOO_BIG);
                        break;
                    }
                    if hub.send_to_pc(&conn, data).is_err() {
                        line.link.send(wire::write(&RelayToPhone::PcOffline));
                        line.link.close();
                        break;
                    }
                }
                _ => {
                    unreadable += 1;
                    if unreadable > MOST_UNREADABLE {
                        log::info!("phone sent too much the relay could not read ({address})");
                        line.link.close();
                        break;
                    }
                }
            },
            Next::TooFast => {
                refuse(hub, line, address, codes::TOO_MANY_TRIES);
                break;
            }
            Next::Gone => break,
        }
    }
    hub.unregister_phone(&conn);
    log::debug!("phone left ({address})");
}
