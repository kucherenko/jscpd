pub enum Method {
    Get,
    Post,
    Delete,
    Other,
}

pub fn status_for(method: Method, authorized: bool) -> u16 {
    match method {
        Method::Get => 200,
        Method::Post if authorized => 201,
        Method::Post => 401,
        Method::Delete => 204,
        Method::Other => 405,
    }
}

pub fn describe(code: u16) -> &'static str {
    match code {
        200..=299 => "success",
        400..=499 => "client error",
        _ => "server error",
    }
}
