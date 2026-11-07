use super::*;

mod dispatch;
mod framing;
mod schema;

fn hello() -> Request {
    Request::new("req-1", OPERATION_HELLO)
}
