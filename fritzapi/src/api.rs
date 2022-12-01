use lazy_static::lazy_static;
use log::info;
use regex::Regex;
use reqwest::blocking::{get as GET, Client, Response};

use crate::error::{FritzError, Result};
use crate::fritz_xml as xml;

// -=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-

/// Computes the string that we use to authenticate.
/// 1. Replace all non-ascii chars in `password` with "."
/// 2. Concat `challenge` and the modified password
/// 3. Convert that to UTF16le
/// 4. MD5 that byte array
/// 5. concat that as hex with challenge again
fn request_response(password: &str, challenge: &str) -> String {
    lazy_static! {
        static ref RE: Regex = Regex::new(r"[^\x00-\x7F]").unwrap();
    }
    let clean_password = RE.replace_all(password, ".");
    let hash_input = format!("{}-{}", challenge, clean_password);
    let bytes: Vec<u8> = hash_input
        .encode_utf16()
        .flat_map(|utf16| utf16.to_le_bytes().to_vec())
        .collect();
    let digest = md5::compute(bytes);
    format!("{}-{:032x}", challenge, digest)
}

const DEFAULT_SID: &str = "0000000000000000";

pub struct Token {
    sid: String,
    host: String,
}

/// Requests a temporary token (session id = sid) from the fritz box using user
/// name and password.
pub fn get_token(host: &str, user: &str, password: &str) -> Result<Token> {
    let url = format!("http://{}/login_sid.lua", host);
    let res: Response = GET(&url)?.error_for_status().map_err(|err| {
        eprintln!("GET login_sid.lua for user {}", user);
        err
    })?;

    let xml = res.text()?;
    let info = xml::parse_session_info(&xml)?;
    if DEFAULT_SID != info.sid {
        return Ok(Token {
            sid: info.sid,
            host: host.to_string(),
        });
    }
    let response = request_response(password, &info.challenge);
    let url = format!(
        "http://{}/login_sid.lua?username={}&response={}",
        host, user, response
    );
    let login: Response = GET(&url)?.error_for_status()?;
    let info = xml::parse_session_info(&login.text()?)?;

    if DEFAULT_SID == info.sid {
        return Err(FritzError::LoginError(
            "login error - sid is still the default after login attempt".to_string(),
        ));
    }

    Ok(Token {
        sid: info.sid,
        host: host.to_string(),
    })
}

pub(crate) enum Commands {
    GetDeviceListInfos,
    GetBasicDeviceStats,
    // GetSwitchPower,
    // GetSwitchEnergy,
    // GetSwitchName,
    // GetTemplateListInfos,
    SetSwitchOff,
    SetSwitchOn,
    SetSwitchToggle,
}

/// Sends raw HTTP requests to the fritz box.
pub(crate) fn request(cmd: Commands, token: &Token, ain: Option<&str>) -> Result<String> {
    use Commands::*;
    let cmd = match cmd {
        GetDeviceListInfos => "getdevicelistinfos",
        GetBasicDeviceStats => "getbasicdevicestats",
        // GetSwitchPower => "getswitchpower",
        // GetSwitchEnergy => "getswitchenergy",
        // GetSwitchName => "getswitchname",
        // GetTemplateListInfos => "gettemplatelistinfos",
        SetSwitchOff => "setswitchoff",
        SetSwitchOn => "setswitchon",
        SetSwitchToggle => "setswitchtoggle",
    };
    let url = format!("http://{}/webservices/homeautoswitch.lua", token.host);
    let mut client = Client::new()
        .get(url)
        .query(&[("switchcmd", cmd), ("sid", &token.sid)]);
    if let Some(ain) = ain {
        client = client.query(&[("ain", ain)]);
    }
    let response = client.send()?;
    let status = response.status();
    info!(
        "[fritz api] {} status: {:?} {:?}",
        cmd,
        status,
        status.canonical_reason().unwrap_or_default()
    );

    Ok(response.text()?)
}

// -=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-

/// Requests & parses raw [`Device`]s.
pub(crate) fn device_infos(token: &Token) -> Result<Vec<xml::Device>> {
    let xml = request(Commands::GetDeviceListInfos, &token, None)?;
    xml::parse_device_infos(xml)
}

/// Requests & parses raw [`DeviceStats`]s.
pub(crate) fn fetch_device_stats(ain: &str, token: &Token) -> Result<Vec<xml::DeviceStats>> {
    let xml = request(Commands::GetBasicDeviceStats, &token, Some(ain))?;
    xml::parse_device_stats(xml)
}

// -=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-=-

#[cfg(test)]
mod tests {
    #[test]
    fn request_response() {
        let response = super::request_response("mühe", "foo");
        assert_eq!(response, "foo-442e12bbceabd35c66964c913a316451");
    }
}
