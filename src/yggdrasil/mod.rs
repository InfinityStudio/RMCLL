#![allow(dead_code)]

use std::fmt::{self, Display};
use std::collections::HashMap;

use uuid::{Uuid, NAMESPACE_OID};
use serde_json;

use requests;

#[derive(Debug)]
pub struct Profile {
    uuid: Uuid,
    name: String,
    properties: HashMap<String, String>,
}

#[derive(Debug)]
pub struct AuthInfo {
    access_token: Uuid,
    user_profile: Profile,
}

pub struct OfflineAuthenticator(String);

pub struct YggdrasilLoginAuthenticator {
    username: String,
    password: String,
    client_token: Uuid,
}

pub trait Authenticator {
    type Error;

    fn auth(&self) -> Result<AuthInfo, Self::Error>;
}

impl Profile {
    #[inline]
    pub fn new(uuid: Uuid, name: String, properties: HashMap<String, String>) -> Profile {
        Profile { uuid, name, properties }
    }

    #[inline]
    pub fn uuid(&self) -> &Uuid {
        &self.uuid
    }

    #[inline]
    pub fn name(&self) -> &String {
        &self.name
    }

    #[inline]
    pub fn properties(&self) -> &HashMap<String, String> {
        &self.properties
    }
}

impl Display for Profile {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.properties.is_empty() {
            write!(f, "{}: {}", self.name, self.uuid.simple())
        } else {
            write!(f, "{}: {} {}", self.name, self.uuid.simple(),
                   serde_json::to_string(&self.properties).unwrap())
        }
    }
}

impl AuthInfo {
    #[inline]
    pub fn new(access_token: Uuid, user_profile: Profile) -> AuthInfo {
        AuthInfo { access_token, user_profile }
    }

    #[inline]
    pub fn access_token(&self) -> &Uuid {
        &self.access_token
    }

    #[inline]
    pub fn user_profile(&self) -> &Profile {
        &self.user_profile
    }
}

impl Authenticator for OfflineAuthenticator {
    type Error = requests::Error;

    fn auth(&self) -> Result<AuthInfo, requests::Error> {
        let access_token = Uuid::new_v4();
        let uuid = Uuid::new_v5(&NAMESPACE_OID, self.0.as_str());
        let profile = Profile::new(uuid, self.0.clone(), HashMap::new());
        Result::Ok(AuthInfo::new(access_token, profile))
    }
}

impl Authenticator for YggdrasilLoginAuthenticator {
    type Error = requests::Error;

    fn auth(&self) -> Result<AuthInfo, requests::Error> {
        let username = self.username.as_str();
        let password = self.password.as_str();
        let (token, profile) = requests::req_authenticate(username, password, &self.client_token)?;
        Result::Ok(AuthInfo::new(token, profile))
    }
}

#[inline]
pub fn offline(offline_name: &str) -> OfflineAuthenticator {
    OfflineAuthenticator(offline_name.to_owned())
}

#[inline]
pub fn yggdrasil(username: &str, password: &str) -> YggdrasilLoginAuthenticator {
    yggdrasil_with_client_token(username.to_owned(), password.to_owned(), Uuid::new_v4())
}

#[inline]
pub fn yggdrasil_with_client_token(username: String,
                                   password: String,
                                   client_token: Uuid) -> YggdrasilLoginAuthenticator {
    YggdrasilLoginAuthenticator { username, password, client_token }
}
