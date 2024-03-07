use std::str::FromStr;

use common::{
    http::{
        HttpRequestStream,
        HttpResponse,
        HttpResponseStream,
    },
    runtime::Runtime,
};
use futures::{
    channel::mpsc,
    stream::BoxStream,
};
use headers::{
    HeaderMap,
    HeaderName,
};
use http::{
    Method,
    StatusCode,
};
use serde::{
    Deserialize,
    Serialize,
};
use url::Url;

use crate::{
    environment::IsolateEnvironment,
    execution_scope::ExecutionScope,
    request_scope::StreamListener,
    HttpActionRequestHead,
};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HttpRequestV8 {
    pub header_pairs: Vec<(String, String)>,
    pub url: String,
    pub method: String,
    pub stream_id: Option<uuid::Uuid>,
}

impl HttpRequestV8 {
    pub fn into_stream<RT: Runtime, E: IsolateEnvironment<RT>>(
        self,
        scope: &mut ExecutionScope<RT, E>,
    ) -> anyhow::Result<HttpRequestStream> {
        let mut header_map = HeaderMap::new();
        for (name, value) in &self.header_pairs {
            header_map.append(HeaderName::from_str(name.as_str())?, value.parse()?);
        }
        let (body_sender, body_receiver) = mpsc::unbounded();
        match self.stream_id {
            Some(stream_id) => {
                scope.new_stream_listener(stream_id, StreamListener::RustStream(body_sender))?;
            },
            None => body_sender.close_channel(),
        };

        Ok(HttpRequestStream {
            body: Box::pin(body_receiver),
            headers: header_map,
            url: Url::parse(&self.url)?,
            method: Method::from_str(&self.method)?,
        })
    }

    pub fn from_request(
        request: HttpActionRequestHead,
        stream_id: Option<uuid::Uuid>,
    ) -> anyhow::Result<Self> {
        let mut header_pairs: Vec<(String, String)> = vec![];

        // Iterate over `&HeaderMap` instead of `HeaderMap` because the latter gives
        // None as the HeaderName for headers with multiple values
        // (https://docs.rs/http/latest/http/header/struct.HeaderMap.html#method.into_iter)
        for (name, value) in &request.headers {
            let value_str = value.to_str()?;
            let header_name_str = name.as_str();
            header_pairs.push((header_name_str.to_string(), value_str.to_string()));
        }

        Ok(Self {
            header_pairs,
            url: request.url.to_string(),
            method: request.method.to_string(),
            stream_id,
        })
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HttpResponseV8 {
    stream_id: Option<uuid::Uuid>,
    status: u16,
    status_text: Option<String>,
    header_pairs: Vec<(String, String)>,
    url: Option<String>,
}

impl HttpResponseV8 {
    pub fn into_response(self) -> anyhow::Result<(HttpResponse, Option<uuid::Uuid>)> {
        let status_code = StatusCode::try_from(self.status)?;

        let mut header_map = HeaderMap::new();
        for (name, value) in &self.header_pairs {
            header_map.append(HeaderName::from_str(name.as_str())?, value.parse()?);
        }

        Ok((
            HttpResponse {
                status: status_code,
                body: None,
                headers: header_map,
                url: self.url.map(|u| Url::parse(u.as_str())).transpose()?,
            },
            self.stream_id,
        ))
    }

    pub fn from_response_stream(
        mut response: HttpResponseStream,
        stream_id: uuid::Uuid,
    ) -> anyhow::Result<(
        Option<BoxStream<'static, anyhow::Result<bytes::Bytes>>>,
        HttpResponseV8,
    )> {
        let body = response.body.take();
        let mut header_pairs: Vec<(String, String)> = vec![];
        // Iterate over `&HeaderMap` instead of `HeaderMap` because the latter gives
        // None as the HeaderName for headers with multiple values
        // (https://docs.rs/http/latest/http/header/struct.HeaderMap.html#method.into_iter)
        for (name, value) in &response.headers {
            let value_str = value.to_str()?;
            let header_name_str = name.as_str();
            header_pairs.push((header_name_str.to_string(), value_str.to_string()));
        }
        // reqwest does not expose status text sent in HTTP response, so we derive it
        // from status code.
        let status_text = response
            .status
            .canonical_reason()
            .map(|reason| reason.to_string());
        Ok((
            body,
            HttpResponseV8 {
                stream_id: Some(stream_id),
                status: response.status.as_u16(),
                status_text,
                header_pairs,
                url: response.url.map(|u| u.to_string()),
            },
        ))
    }
}
