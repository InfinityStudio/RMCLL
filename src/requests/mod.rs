#![allow(dead_code)]

use std::fmt;
use std::error;
use std::result::Result;
use std::collections::HashMap;

use uuid::Uuid;
use serde_json;
use hyper::error::UriError;
use hyper::client::FutureResponse;
use hyper::header::{ContentType, ContentLength};
use hyper::{Client, Method, Request, Error as HyperError};
use hyper_tls::HttpsConnector;
use tokio_core::reactor::{Core, Handle};
use futures::{Poll, Future, Stream, IntoFuture};

use versions;
use yggdrasil;

#[derive(Debug)]
pub enum Error {
    UnrecognizedJson(String),
    NetworkIOError(Box<error::Error + Send + Sync>),
}

pub struct RequestFuture<T>(Box<Future<Item=T, Error=Error>>);

impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        Error::NetworkIOError(Box::new(e))
    }
}

impl From<UriError> for Error {
    fn from(e: UriError) -> Self {
        Error::NetworkIOError(Box::new(e))
    }
}

impl From<HyperError> for Error {
    fn from(e: HyperError) -> Self {
        Error::NetworkIOError(Box::new(e))
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Error::UnrecognizedJson(ref s) => fmt::Display::fmt(s, f),
            Error::NetworkIOError(ref e) => fmt::Display::fmt(e, f),
        }
    }
}

impl<T> RequestFuture<T> {
    fn new<F: Future<Item=T, Error=Error> + 'static>(future: F) -> RequestFuture<T> {
        RequestFuture(Box::new(future))
    }
}

impl<T> Future for RequestFuture<T> {
    type Item = T;
    type Error = Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        self.0.as_mut().poll()
    }
}

fn make_json_https_request(handle: Handle,
                           url: &str,
                           json_value: serde_json::Value) -> Result<FutureResponse, Error> {
    let connector = HttpsConnector::new(4, &handle).unwrap();
    let client = Client::configure().connector(connector).keep_alive(true).build(&handle);

    let request = match json_value {
        serde_json::Value::Null => Request::new(Method::Get, url.parse()?),
        _ => {
            let json = json_value.to_string();
            let mut req = Request::new(Method::Post, url.parse()?);
            req.headers_mut().set(ContentType::json());
            req.headers_mut().set(ContentLength(json.len() as u64));
            req.set_body(json);
            req
        }
    };

    Result::Ok(client.request(request))
}

fn make_json_request(handle: Handle,
                     url: &str,
                     json_value: serde_json::Value) -> RequestFuture<serde_json::Value> {
    RequestFuture::new(make_json_https_request(handle, url, json_value).into_future().and_then(|req| {
        req.map_err(Error::from).and_then(|res| {
            res.body().concat2().map_err(Error::from).and_then(|body| {
                serde_json::from_slice(&body).map_err(Error::from).into_future()
            })
        })
    }))
}

pub fn req_authenticate(username: &str,
                        password: &str,
                        client_token: &Uuid) -> Result<(Uuid, yggdrasil::Profile), Error> {
    let mut core = Core::new().unwrap();

    let req = make_json_request(core.handle(), "https://authserver.mojang.com/authenticate", json!({
        "username": username,
        "password": password,
        "clientToken": client_token.simple().to_string(),
        "agent": { "name": "Minecraft", "version": 1 }
    }));

    core.run(req.map(|json| {
        let error = || Error::UnrecognizedJson(json.to_string());
        let uuid = Uuid::parse_str(json["selectedProfile"]["id"].as_str().ok_or(error())?).map_err(|_| error())?;
        let name = json["selectedProfile"]["name"].as_str().ok_or(error())?.to_owned();
        let properties = HashMap::new(); // TODO: deserialize properties
        let access_token_string = json["accessToken"].as_str().ok_or(error())?;
        let access_token = Uuid::parse_str(access_token_string).map_err(|_| error())?;
        Result::Ok((access_token, yggdrasil::Profile::new(uuid, name, properties)))
    }))?
}

pub fn req_refresh(access_token: &Uuid,
                   client_token: &Uuid) -> Result<(Uuid, yggdrasil::Profile), Error> {
    let mut core = Core::new().unwrap();

    let req = make_json_request(core.handle(), "https://authserver.mojang.com/refresh", json!({
        "accessToken": access_token.simple().to_string(),
        "clientToken": client_token.simple().to_string()
    }));

    core.run(req.map(|json| {
        let error = || Error::UnrecognizedJson(json.to_string());
        let uuid = Uuid::parse_str(json["selectedProfile"]["id"].as_str().ok_or(error())?).map_err(|_| error())?;
        let name = json["selectedProfile"]["name"].as_str().ok_or(error())?.to_owned();
        let properties = HashMap::new(); // TODO: deserialize properties
        let access_token_string = json["accessToken"].as_str().ok_or(error())?;
        let access_token = Uuid::parse_str(access_token_string).map_err(|_| error())?;
        Result::Ok((access_token, yggdrasil::Profile::new(uuid, name, properties)))
    }))?
}

pub fn req_versions() -> Result<serde_json::Value, Error> {
    let mut core = Core::new().unwrap();
    let url = "https://launchermeta.mojang.com/mc/game/version_manifest.json";

    let req = make_json_request(core.handle(), url, serde_json::Value::Null);

    core.run(req)
}

pub fn req_deserialize_version(url: &str) -> Result<versions::MinecraftVersion, Error> {
    let mut core = Core::new().unwrap();

    let req = make_json_request(core.handle(), url, serde_json::Value::Null);

    core.run(req.map(|json| {
        Result::Ok(serde_json::from_value(json.clone()).unwrap())
    }))?
}
