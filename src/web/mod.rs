use crate::{state::State as AppState, web::errors::APIError};
use axum::{
    body::Body, extract::Request, handler::HandlerWithoutStateExt, http::StatusCode,
    response::IntoResponse, routing::*, Router,
};

use color_eyre::eyre::Result;
use std::net::SocketAddr;
use tower::ServiceExt;

use self::errors::{error_404, APIResult};

mod api;
mod errors;
mod middleware;
mod webdav;

pub async fn run(state: AppState, addr: SocketAddr) -> Result<()> {
    let api_router = Router::new()
        // unauthenticated routes
        .nest(
            "/api",
            Router::new()
                .route("/login", post(api::login))
                .route("/logout", post(api::logout))
                .route("/guestbook", get(api::get_guestbook))
                .route("/guestbook", post(api::add_guestbook_entry))
                .route("/test", get((StatusCode::OK, "test")))
                .route("/me", get(api::get_me))
                .route("/public_key", post(api::add_public_key))
                .route("/public_key", delete(api::remove_public_key)),
            // .route("/project", post(api::create_project))
            //                                              .route("/project", delete(api::delete_project))
        )
        .route("/webdav", any(webdav::handler))
        .route("/webdav/", any(webdav::handler))
        .route("/webdav/*rest", any(webdav::handler))
        .fallback(|| async { APIResult::<Body>::Err(APIError::NotFound) })
        .with_state(state.clone());

    // Use a different router based on the hostname.
    let app = |request: Request| async move {
        // We don't use the Host extractor as it returns X-Forwarded-Host if present,
        // which can be spoofed by the client.
        let hostname_header = request
            .headers()
            .get("HOST")
            .ok_or_else(|| APIError::BadRequest("no hostname".to_string()))?
            .to_str()
            .map_err(|_| APIError::BadRequest("invalid hostname".to_string()))?;

        let (hostname, port) = if let Some(colon) = hostname_header.find(':') {
            let (hostname, port) = hostname_header.split_at(colon);
            (hostname, &port[1..])
        } else {
            (hostname_header, "80")
        };

        let domain = addr::parse_domain_name(hostname)
            .map_err(|_| APIError::BadRequest("invalid hostname".to_string()))?;

        let is_on_dawdle_space = if cfg!(debug_assertions) {
            (domain.root() == Some("dawdle.localhost") && domain.suffix() == "localhost")
                || (domain.root().is_none() && domain.suffix() == "localhost")
        } else {
            // this should get redirected by our reverse proxy
            if port != "80" {
                return APIResult::Err(APIError::BadRequest("insecure connection".to_string()));
            }
            domain.root() == Some("dawdle.space") && domain.suffix() == "space"
        };

        let is_api = {
            if cfg!(debug_assertions) {
                is_on_dawdle_space && domain.prefix().is_none()
            } else {
                // this should get redirected by our reverse proxy
                if port != "80" {
                    return APIResult::Err(APIError::BadRequest("insecure connection".to_string()));
                }
                domain.root() == Some("dawdle.space")
                    && domain.prefix().is_none()
                    && domain.suffix() == "space"
            }
        };

        if is_api {
            APIResult::Ok(api_router.oneshot(request).await.into_response())
        } else {
            let website = match is_on_dawdle_space {
                true => {
                    let domains = state
                        .subdomains
                        .read()
                        .map_err(|_| APIError::InternalServerError)?;
                    domains.get(domain.prefix().unwrap()).cloned()
                }
                false => {
                    let domains = state
                        .custom_domains
                        .read()
                        .map_err(|_| APIError::InternalServerError)?;
                    domains.get(hostname).cloned()
                }
            };

            let Some(website) = website else {
                return APIResult::Ok(error_404());
            };

            APIResult::Ok(format!("website: {:?}", website).into_response())
        }
    };

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app.into_service()).await?;

    Ok(())
}
