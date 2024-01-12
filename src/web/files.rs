use std::convert::Infallible;
use std::io::SeekFrom;
use std::ops::RangeInclusive;
use std::path::{Component, Path, PathBuf};

use super::errors::APIError;
use axum::http::{header, HeaderValue, Method, StatusCode};
use axum::response::Response;
use axum::{body::Body, extract::Request, response::IntoResponse};
use http_range_header::RangeUnsatisfiableError;
use httpdate::HttpDate;
use tokio::io::{AsyncReadExt, AsyncSeekExt};
use tokio_util::io::ReaderStream;
use tower::{service_fn, Service};

use percent_encoding::percent_decode;

// based on https://github.com/tower-rs/tower-http
// License: MIT - Copyright (c) 2019-2021 Tower Contributors
//
// Changes:
// - Only extracted the parts needed for this project
// - Added fallback file
// - Added fallback response if fallback file doesn't exist
// - Removed / redirects
// - Serve .html files if no file extension is given as fallback

pub fn create_dir_service(
    path: PathBuf,
    fallback_file: PathBuf,
    fallback: impl IntoResponse + Clone + Send + Sync + 'static,
) -> impl Service<Request, Response = impl IntoResponse, Error = Infallible, Future = impl Send> + Clone
{
    service_fn(move |req: Request| {
        let base_path = path.clone();
        let fallback_file = fallback_file.clone();
        let fallback = fallback.clone();

        async move {
            if req.method() != Method::GET && req.method() != Method::HEAD {
                return Ok(
                    APIError::custom(StatusCode::METHOD_NOT_ALLOWED, "Method not allowed")
                        .into_response(),
                );
            }

            let path_to_file = match build_and_validate_path(&base_path, req.uri().path()) {
                None => {
                    return Ok(
                        APIError::custom(StatusCode::BAD_REQUEST, "invalid path").into_response()
                    )
                }
                Some(path) => path,
            };

            let buf_chunk_size = 65536;
            let range_header = req
                .headers()
                .get(header::RANGE)
                .and_then(|value| value.to_str().ok())
                .map(|s| s.to_owned());

            let if_unmodified_since = req
                .headers()
                .get(header::IF_UNMODIFIED_SINCE)
                .and_then(to_http_date);

            let if_modified_since = req
                .headers()
                .get(header::IF_MODIFIED_SINCE)
                .and_then(to_http_date);

            if req.method() == Method::HEAD {
                return Ok(APIError::error("not supported yet").into_response());
            }

            let path_to_file = if is_dir(&path_to_file).await {
                path_to_file.join("./index.html")
            } else {
                path_to_file
            };

            let (mut file, mime) = match open_file(path_to_file).await {
                Ok(Some(file)) => file,
                Ok(None) => {
                    let Ok(file) = tokio::fs::File::open(&fallback_file).await else {
                        return Ok(fallback.into_response());
                    };
                    (file, guess_mime(&fallback_file))
                }
                Err(err) => return Ok(err.into_response()),
            };

            let meta = match file.metadata().await {
                Ok(meta) => meta,
                Err(_) => return Ok(APIError::bad_request().into_response()),
            };

            if !meta.is_file() {
                return Ok(fallback.into_response());
            }

            let last_modified: Option<HttpDate> = meta.modified().ok().map(|time| time.into());
            if let Some(resp) =
                check_modified_headers(last_modified, if_unmodified_since, if_modified_since)
            {
                return Ok(resp);
            }

            let maybe_range = try_parse_range(range_header.as_deref(), meta.len());
            if let Some(Ok(ranges)) = maybe_range.as_ref() {
                // if there is any other amount of ranges than 1 we'll return an
                // unsatisfiable later as there isn't yet support for multipart ranges
                if ranges.len() == 1 {
                    if let Err(_) = file.seek(SeekFrom::Start(*ranges[0].start())).await {
                        return Ok(APIError::error("failed to seek").into_response());
                    }
                }
            }

            // we can actually return the file now
            Ok(build_response(FileOutput {
                chunk_size: buf_chunk_size,
                file: Some(file),
                last_modified,
                maybe_range,
                metadata: meta,
                mime_header_value: mime,
            }))
        }
    })
}

struct FileOutput {
    // not included on HEAD requests
    pub(super) file: Option<tokio::fs::File>,
    pub(super) metadata: std::fs::Metadata,

    pub(super) chunk_size: usize,
    pub(super) mime_header_value: HeaderValue,
    pub(super) maybe_range: Option<Result<Vec<RangeInclusive<u64>>, RangeUnsatisfiableError>>,
    pub(super) last_modified: Option<HttpDate>,
}

async fn is_dir(path: &PathBuf) -> bool {
    tokio::fs::metadata(path)
        .await
        .map_or(false, |meta_data| meta_data.is_dir())
}

fn build_response(output: FileOutput) -> Response<Body> {
    let mut builder = Response::builder()
        .header(header::CONTENT_TYPE, output.mime_header_value)
        .header(header::ACCEPT_RANGES, "bytes");

    if let Some(last_modified) = output.last_modified {
        builder = builder.header(header::LAST_MODIFIED, last_modified.to_string());
    }

    let size = output.metadata.len();

    match output.maybe_range {
        Some(Ok(ranges)) => {
            if let Some(range) = ranges.first() {
                if ranges.len() > 1 {
                    return APIError::error("multipart ranges not supported yet").into_response();
                } else {
                    let body = if let Some(file) = output.file {
                        let range_size = range.end() - range.start() + 1;

                        let stream =
                            ReaderStream::with_capacity(file.take(range_size), output.chunk_size);
                        Body::from_stream(stream)
                    } else {
                        Body::empty()
                    };

                    builder
                        .header(
                            header::CONTENT_RANGE,
                            format!("bytes {}-{}/{}", range.start(), range.end(), size),
                        )
                        .header(header::CONTENT_LENGTH, range.end() - range.start() + 1)
                        .status(StatusCode::PARTIAL_CONTENT)
                        .body(body)
                        .unwrap()
                }
            } else {
                APIError::error("No range found after parsing range header").into_response()
            }
        }

        Some(Err(_)) => builder
            .header(header::CONTENT_RANGE, format!("bytes */{}", size))
            .status(StatusCode::RANGE_NOT_SATISFIABLE)
            .body(Body::empty())
            .unwrap(),

        // Not a range request
        None => {
            let body = if let Some(file) = output.file {
                Body::from_stream(ReaderStream::with_capacity(file, output.chunk_size))
            } else {
                Body::empty()
            };

            builder
                .header(header::CONTENT_LENGTH, size.to_string())
                .body(body)
                .unwrap()
        }
    }
}

fn try_parse_range(
    maybe_range_ref: Option<&str>,
    file_size: u64,
) -> Option<Result<Vec<RangeInclusive<u64>>, RangeUnsatisfiableError>> {
    maybe_range_ref.map(|header_value| {
        http_range_header::parse_range_header(header_value)
            .and_then(|first_pass| first_pass.validate(file_size))
    })
}

fn check_modified_headers(
    modified: Option<HttpDate>,
    if_unmodified_since: Option<HttpDate>,
    if_modified_since: Option<HttpDate>,
) -> Option<Response> {
    if let Some(since) = if_unmodified_since {
        let precondition = modified
            .as_ref()
            .map(|time| since >= *time)
            .unwrap_or(false);

        if !precondition {
            return Some(
                Response::builder()
                    .status(StatusCode::PRECONDITION_FAILED)
                    .body(Body::empty())
                    .unwrap(),
            );
        }
    }

    if let Some(since) = if_modified_since {
        let unmodified = modified
            .as_ref()
            .map(|time| !(since < *time))
            // no last_modified means its always modified
            .unwrap_or(false);
        if unmodified {
            return Some(
                Response::builder()
                    .status(StatusCode::NOT_MODIFIED)
                    .body(Body::empty())
                    .unwrap(),
            );
        }
    }

    None
}

// returns None if the fallback file doesn't exist
async fn open_file(
    path_to_file: PathBuf,
) -> Result<Option<(tokio::fs::File, HeaderValue)>, APIError> {
    let file = tokio::fs::File::open(&path_to_file).await;
    match file {
        Ok(file) => Ok(Some((file, guess_mime(&path_to_file)))),
        Err(err) => {
            if err.kind() == std::io::ErrorKind::NotFound {
                // try .html if it's not at the end of the file already
                if !path_to_file.ends_with(".html") {
                    if let Ok(file) =
                        tokio::fs::File::open(path_to_file.with_extension("html")).await
                    {
                        return Ok(Some((file, HeaderValue::from_static("text/html"))));
                    }
                }

                Ok(None)
            } else {
                Err(APIError::custom(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    &format!("failed to open file: {}", err),
                ))
            }
        }
    }
}

fn guess_mime(path: &PathBuf) -> HeaderValue {
    mime_guess::from_path(path)
        .first_raw()
        .map(HeaderValue::from_static)
        .unwrap_or_else(|| HeaderValue::from_static("application/octet-stream"))
}

fn to_http_date(value: &HeaderValue) -> Option<HttpDate> {
    std::str::from_utf8(value.as_bytes())
        .ok()
        .and_then(|value| httpdate::parse_http_date(value).ok())
        .map(|time| time.into())
}

fn build_and_validate_path(base_path: &std::path::Path, requested_path: &str) -> Option<PathBuf> {
    let path = requested_path.trim_start_matches('/');
    let path_decoded = percent_decode(path.as_ref()).decode_utf8().ok()?;
    let path_decoded = Path::new(&*path_decoded);

    let mut path_to_file = base_path.to_path_buf();
    for component in path_decoded.components() {
        match component {
            Component::Normal(comp) => {
                // protect against paths like `/foo/c:/bar/baz` (#204)
                match Path::new(&comp)
                    .components()
                    .all(|c| matches!(c, Component::Normal(_)))
                {
                    true => path_to_file.push(comp),
                    false => return None,
                }
            }
            Component::CurDir => {}
            Component::Prefix(_) | Component::RootDir | Component::ParentDir => {
                return None;
            }
        }
    }
    Some(path_to_file)
}