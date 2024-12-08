#![allow(unused_imports)]

mod message_board;
pub use message_board::MessageBoard;

mod easel;
pub use easel::Easel;

pub mod beach_ball;
pub use beach_ball::BeachBall;

pub mod snowflake;
pub use snowflake::Snowflake;

pub fn setup_network_objects() {
    use super::network_object::register_objects;
    register_objects! {
        BeachBall
    }
}
