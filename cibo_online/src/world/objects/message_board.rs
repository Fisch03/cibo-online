use crate::{assets, AsSprite, Sprite};
use alloc::boxed::Box;
use monos_gfx::{Dimension, Position, Rect};

#[derive(Debug)]
pub struct MessageBoard {
    position: Position,
    hitbox: Rect,
}

impl MessageBoard {
    pub fn new(position: Position) -> Box<dyn AsSprite> {
        let dimensions = assets().message_board.dimensions();
        let hitbox = Rect::new(
            Position::new(0, dimensions.height as i64 - 10),
            Position::from_dimensions(dimensions),
        );

        Box::new(MessageBoard { position, hitbox })
    }
}

impl AsSprite for MessageBoard {
    fn as_sprite(&self) -> Sprite {
        Sprite::Static {
            position: self.position,
            image: &assets().message_board,
        }
    }

    #[inline]
    fn position(&self) -> Position {
        self.position
    }

    #[inline]
    fn dimensions(&self) -> Dimension {
        assets().message_board.dimensions()
    }

    #[inline]
    fn hitbox_rel(&self) -> Option<Rect> {
        Some(self.hitbox)
    }
}
